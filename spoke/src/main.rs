use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::*;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, mpsc, oneshot, watch},
    time::{Duration, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, warn};
use uuid::Uuid;

mod orchestrator;
mod source_bundle;
mod workspace;

#[cfg(test)]
mod tests;

#[derive(Parser)]
#[command(
    name = "pumpkinpi-spoke",
    version,
    about = "PumpkinPi situated execution daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
    #[arg(long, env = "PUMPKINPI_SPOKE_DATA")]
    data_dir: Option<PathBuf>,
}
#[derive(Subcommand)]
enum Cmd {
    Enroll {
        #[arg(long)]
        hub: String,
        #[arg(long)]
        setup_key: String,
    },
    Serve {
        #[arg(long)]
        hub: Option<String>,
    },
    Reset {
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Clone, Serialize, Deserialize)]
struct Config {
    spoke_id: SpokeId,
    hub_url: String,
    trusted_roots: Vec<PathBuf>,
    max_runs_per_project: usize,
    #[serde(default)]
    pi_binary: Option<PathBuf>,
}
#[derive(Default, Serialize, Deserialize)]
struct Store {
    projects: BTreeMap<ProjectId, ProjectRecord>,
    sources: BTreeMap<ProjectId, SourceOfIntentRecord>,
    chats: BTreeMap<ProjectId, IntentChatRecord>,
    timelines: BTreeMap<ProjectId, Vec<TimelineItem>>,
    operations: BTreeMap<OperationId, OperationRecord>,
    sessions: BTreeMap<SessionId, SessionRecord>,
    #[serde(default)]
    reviews: BTreeMap<ReviewId, ReviewRecord>,
    #[serde(default)]
    workspaces: BTreeMap<OperationId, workspace::WorkspaceRecord>,
    #[serde(default)]
    realizations: BTreeMap<OperationId, orchestrator::RealizationMachine>,
}
const INTERNAL_RESUME_MESSAGE: &str = "__pumpkinpi_internal_resume_active_realization_v3__";

type Tx = mpsc::UnboundedSender<SpokeToHub>;
type Interactions = Arc<Mutex<HashMap<(OperationId, String), oneshot::Sender<serde_json::Value>>>>;
#[derive(Clone)]
struct State {
    config: Config,
    data_dir: PathBuf,
    store: Arc<Mutex<Store>>,
    project_lanes: Arc<Mutex<HashMap<ProjectId, Arc<Mutex<()>>>>>,
    realization_lanes: Arc<Mutex<HashMap<ProjectId, Arc<Mutex<()>>>>>,
    cancellations: Arc<Mutex<HashMap<OperationId, watch::Sender<bool>>>>,
    interactions: Interactions,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let dir = cli.data_dir.unwrap_or_else(default_dir);
    match cli.command {
        Cmd::Enroll { hub, setup_key } => enroll(&dir, &hub, &setup_key).await,
        Cmd::Serve { hub } => serve(dir, hub).await,
        Cmd::Reset { yes } => {
            if !yes {
                return Err(anyhow!("reset destroys prerelease state; pass --yes"));
            }
            if dir.exists() {
                tokio::fs::remove_dir_all(&dir).await?
            }
            Ok(())
        }
    }
}
fn default_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/state/pumpkinpi-spoke-v3")
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}
fn ws_url(h: &str) -> String {
    let h = h
        .trim_end_matches('/')
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    format!("{h}/ws/spoke")
}

async fn enroll(dir: &Path, hub: &str, setup_key: &str) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let key = SigningKey::generate(&mut OsRng);
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "spoke".into());
    let body = EnrollRequest {
        setup_key: setup_key.into(),
        hostname,
        version: env!("CARGO_PKG_VERSION").into(),
        public_key: BASE64.encode(key.verifying_key().to_bytes()),
    };
    let response = reqwest::Client::new()
        .post(format!("{}/api/spokes/enroll", hub.trim_end_matches('/')))
        .json(&body)
        .send()
        .await?
        .json::<EnrollResponse>()
        .await?;
    if !response.ok {
        return Err(anyhow!(
            response.error.unwrap_or_else(|| "enrollment failed".into())
        ));
    }
    let config = Config {
        spoke_id: response.spoke_id.context("missing spoke id")?,
        hub_url: response.hub_url.unwrap_or_else(|| hub.into()),
        trusted_roots: vec![
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/")),
        ],
        max_runs_per_project: 4,
        pi_binary: None,
    };
    write_secure(
        &dir.join("config.json"),
        &serde_json::to_vec_pretty(&config)?,
    )
    .await?;
    write_secure(
        &dir.join("spoke.key"),
        BASE64.encode(key.to_bytes()).as_bytes(),
    )
    .await?;
    save_store(dir, &Store::default()).await?;
    println!("enrolled spoke {}", config.spoke_id);
    Ok(())
}
async fn serve(dir: PathBuf, override_hub: Option<String>) -> Result<()> {
    let mut config: Config = serde_json::from_slice(
        &tokio::fs::read(dir.join("config.json"))
            .await
            .context("spoke is not enrolled")?,
    )?;
    if let Some(h) = override_hub {
        config.hub_url = h
    }
    let mut store = load_store(&dir).await?;
    if reconcile_interrupted(&mut store) {
        save_store(&dir, &store).await?;
    }
    let state = State {
        config: config.clone(),
        data_dir: dir.clone(),
        store: Arc::new(Mutex::new(store)),
        project_lanes: Default::default(),
        realization_lanes: Default::default(),
        cancellations: Default::default(),
        interactions: Default::default(),
    };
    loop {
        match connection(state.clone()).await {
            Ok(()) => warn!("hub disconnected"),
            Err(e) => error!(error=%e,"hub connection failed"),
        };
        sleep(Duration::from_secs(2)).await
    }
}
async fn connection(state: State) -> Result<()> {
    let key_bytes = BASE64.decode(
        tokio::fs::read_to_string(state.data_dir.join("spoke.key"))
            .await?
            .trim(),
    )?;
    let key = SigningKey::from_bytes(&key_bytes.try_into().map_err(|_| anyhow!("invalid key"))?);
    let (socket, _) = connect_async(ws_url(&state.config.hub_url)).await?;
    let (mut write, mut read) = socket.split();
    write
        .send(Message::Text(
            serde_json::to_string(&SpokeToHub::Hello {
                protocol_version: PROTOCOL_VERSION,
                spoke_id: state.config.spoke_id.clone(),
                version: env!("CARGO_PKG_VERSION").into(),
            })?
            .into(),
        ))
        .await?;
    let challenge: text::Challenge = serde_json::from_str(&message_text(
        read.next().await.context("closed before challenge")??,
    )?)?;
    if challenge.kind != "spoke_challenge" {
        return Err(anyhow!("unexpected Hub challenge"));
    }
    let signature = BASE64.encode(key.sign(challenge.nonce.as_bytes()).to_bytes());
    write
        .send(Message::Text(
            serde_json::to_string(&SpokeToHub::Auth {
                protocol_version: PROTOCOL_VERSION,
                spoke_id: state.config.spoke_id.clone(),
                signature,
            })?
            .into(),
        ))
        .await?;
    let auth: text::Authenticated = serde_json::from_str(&message_text(
        read.next().await.context("closed before auth")??,
    )?)?;
    if auth.kind != "spoke_authenticated" {
        return Err(anyhow!("authentication rejected"));
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<SpokeToHub>();
    let writer = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            if write
                .send(Message::Text(serde_json::to_string(&v).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    send_inventory(&state, &tx).await?;
    schedule_recovery(&state, &tx).await?;
    let mut beat = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {_=beat.tick()=>{tx.send(SpokeToHub::Heartbeat{protocol_version:PROTOCOL_VERSION})?}, msg=read.next()=>{let Some(msg)=msg else{break}; let text=message_text(msg?)?; let command:HubToSpoke=serde_json::from_str(&text)?; if let HubToSpoke::Command{request,provider_env}=command { handle_request(state.clone(),tx.clone(),request,provider_env).await; }}}
    }
    writer.abort();
    Ok(())
}
async fn schedule_recovery(state: &State, tx: &Tx) -> Result<()> {
    let pending = {
        let mut store = state.store.lock().await;
        let ids = store
            .operations
            .values()
            .filter(|operation| {
                operation.status == OperationStatus::Queued
                    && store.realizations.contains_key(&operation.operation_id)
                    && store
                        .sources
                        .get(&operation.project_id)
                        .is_some_and(|source| source.status == SourceStatus::Active)
            })
            .map(|operation| (operation.operation_id.clone(), operation.project_id.clone()))
            .collect::<Vec<_>>();
        for (operation_id, _) in &ids {
            if let Some(operation) = store.operations.get_mut(operation_id) {
                operation.status = OperationStatus::Accepted;
                operation.error = Some("resuming after Spoke restart".into());
                operation.updated_at = now();
                operation.completed_at = None;
            }
        }
        save_store(&state.data_dir, &store).await?;
        ids
    };

    for (operation_id, project_id) in pending {
        if let Some(workspace) = state
            .store
            .lock()
            .await
            .workspaces
            .get(&operation_id)
            .cloned()
        {
            workspace::rollback(&workspace).await?;
        }
        let (cancel_tx, cancel_rx) = watch::channel(false);
        state
            .cancellations
            .lock()
            .await
            .insert(operation_id.clone(), cancel_tx);
        let state = state.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(error) = run_operation(
                state.clone(),
                tx.clone(),
                project_id.clone(),
                operation_id.clone(),
                INTERNAL_RESUME_MESSAGE.into(),
                cancel_rx,
                BTreeMap::new(),
            )
            .await
            {
                let cancelled = state
                    .store
                    .lock()
                    .await
                    .operations
                    .get(&operation_id)
                    .is_some_and(|operation| operation.status == OperationStatus::Cancelled);
                if !cancelled {
                    let _ = set_realization_status(&state, &project_id, RealizationStatus::Blocked)
                        .await;
                    let _ = update_operation(
                        &state,
                        &tx,
                        &operation_id,
                        OperationStatus::Failed,
                        Some(error.to_string()),
                    )
                    .await;
                }
            }
            state.cancellations.lock().await.remove(&operation_id);
        });
    }
    Ok(())
}

fn message_text(m: Message) -> Result<String> {
    if let Message::Text(t) = m {
        Ok(t.to_string())
    } else {
        Err(anyhow!("expected text frame"))
    }
}
mod text {
    use serde::Deserialize;
    #[derive(Deserialize)]
    pub struct Challenge {
        #[serde(rename = "type")]
        pub kind: String,
        pub nonce: String,
    }
    #[derive(Deserialize)]
    pub struct Authenticated {
        #[serde(rename = "type")]
        pub kind: String,
    }
}
async fn send_inventory(state: &State, tx: &Tx) -> Result<()> {
    let projects = state
        .store
        .lock()
        .await
        .projects
        .values()
        .cloned()
        .collect();
    tx.send(SpokeToHub::Inventory {
        protocol_version: PROTOCOL_VERSION,
        complete: true,
        revision: now(),
        projects,
    })?;
    Ok(())
}
fn event(id: Option<RequestId>, payload: ClientPayload) -> SpokeToHub {
    SpokeToHub::ClientEvent {
        protocol_version: PROTOCOL_VERSION,
        event: Box::new(ClientEvent {
            protocol_version: PROTOCOL_VERSION,
            id,
            payload,
        }),
    }
}

async fn list_project_directories(state: &State, query: &str) -> Result<(String, Vec<String>)> {
    if query.trim().is_empty() {
        let mut roots = state
            .config
            .trusted_roots
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        return Ok((String::new(), roots));
    }

    let requested = PathBuf::from(query);
    let (directory, prefix) = if requested.is_dir() {
        (requested, String::new())
    } else {
        (
            requested
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .to_path_buf(),
            requested
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
        )
    };
    let canonical = tokio::fs::canonicalize(&directory)
        .await
        .with_context(|| format!("directory is unavailable: {}", directory.display()))?;
    let mut allowed = false;
    for root in &state.config.trusted_roots {
        if let Ok(root) = tokio::fs::canonicalize(root).await
            && canonical.starts_with(root)
        {
            allowed = true;
            break;
        }
    }
    if !allowed {
        return Err(anyhow!("directory is outside the Spoke's trusted roots"));
    }

    let mut entries = tokio::fs::read_dir(&canonical).await?;
    let mut directories = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.to_ascii_lowercase().starts_with(&prefix) || !entry.file_type().await?.is_dir() {
            continue;
        }
        directories.push(entry.path().to_string_lossy().into_owned());
        if directories.len() == 100 {
            break;
        }
    }
    directories.sort();
    Ok((canonical.to_string_lossy().into_owned(), directories))
}

async fn handle_request(
    state: State,
    tx: Tx,
    request: ClientRequest,
    provider_env: BTreeMap<String, String>,
) {
    let id = request.id.clone();
    if let Err(e) = handle_inner(state, tx.clone(), request, provider_env).await {
        let _ = tx.send(event(
            Some(id),
            ClientPayload::Error {
                code: "spoke_error".into(),
                message: e.to_string(),
            },
        ));
    }
}
async fn handle_inner(
    state: State,
    tx: Tx,
    request: ClientRequest,
    provider_env: BTreeMap<String, String>,
) -> Result<()> {
    let id = request.id.clone();
    match request.command {
        ClientCommand::ProjectList { .. } => {
            let projects = state
                .store
                .lock()
                .await
                .projects
                .values()
                .cloned()
                .collect();
            tx.send(event(Some(id), ClientPayload::ProjectList { projects }))?
        }
        ClientCommand::ProjectGet { project_id, .. } => {
            let snap = snapshot(&state, &project_id, None).await?;
            tx.send(event(
                Some(id),
                ClientPayload::ProjectSnapshot {
                    snapshot: Box::new(snap),
                },
            ))?
        }
        ClientCommand::ProjectPathList { path, .. } => {
            let (parent, directories) = list_project_directories(&state, &path).await?;
            tx.send(event(
                Some(id),
                ClientPayload::ProjectPathList {
                    spoke_id: state.config.spoke_id.clone(),
                    parent,
                    directories,
                },
            ))?
        }
        ClientCommand::IntentGetProjection { project_id, .. } => {
            let store = state.store.lock().await;
            let source = store
                .sources
                .get(&project_id)
                .context("source unavailable")?;
            let content = store.timelines.get(&project_id).and_then(|items| items.iter().rev().find(|item| matches!(item.kind, TimelineKind::AssistantProjection | TimelineKind::Outcome | TimelineKind::Question)).and_then(|item| item.content.clone().or(item.summary.clone()))).unwrap_or_else(|| "The initial project context has been inspected; intent is awaiting clarification.".into());
            tx.send(event(
                Some(id),
                ClientPayload::Projection {
                    spoke_id: state.config.spoke_id.clone(),
                    project_id,
                    revision: source.revision,
                    content,
                },
            ))?
        }
        ClientCommand::IntentSubscribe {
            project_id, cursor, ..
        } => {
            let snap = snapshot(&state, &project_id, cursor).await?;
            tx.send(event(
                Some(id),
                ClientPayload::ProjectSnapshot {
                    snapshot: Box::new(snap),
                },
            ))?
        }
        ClientCommand::ProjectInitialize { cwd, name, .. } => {
            initialize(&state, &tx, id, cwd, name).await?
        }
        ClientCommand::ProjectModelSet {
            project_id,
            provider,
            model,
            ..
        } => {
            let project = {
                let mut s = state.store.lock().await;
                let project = s
                    .projects
                    .get_mut(&project_id)
                    .context("project not found")?;
                project.default_provider = Some(provider);
                project.default_model = Some(model);
                project.updated_at = now();
                let result = project.clone();
                save_store(&state.data_dir, &s).await?;
                result
            };
            tx.send(event(Some(id), ClientPayload::ProjectUpdated { project }))?;
            send_inventory(&state, &tx).await?;
        }
        ClientCommand::ProjectRemove { project_id, .. } => {
            let projects = {
                let mut s = state.store.lock().await;
                s.projects.remove(&project_id);
                s.sources.remove(&project_id);
                s.chats.remove(&project_id);
                s.timelines.remove(&project_id);
                s.operations
                    .retain(|_, operation| operation.project_id != project_id);
                s.sessions
                    .retain(|_, session| session.project_id != project_id);
                s.reviews
                    .retain(|_, review| review.project_id != project_id);
                s.workspaces
                    .retain(|_, workspace| workspace.project_id != project_id);
                let remaining_workspaces = s.workspaces.keys().cloned().collect::<BTreeSet<_>>();
                s.realizations
                    .retain(|operation_id, _| remaining_workspaces.contains(operation_id));
                save_store(&state.data_dir, &s).await?;
                s.projects.values().cloned().collect()
            };
            tx.send(event(Some(id), ClientPayload::ProjectList { projects }))?;
            send_inventory(&state, &tx).await?
        }
        ClientCommand::IntentSend {
            project_id,
            message,
            expected_revision,
            ..
        } => {
            accept_intent(
                state,
                tx,
                id,
                project_id,
                message,
                expected_revision,
                provider_env,
            )
            .await?
        }
        ClientCommand::IntentAnswer {
            operation_id,
            request_id,
            response,
            ..
        } => {
            let sender = state
                .interactions
                .lock()
                .await
                .remove(&(operation_id.clone(), request_id))
                .context("interaction request is stale or already answered")?;
            sender
                .send(response)
                .map_err(|_| anyhow!("interaction is no longer waiting"))?;
            let operation = state
                .store
                .lock()
                .await
                .operations
                .get(&operation_id)
                .cloned()
                .context("operation missing")?;
            tx.send(event(Some(id), ClientPayload::Operation { operation }))?;
        }
        ClientCommand::IntentCancel {
            project_id,
            operation_id,
            ..
        } => {
            if let Some(cancel) = state.cancellations.lock().await.get(&operation_id) {
                let _ = cancel.send(true);
            }
            update_operation(&state, &tx, &operation_id, OperationStatus::Cancelled, None).await?;
            set_realization_status(&state, &project_id, RealizationStatus::Paused).await?;
            let operation = state
                .store
                .lock()
                .await
                .operations
                .get(&operation_id)
                .cloned()
                .context("operation missing")?;
            tx.send(event(Some(id), ClientPayload::Operation { operation }))?;
        }
        _ => return Err(anyhow!("command is not valid at spoke")),
    }
    Ok(())
}

async fn initialize(
    state: &State,
    tx: &Tx,
    id: RequestId,
    cwd: String,
    name: Option<String>,
) -> Result<()> {
    let path = tokio::fs::canonicalize(&cwd)
        .await
        .context("project path does not exist")?;
    if !state
        .config
        .trusted_roots
        .iter()
        .any(|r| path.starts_with(r))
    {
        return Err(anyhow!("project is outside trusted roots"));
    }
    let pid = ProjectId(new_id("proj"));
    let sid = SourceOfIntentId(new_id("soi"));
    let cid = IntentChatId(new_id("chat"));
    let n = now();
    let context = inspect(&path).await;
    let authoritative_bundle = source_bundle::import(&path)?;
    let bundle_projection = source_bundle::manifest_for_prompt(authoritative_bundle.as_ref());
    let payload = format!(
        "# Project Intent\n\n## Authority\nThe attached content-addressed document bundle is canonical and must be implemented without lossy replacement.\n\n{bundle_projection}\n\n## Situated Context\n{context}\n\n## Conversational Amendments\nTo be clarified with the owner.\n\n## Constraints\n- Work only within {} unless explicitly authorized.\n",
        path.display()
    );
    let project = ProjectRecord {
        project_id: pid.clone(),
        spoke_id: state.config.spoke_id.clone(),
        name: name.unwrap_or_else(|| {
            path.file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("project")
                .into()
        }),
        cwd: path.to_string_lossy().into(),
        source_of_intent_id: sid.clone(),
        intent_chat_id: cid.clone(),
        initialization_status: InitializationStatus::Clarifying,
        default_provider: None,
        default_model: None,
        run_as_user: project_owner(&path),
        allow_root_sessions: false,
        status: ProjectStatus::Active,
        trusted: true,
        realization_status: RealizationStatus::Inactive,
        created_at: n,
        updated_at: n,
    };
    let source = SourceOfIntentRecord {
        source_of_intent_id: sid,
        spoke_id: state.config.spoke_id.clone(),
        project_id: pid.clone(),
        format: "markdown.v1".into(),
        revision: 0,
        content_hash: source_bundle::source_hash(&payload, authoritative_bundle.as_ref()),
        canonical_payload: payload,
        authoritative_bundle,
        status: SourceStatus::Assembling,
        created_at: n,
        updated_at: n,
    };
    let chat = IntentChatRecord {
        intent_chat_id: cid,
        spoke_id: state.config.spoke_id.clone(),
        project_id: pid.clone(),
        source_of_intent_revision: 0,
        status: IntentStatus::WaitingForUser,
        next_cursor: 1,
        created_at: n,
        updated_at: n,
        last_active_at: n,
    };
    {
        let mut s = state.store.lock().await;
        s.projects.insert(pid.clone(), project.clone());
        s.sources.insert(pid.clone(), source);
        s.chats.insert(pid.clone(), chat);
        append_item_locked(&mut s,&pid,None,TimelineKind::Question,Some("Initial context inspected".into()),Some("I inspected the project context. What should this project achieve first, and what constraints or validation requirements matter most?".into()),Some(OperationStatus::Blocked));
        save_store(&state.data_dir, &s).await?
    }
    tx.send(event(
        Some(id),
        ClientPayload::ProjectSnapshot {
            snapshot: Box::new(snapshot(state, &pid, None).await?),
        },
    ))?;
    tx.send(event(None, ClientPayload::ProjectUpdated { project }))?;
    send_inventory(state, tx).await?;
    Ok(())
}
async fn inspect(path: &Path) -> String {
    let mut found = Vec::new();
    for name in [
        "README.md",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "Makefile",
        "docs/design/README.md",
    ] {
        if path.join(name).exists() {
            found.push(name)
        }
    }
    let git = if path.join(".git").exists() {
        "Git repository"
    } else {
        "No .git directory"
    };
    format!(
        "- {git}\n- Detected files: {}",
        if found.is_empty() {
            "none of the standard manifests".into()
        } else {
            found.join(", ")
        }
    )
}

async fn accept_intent(
    state: State,
    tx: Tx,
    id: RequestId,
    pid: ProjectId,
    message: String,
    expected: Option<u64>,
    provider_env: BTreeMap<String, String>,
) -> Result<()> {
    let opid = OperationId(new_id("op"));
    let n = now();
    let op;
    {
        let mut s = state.store.lock().await;
        let chat = s.chats.get(&pid).context("project not found")?.clone();
        if expected.is_some_and(|r| r != chat.source_of_intent_revision) {
            return Err(anyhow!("source of intent revision conflict"));
        }
        op = OperationRecord {
            operation_id: opid.clone(),
            request_id: Some(id.clone()),
            spoke_id: state.config.spoke_id.clone(),
            project_id: pid.clone(),
            intent_chat_id: chat.intent_chat_id,
            source_of_intent_revision: Some(chat.source_of_intent_revision),
            kind: "intent.send".into(),
            status: OperationStatus::Accepted,
            error: None,
            created_at: n,
            updated_at: n,
            completed_at: None,
        };
        s.operations.insert(opid.clone(), op.clone());
        let item = append_item_locked(
            &mut s,
            &pid,
            Some(opid.clone()),
            TimelineKind::UserIntent,
            None,
            Some(message.clone()),
            Some(OperationStatus::Accepted),
        );
        save_store(&state.data_dir, &s).await?;
        tx.send(event(None, ClientPayload::Timeline { item }))?
    }
    tx.send(event(Some(id), ClientPayload::Accepted { operation: op }))?;
    let (cancel_tx, cancel_rx) = watch::channel(false);
    state
        .cancellations
        .lock()
        .await
        .insert(opid.clone(), cancel_tx);
    tokio::spawn(async move {
        if let Err(e) = run_operation(
            state.clone(),
            tx.clone(),
            pid.clone(),
            opid.clone(),
            message,
            cancel_rx,
            provider_env,
        )
        .await
        {
            let cancelled = state
                .store
                .lock()
                .await
                .operations
                .get(&opid)
                .is_some_and(|operation| operation.status == OperationStatus::Cancelled);
            if !cancelled {
                let _ = set_realization_status(&state, &pid, RealizationStatus::Blocked).await;
                let _ = update_operation(
                    &state,
                    &tx,
                    &opid,
                    OperationStatus::Failed,
                    Some(e.to_string()),
                )
                .await;
            }
        }
        state.cancellations.lock().await.remove(&opid);
    });
    Ok(())
}
async fn run_operation(
    state: State,
    tx: Tx,
    pid: ProjectId,
    opid: OperationId,
    message: String,
    cancel: watch::Receiver<bool>,
    provider_env: BTreeMap<String, String>,
) -> Result<()> {
    update_operation(&state, &tx, &opid, OperationStatus::Running, None).await?;

    // Canonical intent updates serialize, but the lock is released before realization so a new
    // owner turn can supersede an active revision.
    let intent_lane = {
        let mut lanes = state.project_lanes.lock().await;
        lanes
            .entry(pid.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let intent_guard = intent_lane.lock().await;
    let (project, source) = {
        let s = state.store.lock().await;
        (
            s.projects.get(&pid).context("project missing")?.clone(),
            s.sources.get(&pid).context("source missing")?.clone(),
        )
    };
    let observed_bundle = source_bundle::import(Path::new(&project.cwd))?;
    let bundle_changed = source
        .authoritative_bundle
        .as_ref()
        .map(|bundle| &bundle.bundle_hash)
        != observed_bundle.as_ref().map(|bundle| &bundle.bundle_hash);
    let before_intent = project_fingerprint(PathBuf::from(&project.cwd)).await?;
    let recovering = message == INTERNAL_RESUME_MESSAGE;
    let authoritative_manifest = format!(
        "CANONICAL BUNDLE:\n{}\n\nCURRENT ON-DISK MANIFEST{}:\n{}",
        source_bundle::manifest_for_prompt(source.authoritative_bundle.as_ref()),
        if bundle_changed {
            " (CHANGED; adopt only when explicitly directed by owner)"
        } else {
            ""
        },
        source_bundle::manifest_for_prompt(observed_bundle.as_ref()),
    );
    let proposal = if recovering {
        IntentTurnProposal {
            acts: vec![IntentAct::Resume],
            source_coverage: source
                .authoritative_bundle
                .as_ref()
                .map(source_bundle::coverage)
                .unwrap_or_default(),
            projection: format!(
                "Resuming durable realization of Source of Intent revision {} after Spoke restart.",
                source.revision
            ),
            question: None,
            source_update: None,
            assumptions: Vec::new(),
        }
    } else {
        let prompt = orchestrator::intent_prompt(
            source.revision,
            &source.canonical_payload,
            &authoritative_manifest,
            &message,
            source.status == SourceStatus::Assembling,
        );
        let (_, raw) = run_internal(
            &state,
            &tx,
            &project,
            &opid,
            SessionPurpose::Intent,
            Some(source.revision),
            &prompt,
            cancel.clone(),
            &provider_env,
        )
        .await?;
        orchestrator::parse_intent_proposal(&raw, source.revision)?
    };
    let after_intent = project_fingerprint(PathBuf::from(&project.cwd)).await?;
    if after_intent != before_intent {
        if let Some(bundle) = &observed_bundle {
            source_bundle::restore(Path::new(&project.cwd), bundle)?;
        }
        return Err(anyhow!(
            "Intent Agent mutated Project reality; proposal rejected and authoritative documents restored"
        ));
    }
    let proposed_bundle = match &proposal.source_update {
        Some(update) if update.refresh_authoritative_bundle => {
            if !proposal.acts.contains(&IntentAct::ReferenceContext) {
                return Err(anyhow!(
                    "authoritative bundle refresh requires a reference_context intent act"
                ));
            }
            observed_bundle.clone()
        }
        _ => {
            if bundle_changed {
                return Err(anyhow!(
                    "authoritative source documents changed; owner must explicitly adopt the current manifest"
                ));
            }
            source.authoritative_bundle.clone()
        }
    };
    source_bundle::validate_coverage(proposed_bundle.as_ref(), &proposal.source_coverage)?;

    let paused = proposal
        .acts
        .iter()
        .any(|act| matches!(act, IntentAct::Pause | IntentAct::Cancel));
    let mut active = source.status == SourceStatus::Active;
    let mut revision = source.revision;
    if let Some(update) = &proposal.source_update {
        let mut superseded = Vec::new();
        let item = {
            let mut s = state.store.lock().await;
            let src = s.sources.get_mut(&pid).context("source missing")?;
            if src.revision != update.base_revision {
                return Err(anyhow!("source changed while intent proposal was running"));
            }
            src.revision += 1;
            src.canonical_payload = update.canonical_payload.clone();
            src.authoritative_bundle = proposed_bundle.clone();
            src.content_hash = source_bundle::source_hash(
                &src.canonical_payload,
                src.authoritative_bundle.as_ref(),
            );
            src.status = if update.activate {
                SourceStatus::Active
            } else {
                SourceStatus::Assembling
            };
            src.updated_at = now();
            revision = src.revision;
            active = update.activate;
            let chat = s.chats.get_mut(&pid).context("chat missing")?;
            chat.source_of_intent_revision = revision;
            chat.status = if active {
                IntentStatus::Working
            } else {
                IntentStatus::WaitingForUser
            };
            if let Some(operation) = s.operations.get_mut(&opid) {
                operation.source_of_intent_revision = Some(revision);
                operation.updated_at = now();
            }
            if let Some(project) = s.projects.get_mut(&pid) {
                project.initialization_status = if active {
                    InitializationStatus::Ready
                } else {
                    InitializationStatus::Clarifying
                };
                project.realization_status = if active {
                    RealizationStatus::Reconciling
                } else {
                    RealizationStatus::Inactive
                };
                project.updated_at = now();
            }
            for operation in s.operations.values_mut().filter(|operation| {
                operation.project_id == pid
                    && operation.operation_id != opid
                    && matches!(
                        operation.status,
                        OperationStatus::Accepted | OperationStatus::Running
                    )
            }) {
                operation.status = OperationStatus::Cancelled;
                operation.error = Some(format!(
                    "superseded by Source of Intent revision {revision}"
                ));
                operation.updated_at = now();
                operation.completed_at = Some(now());
                superseded.push(operation.operation_id.clone());
            }
            let item = append_item_locked(
                &mut s,
                &pid,
                Some(opid.clone()),
                TimelineKind::IntentUpdate,
                Some(format!("Source of Intent committed as revision {revision}")),
                Some(if active {
                    "Intent is active; iterative realization and independent review will continue until no findings remain."
                } else {
                    "Intent remains under clarification; Project mutation has not started."
                }
                .into()),
                Some(OperationStatus::Completed),
            );
            save_store(&state.data_dir, &s).await?;
            item
        };
        for old in superseded {
            if let Some(sender) = state.cancellations.lock().await.get(&old) {
                let _ = sender.send(true);
            }
        }
        tx.send(event(None, ClientPayload::Timeline { item }))?;
    }

    let kind = if proposal.question.is_some() {
        TimelineKind::Question
    } else {
        TimelineKind::AssistantProjection
    };
    let content = proposal
        .question
        .clone()
        .unwrap_or_else(|| proposal.projection.clone());
    let item = append_item(
        &state,
        &pid,
        Some(opid.clone()),
        kind,
        None,
        Some(content),
        if proposal.question.is_some() {
            Some(OperationStatus::Blocked)
        } else {
            Some(OperationStatus::Completed)
        },
    )
    .await?;
    tx.send(event(None, ClientPayload::Timeline { item }))?;
    drop(intent_guard);

    if let Some(question) = proposal.question {
        set_realization_status(&state, &pid, RealizationStatus::WaitingForUser).await?;
        let _ = question;
        return update_operation(&state, &tx, &opid, OperationStatus::Blocked, None).await;
    }
    if paused {
        cancel_project_operations(&state, &pid, &opid, "paused or cancelled by owner").await?;
        set_realization_status(&state, &pid, RealizationStatus::Paused).await?;
        return update_operation(&state, &tx, &opid, OperationStatus::Completed, None).await;
    }
    if !active {
        return update_operation(&state, &tx, &opid, OperationStatus::Blocked, None).await;
    }

    let realization_lane = {
        let mut lanes = state.realization_lanes.lock().await;
        lanes
            .entry(pid.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _realization_guard = realization_lane.lock().await;
    let mut isolated = if let Some(record) = state.store.lock().await.workspaces.get(&opid).cloned()
    {
        workspace::verify(&record).await?;
        record
    } else {
        let record =
            workspace::prepare(&state.data_dir, &pid, &opid, Path::new(&project.cwd)).await?;
        persist_workspace(&state, record.clone()).await?;
        record
    };
    let mut execution_project = project.clone();
    execution_project.cwd = isolated.execution_cwd.to_string_lossy().into_owned();
    let mut machine = state
        .store
        .lock()
        .await
        .realizations
        .get(&opid)
        .cloned()
        .filter(|machine| machine.revision == revision)
        .unwrap_or_else(|| orchestrator::RealizationMachine::start(revision));
    persist_realization(&state, &opid, machine.clone()).await?;

    loop {
        if *cancel.borrow() {
            return Err(anyhow!("operation cancelled"));
        }
        let current_source = state
            .store
            .lock()
            .await
            .sources
            .get(&pid)
            .context("source missing")?
            .clone();
        if current_source.revision != machine.revision
            || current_source.status != SourceStatus::Active
        {
            return Err(anyhow!(
                "realization became stale at intent revision {}",
                machine.revision
            ));
        }

        if let Some(bundle) = &current_source.authoritative_bundle {
            source_bundle::verify_on_disk(Path::new(&execution_project.cwd), bundle)?;
        }
        let authoritative_manifest =
            source_bundle::manifest_for_prompt(current_source.authoritative_bundle.as_ref());
        set_realization_status(&state, &pid, RealizationStatus::Reconciling).await?;
        let progress = append_item(
            &state,
            &pid,
            Some(opid.clone()),
            TimelineKind::Progress,
            Some(format!(
                "Realization iteration {} started",
                machine.iteration
            )),
            machine
                .findings
                .first()
                .map(|finding| finding.fault.clone()),
            Some(OperationStatus::Running),
        )
        .await?;
        tx.send(event(None, ClientPayload::Timeline { item: progress }))?;

        let prompt = orchestrator::implementation_prompt(
            machine.revision,
            &current_source.canonical_payload,
            machine.iteration,
            &machine.findings,
            &authoritative_manifest,
        );
        let (_, raw) = match run_internal(
            &state,
            &tx,
            &execution_project,
            &opid,
            SessionPurpose::Implementation,
            Some(machine.revision),
            &prompt,
            cancel.clone(),
            &provider_env,
        )
        .await
        {
            Ok(output) => output,
            Err(error) => {
                workspace::rollback(&isolated).await?;
                return Err(error);
            }
        };
        let implementation: ImplementationRunResult = match serde_json::from_str(raw.trim()) {
            Ok(result) => result,
            Err(error) => {
                workspace::rollback(&isolated).await?;
                return Err(anyhow!(
                    "implementation Run violated its typed contract: {error}"
                ));
            }
        };
        if let Err(error) = source_bundle::validate_coverage(
            current_source.authoritative_bundle.as_ref(),
            &implementation.source_coverage,
        ) {
            workspace::rollback(&isolated).await?;
            return Err(error);
        }
        if let Some(bundle) = &current_source.authoritative_bundle
            && let Err(error) =
                source_bundle::verify_on_disk(Path::new(&execution_project.cwd), bundle)
        {
            source_bundle::restore(Path::new(&execution_project.cwd), bundle)?;
            workspace::rollback(&isolated).await?;
            return Err(anyhow!(
                "implementation modified authoritative Source of Intent documents; isolated changes were rolled back: {error}"
            ));
        }
        machine.implementation_completed(&implementation)?;
        workspace::checkpoint(&mut isolated, machine.iteration).await?;
        persist_workspace(&state, isolated.clone()).await?;
        persist_realization(&state, &opid, machine.clone()).await?;

        let outcome = append_item(
            &state,
            &pid,
            Some(opid.clone()),
            TimelineKind::Outcome,
            Some(format!(
                "Iteration {} implemented: {}",
                machine.iteration, implementation.objective
            )),
            Some(format!(
                "{}\n\nValidation:\n{}\n\nEvidence:\n{}",
                implementation.summary,
                implementation.validation.join("\n"),
                implementation.evidence.join("\n")
            )),
            Some(OperationStatus::Running),
        )
        .await?;
        tx.send(event(None, ClientPayload::Timeline { item: outcome }))?;

        if let Some(question) = &implementation.question {
            let item = append_item(
                &state,
                &pid,
                Some(opid.clone()),
                TimelineKind::Question,
                Some("Realization requires an owner decision".into()),
                Some(question.clone()),
                Some(OperationStatus::Blocked),
            )
            .await?;
            tx.send(event(None, ClientPayload::Timeline { item }))?;
            persist_realization(&state, &opid, machine.clone()).await?;
            set_realization_status(&state, &pid, RealizationStatus::WaitingForUser).await?;
            return update_operation(&state, &tx, &opid, OperationStatus::Blocked, None).await;
        }

        set_realization_status(&state, &pid, RealizationStatus::Reviewing).await?;
        let before_review = project_fingerprint(PathBuf::from(&execution_project.cwd)).await?;
        let prompt = orchestrator::review_prompt(
            machine.revision,
            &current_source.canonical_payload,
            &implementation,
            &authoritative_manifest,
        );
        let (review_run_id, raw) = run_internal(
            &state,
            &tx,
            &execution_project,
            &opid,
            SessionPurpose::Review,
            Some(machine.revision),
            &prompt,
            cancel.clone(),
            &provider_env,
        )
        .await?;
        if let Some(bundle) = &current_source.authoritative_bundle
            && source_bundle::verify_on_disk(Path::new(&execution_project.cwd), bundle).is_err()
        {
            source_bundle::restore(Path::new(&execution_project.cwd), bundle)?;
            workspace::rollback(&isolated).await?;
            return Err(anyhow!(
                "reviewer modified authoritative Source of Intent documents; isolated changes were rolled back"
            ));
        }
        let after_review = project_fingerprint(PathBuf::from(&execution_project.cwd)).await?;
        if after_review != before_review {
            workspace::rollback(&isolated).await?;
            return Err(anyhow!(
                "independent reviewer mutated isolated Project reality; changes were rolled back"
            ));
        }
        let review: ReviewRunResult =
            serde_json::from_str(raw.trim()).context("review Run violated its typed contract")?;
        review.validate().map_err(anyhow::Error::msg)?;
        source_bundle::validate_coverage(
            current_source.authoritative_bundle.as_ref(),
            &review.source_coverage,
        )?;
        let record = persist_review(
            &state,
            &pid,
            review_run_id,
            machine.revision,
            Path::new(&execution_project.cwd),
            &before_review,
            &review,
        )
        .await?;
        machine.review_completed(review)?;
        persist_realization(&state, &opid, machine.clone()).await?;

        if record.verdict == ReviewVerdict::Approved {
            workspace::promote(&mut isolated).await?;
            persist_workspace(&state, isolated.clone()).await?;
            set_realization_status(&state, &pid, RealizationStatus::Satisfied).await?;
            let item = append_item(
                &state,
                &pid,
                Some(opid.clone()),
                TimelineKind::Evidence,
                Some(format!("Source of Intent revision {} approved", machine.revision)),
                Some(format!(
                    "Independent whole-Project review found no fault. Reviewed scope:\n{}\n\nChecks:\n{}",
                    record.reviewed_scope.join("\n"),
                    record.checks.join("\n")
                )),
                Some(OperationStatus::Completed),
            )
            .await?;
            tx.send(event(None, ClientPayload::Timeline { item }))?;
            return update_operation(&state, &tx, &opid, OperationStatus::Completed, None).await;
        }

        let content = record
            .findings
            .iter()
            .map(|finding| {
                format!(
                    "- {}: {} ({})",
                    finding.requirement,
                    finding.fault,
                    finding.evidence.join("; ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let item = append_item(
            &state,
            &pid,
            Some(opid.clone()),
            TimelineKind::Evidence,
            Some(format!(
                "Review found {} fault(s); continuing",
                record.findings.len()
            )),
            Some(content),
            Some(OperationStatus::Running),
        )
        .await?;
        tx.send(event(None, ClientPayload::Timeline { item }))?;
    }
}

async fn persist_realization(
    state: &State,
    operation_id: &OperationId,
    machine: orchestrator::RealizationMachine,
) -> Result<()> {
    let mut store = state.store.lock().await;
    store.realizations.insert(operation_id.clone(), machine);
    save_store(&state.data_dir, &store).await
}

async fn persist_workspace(state: &State, record: workspace::WorkspaceRecord) -> Result<()> {
    let mut store = state.store.lock().await;
    store.workspaces.insert(record.operation_id.clone(), record);
    save_store(&state.data_dir, &store).await
}

async fn cancel_project_operations(
    state: &State,
    project_id: &ProjectId,
    except: &OperationId,
    reason: &str,
) -> Result<()> {
    let cancelled = {
        let mut store = state.store.lock().await;
        let mut cancelled = Vec::new();
        for operation in store.operations.values_mut().filter(|operation| {
            &operation.project_id == project_id
                && &operation.operation_id != except
                && matches!(
                    operation.status,
                    OperationStatus::Accepted | OperationStatus::Running | OperationStatus::Blocked
                )
        }) {
            operation.status = OperationStatus::Cancelled;
            operation.error = Some(reason.into());
            operation.updated_at = now();
            operation.completed_at = Some(now());
            cancelled.push(operation.operation_id.clone());
        }
        save_store(&state.data_dir, &store).await?;
        cancelled
    };
    let cancellations = state.cancellations.lock().await;
    for operation_id in cancelled {
        if let Some(sender) = cancellations.get(&operation_id) {
            let _ = sender.send(true);
        }
    }
    Ok(())
}

async fn set_realization_status(
    state: &State,
    project_id: &ProjectId,
    status: RealizationStatus,
) -> Result<()> {
    let mut store = state.store.lock().await;
    let project = store
        .projects
        .get_mut(project_id)
        .context("project missing")?;
    project.realization_status = status.clone();
    project.updated_at = now();
    if let Some(chat) = store.chats.get_mut(project_id) {
        chat.status = match status {
            RealizationStatus::Reconciling | RealizationStatus::Reviewing => IntentStatus::Working,
            RealizationStatus::WaitingForUser => IntentStatus::WaitingForUser,
            RealizationStatus::Blocked => IntentStatus::Blocked,
            RealizationStatus::Stale => IntentStatus::Stale,
            RealizationStatus::Inactive
            | RealizationStatus::Paused
            | RealizationStatus::Satisfied => IntentStatus::Ready,
        };
        chat.updated_at = now();
    }
    save_store(&state.data_dir, &store).await
}

async fn persist_review(
    state: &State,
    project_id: &ProjectId,
    run_id: RunId,
    revision: u64,
    observed_cwd: &Path,
    expected_fingerprint: &str,
    result: &ReviewRunResult,
) -> Result<ReviewRecord> {
    result.validate().map_err(anyhow::Error::msg)?;
    let observed_content_hash = project_fingerprint(observed_cwd.to_path_buf()).await?;
    if observed_content_hash != expected_fingerprint {
        return Err(anyhow!(
            "independent reviewer mutated Project reality; review rejected"
        ));
    }
    let record = ReviewRecord {
        review_id: ReviewId(new_id("review")),
        spoke_id: state.config.spoke_id.clone(),
        project_id: project_id.clone(),
        run_id,
        source_of_intent_revision: revision,
        observed_content_hash,
        reviewed_scope: result.reviewed_scope.clone(),
        checks: result.checks.clone(),
        findings: result
            .findings
            .iter()
            .map(|finding| ReviewFinding {
                finding_id: FindingId(new_id("finding")),
                requirement: finding.requirement.clone(),
                fault: finding.fault.clone(),
                evidence: finding.evidence.clone(),
                suggested_next_objective: finding.suggested_next_objective.clone(),
            })
            .collect(),
        unreviewed_required_scope: result.unreviewed_required_scope.clone(),
        verdict: result.verdict.clone(),
        created_at: now(),
    };
    let mut store = state.store.lock().await;
    let source = store.sources.get(project_id).context("source missing")?;
    if source.revision != revision || source.status != SourceStatus::Active {
        return Err(anyhow!(
            "review is stale for Source of Intent revision {revision}"
        ));
    }
    store
        .reviews
        .insert(record.review_id.clone(), record.clone());
    save_store(&state.data_dir, &store).await?;
    Ok(record)
}

async fn project_fingerprint(root: PathBuf) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        fn visit(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                let name = entry.file_name();
                if file_type.is_dir() {
                    if name == ".git" || name == "target" {
                        continue;
                    }
                    visit(root, &entry.path(), files)?;
                } else if file_type.is_file() {
                    let full_path = entry.path();
                    files.push(
                        full_path
                            .strip_prefix(root)
                            .unwrap_or(&full_path)
                            .to_path_buf(),
                    );
                }
            }
            Ok(())
        }
        let mut files = Vec::new();
        visit(&root, &root, &mut files)?;
        files.sort();
        let mut digest = Sha256::new();
        for relative in files {
            digest.update(relative.to_string_lossy().as_bytes());
            digest.update([0]);
            digest.update(std::fs::read(root.join(&relative))?);
            digest.update([0]);
        }
        Ok::<_, std::io::Error>(hex::encode(digest.finalize()))
    })
    .await
    .context("project fingerprint task failed")?
    .context("project fingerprint failed")
}

#[allow(clippy::too_many_arguments)]
async fn run_internal(
    state: &State,
    tx: &Tx,
    project: &ProjectRecord,
    operation: &OperationId,
    purpose: SessionPurpose,
    revision: Option<u64>,
    prompt: &str,
    cancel: watch::Receiver<bool>,
    provider_env: &BTreeMap<String, String>,
) -> Result<(RunId, String)> {
    let id = SessionId(new_id("sess"));
    let run_id = RunId(new_id("run"));
    let n = now();
    {
        let mut s = state.store.lock().await;
        s.sessions.insert(
            id.clone(),
            SessionRecord {
                session_id: id.clone(),
                run_id: Some(run_id.clone()),
                spoke_id: project.spoke_id.clone(),
                project_id: project.project_id.clone(),
                purpose: purpose.clone(),
                source_of_intent_revision: revision,
                parent_operation_id: Some(operation.clone()),
                status: SessionStatus::Running,
                run_as_user: project.run_as_user.clone(),
                run_as_root: false,
                pi_session_file: None,
                created_at: n,
                updated_at: n,
            },
        );
        save_store(&state.data_dir, &s).await?
    }
    let result = run_pi(
        state,
        tx,
        operation,
        project,
        purpose,
        prompt,
        cancel,
        provider_env,
    )
    .await;
    {
        let mut s = state.store.lock().await;
        if let Some(session) = s.sessions.get_mut(&id) {
            session.status = if result.is_ok() {
                SessionStatus::Stopped
            } else {
                SessionStatus::Crashed
            };
            session.updated_at = now()
        }
        save_store(&state.data_dir, &s).await?
    }
    result.map(|text| (run_id, text))
}
#[allow(clippy::too_many_arguments)]
async fn run_pi(
    state: &State,
    tx: &Tx,
    operation: &OperationId,
    project: &ProjectRecord,
    purpose: SessionPurpose,
    prompt: &str,
    mut cancel: watch::Receiver<bool>,
    provider_env: &BTreeMap<String, String>,
) -> Result<String> {
    let pi_binary = state.config.pi_binary.clone().unwrap_or_else(|| {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".local/share/mise/installs/pi/latest/pi/pi"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("pi"))
    });
    let isolated = state.store.lock().await.workspaces.get(operation).cloned();
    let mut cmd = Command::new("bwrap");
    cmd.args([
        "--die-with-parent",
        "--ro-bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--proc",
        "/proc",
        "--bind",
        "/tmp",
        "/tmp",
    ]);
    let sandbox_cwd = if let Some(workspace) = &isolated {
        let writable = matches!(
            purpose,
            SessionPurpose::Implementation
                | SessionPurpose::Validation
                | SessionPurpose::Review
                | SessionPurpose::Recovery
        );
        cmd.arg(if writable { "--bind" } else { "--ro-bind" })
            .arg(&workspace.worktree_root)
            .arg(&workspace.primary_root);
        workspace.primary_cwd.clone()
    } else {
        PathBuf::from(&project.cwd)
    };
    cmd.arg("--chdir")
        .arg(&sandbox_cwd)
        .arg("--")
        .arg(&pi_binary)
        .arg("--mode")
        .arg("rpc")
        .arg("--no-session")
        .current_dir("/")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(p) = &project.default_provider {
        cmd.arg("--provider").arg(p);
    }
    if let Some(m) = &project.default_model {
        cmd.arg("--model").arg(m);
    }
    for (name, value) in provider_env {
        cmd.env(name, value);
    }
    apply_identity(&mut cmd, project.run_as_user.as_deref())?;
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to start Pi {purpose:?} run"))?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut tail = std::collections::VecDeque::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if tail.len() == 64 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });
    stdin
        .write_all(
            format!(
                "{}\n",
                json!({"id":"prompt","type":"prompt","message":prompt})
            )
            .as_bytes(),
        )
        .await?;
    stdin.flush().await?;
    let mut lines = BufReader::new(stdout).lines();
    let mut final_text = String::new();
    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_ok() && *cancel.borrow() { let _ = child.kill().await; return Err(anyhow!("operation cancelled")); }
            }
            line = lines.next_line() => {
                let Some(line) = line? else { break };
                let v: serde_json::Value = match serde_json::from_str(line.trim_end_matches('\r')) { Ok(v) => v, Err(_) => continue };
                let event_type = v.get("type").and_then(|x| x.as_str()).unwrap_or_default();
                if event_type == "extension_ui_request" {
                    let request_id = v.get("id").and_then(|x| x.as_str()).context("extension UI request missing id")?.to_string();
                    let method = v.get("method").and_then(|x| x.as_str()).unwrap_or("notify").to_string();
                    if matches!(method.as_str(), "select" | "confirm" | "input" | "editor") {
                        let (answer_tx, answer_rx) = oneshot::channel();
                        state.interactions.lock().await.insert((operation.clone(), request_id.clone()), answer_tx);
                        update_operation(state, tx, operation, OperationStatus::Blocked, None).await?;
                        tx.send(event(None, ClientPayload::Interaction { spoke_id: project.spoke_id.clone(), project_id: project.project_id.clone(), operation_id: operation.clone(), request_id: request_id.clone(), method: method.clone(), payload: v.clone() }))?;
                        let response = tokio::select! {
                            answer = answer_rx => answer.context("interaction response channel closed")?,
                            changed = cancel.changed() => { let _ = changed; state.interactions.lock().await.remove(&(operation.clone(), request_id.clone())); let _ = child.kill().await; return Err(anyhow!("operation cancelled while awaiting interaction")); }
                        };
                        let mut command = response.as_object().cloned().context("interaction response must be a JSON object")?;
                        command.insert("type".into(), json!("extension_ui_response"));
                        command.insert("id".into(), json!(request_id));
                        stdin.write_all(format!("{}\n", serde_json::Value::Object(command)).as_bytes()).await?;
                        stdin.flush().await?;
                        update_operation(state, tx, operation, OperationStatus::Running, None).await?;
                    } else {
                        tx.send(event(None, ClientPayload::Interaction { spoke_id: project.spoke_id.clone(), project_id: project.project_id.clone(), operation_id: operation.clone(), request_id, method, payload: v.clone() }))?;
                    }
                }
                if event_type == "message_end" && let Some(t) = message_content(&v) { final_text = t }
                if event_type == "agent_settled" { break }
            }
        }
    }
    // RPC mode is persistent; this helper owns a one-operation process.
    let _ = child.kill().await;
    let status = child.wait().await?;
    let stderr_tail = stderr_task.await.unwrap_or_default();
    if !status.success() && final_text.is_empty() {
        return Err(anyhow!("Pi exited with {status}; stderr: {stderr_tail}"));
    }
    if final_text.is_empty() {
        return Err(anyhow!(
            "Pi produced no final assistant message; stderr: {stderr_tail}"
        ));
    }
    Ok(final_text)
}
fn message_content(v: &serde_json::Value) -> Option<String> {
    let msg = v.get("message")?;
    if let Some(s) = msg.get("content").and_then(|x| x.as_str()) {
        return Some(s.into());
    }
    let arr = msg.get("content")?.as_array()?;
    let text = arr
        .iter()
        .filter_map(|x| x.get("text").and_then(|x| x.as_str()))
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}
async fn update_operation(
    state: &State,
    tx: &Tx,
    id: &OperationId,
    status: OperationStatus,
    error: Option<String>,
) -> Result<()> {
    let op = {
        let mut s = state.store.lock().await;
        let op = s.operations.get_mut(id).context("operation missing")?;
        op.status = status.clone();
        op.error = error;
        op.updated_at = now();
        if matches!(
            status,
            OperationStatus::Completed
                | OperationStatus::Failed
                | OperationStatus::Cancelled
                | OperationStatus::Rejected
        ) {
            op.completed_at = Some(now())
        }
        let out = op.clone();
        save_store(&state.data_dir, &s).await?;
        out
    };
    tx.send(event(None, ClientPayload::Operation { operation: op }))?;
    Ok(())
}
async fn append_item(
    state: &State,
    pid: &ProjectId,
    op: Option<OperationId>,
    kind: TimelineKind,
    summary: Option<String>,
    content: Option<String>,
    status: Option<OperationStatus>,
) -> Result<TimelineItem> {
    let mut s = state.store.lock().await;
    let item = append_item_locked(&mut s, pid, op, kind, summary, content, status);
    save_store(&state.data_dir, &s).await?;
    Ok(item)
}
fn append_item_locked(
    s: &mut Store,
    pid: &ProjectId,
    op: Option<OperationId>,
    kind: TimelineKind,
    summary: Option<String>,
    content: Option<String>,
    status: Option<OperationStatus>,
) -> TimelineItem {
    let chat = s.chats.get_mut(pid).unwrap();
    let cursor = chat.next_cursor;
    chat.next_cursor += 1;
    chat.last_active_at = now();
    chat.updated_at = now();
    let item = TimelineItem {
        timeline_item_id: TimelineItemId(new_id("item")),
        spoke_id: chat.spoke_id.clone(),
        project_id: pid.clone(),
        intent_chat_id: chat.intent_chat_id.clone(),
        operation_id: op,
        session_id: None,
        run_id: None,
        source_of_intent_revision: Some(chat.source_of_intent_revision),
        kind,
        visibility: Visibility::Primary,
        status,
        summary,
        content,
        cursor,
        created_at: now(),
        updated_at: now(),
        completed_at: None,
    };
    s.timelines
        .entry(pid.clone())
        .or_default()
        .push(item.clone());
    item
}
async fn snapshot(state: &State, pid: &ProjectId, cursor: Option<u64>) -> Result<ProjectSnapshot> {
    let s = state.store.lock().await;
    let all = s.timelines.get(pid).cloned().unwrap_or_default();
    let available = all.first().map(|x| x.cursor).unwrap_or(1);
    let gap = cursor.filter(|c| *c + 1 < available);
    let timeline = all
        .into_iter()
        .filter(|x| cursor.is_none_or(|c| x.cursor > c))
        .collect();
    Ok(ProjectSnapshot {
        project: s.projects.get(pid).context("project not found")?.clone(),
        source: SourceOfIntentMetadata::from(s.sources.get(pid).context("source unavailable")?),
        chat: s.chats.get(pid).context("chat unavailable")?.clone(),
        timeline,
        operations: s
            .operations
            .values()
            .filter(|x| &x.project_id == pid)
            .cloned()
            .collect(),
        reviews: s
            .reviews
            .values()
            .filter(|x| &x.project_id == pid)
            .cloned()
            .collect(),
        gap_before: gap,
    })
}
#[cfg(unix)]
fn project_owner(path: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata(path).ok()?.uid();
    passwd().into_iter().find(|x| x.1 == uid).map(|x| x.0)
}
#[cfg(not(unix))]
fn project_owner(_: &Path) -> Option<String> {
    None
}
#[cfg(unix)]
fn passwd() -> Vec<(String, u32, u32, String)> {
    std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let p = line.split(':').collect::<Vec<_>>();
            Some((
                p.first()?.to_string(),
                p.get(2)?.parse().ok()?,
                p.get(3)?.parse().ok()?,
                p.get(5)?.to_string(),
            ))
        })
        .collect()
}
#[cfg(unix)]
fn apply_identity(cmd: &mut Command, user: Option<&str>) -> Result<()> {
    if let Some(name) = user {
        let (_, uid, gid, home) = passwd()
            .into_iter()
            .find(|x| x.0 == name)
            .with_context(|| format!("execution user {name} not found"))?;
        cmd.uid(uid)
            .gid(gid)
            .env("HOME", home)
            .env("USER", name)
            .env("LOGNAME", name);
    }
    Ok(())
}
#[cfg(not(unix))]
fn apply_identity(_: &mut Command, _: Option<&str>) -> Result<()> {
    Ok(())
}
fn reconcile_interrupted(store: &mut Store) -> bool {
    let active_sessions = store
        .sessions
        .values()
        .filter(|session| {
            matches!(
                session.status,
                SessionStatus::Starting | SessionStatus::Running | SessionStatus::Blocked
            )
        })
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();
    let affected = store
        .operations
        .values()
        .filter(|operation| {
            matches!(
                operation.status,
                OperationStatus::Accepted | OperationStatus::Running
            )
        })
        .map(|operation| (operation.operation_id.clone(), operation.project_id.clone()))
        .collect::<Vec<_>>();
    if affected.is_empty() && active_sessions.is_empty() {
        return false;
    }
    for session_id in active_sessions {
        if let Some(session) = store.sessions.get_mut(&session_id) {
            session.status = SessionStatus::Crashed;
            session.updated_at = now();
        }
    }
    for (operation_id, project_id) in affected {
        let resumable = store.realizations.contains_key(&operation_id)
            && store.workspaces.contains_key(&operation_id)
            && store
                .sources
                .get(&project_id)
                .is_some_and(|source| source.status == SourceStatus::Active);
        if resumable {
            if let Some(machine) = store.realizations.get_mut(&operation_id) {
                machine.phase = orchestrator::RealizationPhase::Implementing;
            }
            if let Some(operation) = store.operations.get_mut(&operation_id) {
                operation.status = OperationStatus::Queued;
                operation.error = Some(
                    "Spoke restarted; isolated workspace will roll back to its durable checkpoint and resume"
                        .into(),
                );
                operation.updated_at = now();
                operation.completed_at = None;
            }
            if let Some(project) = store.projects.get_mut(&project_id) {
                project.realization_status = RealizationStatus::Reconciling;
                project.updated_at = now();
            }
            if let Some(chat) = store.chats.get_mut(&project_id) {
                chat.status = IntentStatus::Working;
            }
            append_item_locked(
                store,
                &project_id,
                Some(operation_id),
                TimelineKind::Lifecycle,
                Some("Resuming after Spoke restart".into()),
                Some(
                    "Uncommitted isolated changes will be discarded; realization resumes from the last durable checkpoint."
                        .into(),
                ),
                Some(OperationStatus::Queued),
            );
        } else {
            if let Some(operation) = store.operations.get_mut(&operation_id) {
                operation.status = OperationStatus::Failed;
                operation.error = Some(
                    "Spoke restarted before realization had a durable isolated checkpoint".into(),
                );
                operation.updated_at = now();
                operation.completed_at = Some(now());
            }
            if let Some(project) = store.projects.get_mut(&project_id) {
                project.realization_status = RealizationStatus::Blocked;
                project.updated_at = now();
            }
            append_item_locked(
                store,
                &project_id,
                Some(operation_id),
                TimelineKind::Error,
                Some("Intent maintenance was interrupted before durable realization".into()),
                Some(
                    "The acknowledged message remains in the timeline and can be retried safely."
                        .into(),
                ),
                Some(OperationStatus::Failed),
            );
        }
    }
    true
}
async fn load_store(dir: &Path) -> Result<Store> {
    let p = dir.join("store-v3.json");
    if !p.exists() {
        return Ok(Store::default());
    }
    Ok(serde_json::from_slice(&tokio::fs::read(p).await?)?)
}
async fn save_store(dir: &Path, s: &Store) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let p = dir.join("store-v3.json");
    let tmp = dir.join("store-v3.json.tmp");
    write_secure(&tmp, &serde_json::to_vec_pretty(s)?).await?;
    tokio::fs::rename(tmp, p).await?;
    Ok(())
}
async fn write_secure(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        tokio::fs::create_dir_all(p).await?
    }
    tokio::fs::write(path, data).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?
    }
    Ok(())
}

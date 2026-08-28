use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::*;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Parser)]
#[command(name = "pumpkinpi-hub", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    #[arg(long, env = "PUMPKINPI_HUB_DATA")]
    data_dir: Option<PathBuf>,
}
#[derive(Subcommand)]
enum Cmd {
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080")]
        listen: String,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        public_url: String,
    },
    OwnerToken,
    Spoke {
        #[command(subcommand)]
        command: SpokeCommand,
    },
    Reset {
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Subcommand)]
enum SpokeCommand {
    Create { name: String },
    List,
    Disable { spoke_id: String },
    Revoke { spoke_id: String },
    IssueSetupKey { spoke_id: String },
}
#[derive(Clone, Serialize, Deserialize)]
struct SpokeAuth {
    record: SpokeRecord,
    public_key: Option<String>,
    setup_key_hash: Option<String>,
    setup_expires_at: Option<u64>,
    projects: BTreeMap<ProjectId, ProjectRecord>,
}
#[derive(Default, Serialize, Deserialize)]
struct Store {
    owner_token_hash: Option<String>,
    spokes: BTreeMap<SpokeId, SpokeAuth>,
    #[serde(default)]
    snapshots: Vec<ProjectSnapshot>,
    #[serde(default)]
    providers: Vec<StoredProvider>,
}
#[derive(Clone, Serialize, Deserialize)]
struct StoredProvider {
    account: ProviderAccount,
    nonce: String,
    ciphertext: String,
}
#[derive(Clone)]
struct App {
    store: Arc<Mutex<Store>>,
    spokes: Arc<Mutex<HashMap<SpokeId, mpsc::UnboundedSender<Message>>>>,
    clients: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    subs: Arc<Mutex<HashMap<String, BTreeSet<ProjectKey>>>>,
    pending: Arc<Mutex<HashMap<(SpokeId, RequestId), String>>>,
    data: PathBuf,
    public_url: String,
    master_key: [u8; 32],
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let dir = cli.data_dir.unwrap_or_else(default_dir);
    match cli.cmd {
        Cmd::Serve { listen, public_url } => serve(dir, listen, public_url).await,
        Cmd::OwnerToken => owner_token(&dir).await,
        Cmd::Spoke { command } => spoke_cli(&dir, command).await,
        Cmd::Reset { yes } => {
            if !yes {
                return Err(anyhow!("reset destroys prerelease state; pass --yes"));
            }
            if dir.exists() {
                tokio::fs::remove_dir_all(dir).await?
            }
            Ok(())
        }
    }
}
fn default_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| ".".into())
        .join(".local/state/pumpkinpi-hub-v3")
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn secret(prefix: &str) -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    format!("{prefix}_{}", BASE64.encode(b).replace(['+', '/', '='], ""))
}
fn hash(v: &str) -> String {
    hex::encode(Sha256::digest(v.as_bytes()))
}
async fn owner_token(dir: &Path) -> Result<()> {
    let mut s = load(dir).await?;
    let token = secret("ppc");
    s.owner_token_hash = Some(hash(&token));
    save(dir, &s).await?;
    println!("{token}");
    Ok(())
}
async fn spoke_cli(dir: &Path, command: SpokeCommand) -> Result<()> {
    let mut store = load(dir).await?;
    match command {
        SpokeCommand::Create { name } => {
            let (id, key) = create_spoke(&mut store, name);
            println!("spoke_id: {id}\nsetup_key: {key}");
        }
        SpokeCommand::List => println!(
            "{}",
            serde_json::to_string_pretty(
                &store.spokes.values().map(|s| &s.record).collect::<Vec<_>>()
            )?
        ),
        SpokeCommand::Disable { spoke_id } => {
            store
                .spokes
                .get_mut(&SpokeId(spoke_id))
                .context("spoke not found")?
                .record
                .status = SpokeStatus::Disabled;
        }
        SpokeCommand::Revoke { spoke_id } => {
            store
                .spokes
                .get_mut(&SpokeId(spoke_id))
                .context("spoke not found")?
                .record
                .status = SpokeStatus::Revoked;
        }
        SpokeCommand::IssueSetupKey { spoke_id } => {
            let spoke = store
                .spokes
                .get_mut(&SpokeId(spoke_id))
                .context("spoke not found")?;
            let key = secret("pps_setup");
            spoke.setup_key_hash = Some(hash(&key));
            spoke.setup_expires_at = Some(now() + 1800);
            println!("setup_key: {key}");
        }
    }
    save(dir, &store).await
}
fn create_spoke(s: &mut Store, name: String) -> (SpokeId, String) {
    let id = SpokeId(format!("spoke_{}", Uuid::new_v4().simple()));
    let key = secret("pps_setup");
    let n = now();
    s.spokes.insert(
        id.clone(),
        SpokeAuth {
            record: SpokeRecord {
                spoke_id: id.clone(),
                name,
                hostname: String::new(),
                version: String::new(),
                status: SpokeStatus::Offline,
                created_at: n,
                enrolled_at: None,
                last_seen_at: None,
            },
            public_key: None,
            setup_key_hash: Some(hash(&key)),
            setup_expires_at: Some(n + 1800),
            projects: BTreeMap::new(),
        },
    );
    (id, key)
}
async fn serve(data: PathBuf, listen: String, public_url: String) -> Result<()> {
    tokio::fs::create_dir_all(&data).await?;
    let master_key = load_master_key(&data).await?;
    let mut initial_store = load(&data).await?;
    for spoke in initial_store.spokes.values_mut() {
        if spoke.record.status == SpokeStatus::Online {
            spoke.record.status = SpokeStatus::Offline;
        }
    }
    save(&data, &initial_store).await?;
    let app = App {
        store: Arc::new(Mutex::new(initial_store)),
        spokes: Default::default(),
        clients: Default::default(),
        subs: Default::default(),
        pending: Default::default(),
        data,
        public_url,
        master_key,
    };
    let router = Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"ok":true,"protocol_version":PROTOCOL_VERSION})) }),
        )
        .route("/api/spokes/enroll", post(http_enroll))
        .route("/ws/spoke", get(spoke_ws))
        .route("/ws/client", get(client_ws))
        .with_state(app);
    let l = tokio::net::TcpListener::bind(&listen).await?;
    info!(%listen,"PumpkinPi personal Hub listening");
    axum::serve(l, router).await?;
    Ok(())
}
async fn http_enroll(State(app): State<App>, Json(body): Json<EnrollRequest>) -> impl IntoResponse {
    if let Err(error) = refresh_admin_state(&app).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(EnrollResponse {
                ok: false,
                spoke_id: None,
                hub_url: None,
                error: Some(error.to_string()),
            }),
        );
    }
    let mut s = app.store.lock().await;
    let found = s.spokes.iter_mut().find(|(_, v)| {
        v.setup_key_hash.as_deref() == Some(hash(&body.setup_key).as_str())
            && v.setup_expires_at.is_some_and(|x| x >= now())
    });
    let response = if let Some((id, v)) = found {
        v.public_key = Some(body.public_key);
        v.setup_key_hash = None;
        v.setup_expires_at = None;
        v.record.hostname = body.hostname;
        v.record.version = body.version;
        v.record.enrolled_at = Some(now());
        EnrollResponse {
            ok: true,
            spoke_id: Some(id.clone()),
            hub_url: Some(app.public_url.clone()),
            error: None,
        }
    } else {
        EnrollResponse {
            ok: false,
            spoke_id: None,
            hub_url: None,
            error: Some("invalid or expired setup key".into()),
        }
    };
    let _ = save(&app.data, &s).await;
    (
        if response.ok {
            StatusCode::OK
        } else {
            StatusCode::UNAUTHORIZED
        },
        Json(response),
    )
}
async fn spoke_ws(ws: WebSocketUpgrade, State(app): State<App>) -> impl IntoResponse {
    ws.on_upgrade(move |s| handle_spoke(s, app))
}
async fn client_ws(ws: WebSocketUpgrade, State(app): State<App>) -> impl IntoResponse {
    ws.on_upgrade(move |s| handle_client(s, app))
}

async fn handle_spoke(socket: WebSocket, app: App) {
    if let Err(e) = spoke_session(socket, app.clone()).await {
        warn!(error=%e,"spoke disconnected")
    }
}
async fn spoke_session(socket: WebSocket, app: App) -> Result<()> {
    let (mut sink, mut stream) = socket.split();
    let hello: SpokeToHub = parse_text(stream.next().await.context("missing hello")??)?;
    let SpokeToHub::Hello {
        protocol_version,
        spoke_id,
        version,
    } = hello
    else {
        return Err(anyhow!("first message must be hello"));
    };
    check_version(protocol_version)?;
    let nonce = secret("challenge");
    sink.send(Message::Text(
        json!({"type":"spoke_challenge","nonce":nonce})
            .to_string()
            .into(),
    ))
    .await?;
    let auth: SpokeToHub = parse_text(stream.next().await.context("missing auth")??)?;
    let SpokeToHub::Auth {
        spoke_id: auth_id,
        signature,
        ..
    } = auth
    else {
        return Err(anyhow!("missing auth"));
    };
    if auth_id != spoke_id {
        return Err(anyhow!("identity changed"));
    }
    {
        let mut store = app.store.lock().await;
        let rec = store.spokes.get_mut(&spoke_id).context("unknown spoke")?;
        let public = BASE64.decode(rec.public_key.as_deref().context("spoke is not enrolled")?)?;
        let key =
            VerifyingKey::from_bytes(&public.try_into().map_err(|_| anyhow!("bad public key"))?)?;
        key.verify(
            nonce.as_bytes(),
            &Signature::from_slice(&BASE64.decode(signature)?)?,
        )?;
        if matches!(
            rec.record.status,
            SpokeStatus::Disabled | SpokeStatus::Revoked
        ) {
            return Err(anyhow!("spoke disabled"));
        }
        rec.record.status = SpokeStatus::Online;
        rec.record.version = version;
        rec.record.last_seen_at = Some(now());
        save(&app.data, &store).await?
    }
    sink.send(Message::Text(
        json!({"type":"spoke_authenticated"}).to_string().into(),
    ))
    .await?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    app.spokes.lock().await.insert(spoke_id.clone(), tx);
    let writer = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            if sink.send(v).await.is_err() {
                break;
            }
        }
    });
    broadcast_spoke(&app, &spoke_id).await;
    while let Some(msg) = stream.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(error) => {
                warn!(%spoke_id, %error, "Spoke socket closed with an error");
                break;
            }
        };
        let msg: SpokeToHub = match parse_text(msg) {
            Ok(message) => message,
            Err(error) => {
                warn!(%spoke_id, %error, "Ignoring invalid Spoke frame");
                continue;
            }
        };
        match msg {
            SpokeToHub::Inventory {
                protocol_version,
                projects,
                ..
            } => {
                check_version(protocol_version)?;
                let mut s = app.store.lock().await;
                if let Some(v) = s.spokes.get_mut(&spoke_id) {
                    v.projects = projects
                        .into_iter()
                        .map(|p| (p.project_id.clone(), p))
                        .collect();
                    v.record.last_seen_at = Some(now())
                }
                save(&app.data, &s).await?
            }
            SpokeToHub::Heartbeat { protocol_version } => {
                check_version(protocol_version)?;
                if let Some(v) = app.store.lock().await.spokes.get_mut(&spoke_id) {
                    v.record.last_seen_at = Some(now())
                }
            }
            SpokeToHub::ClientEvent {
                protocol_version,
                event,
            } => {
                check_version(protocol_version)?;
                deliver(&app, &spoke_id, *event).await
            }
            _ => {}
        }
    }
    app.spokes.lock().await.remove(&spoke_id);
    {
        let mut s = app.store.lock().await;
        if let Some(v) = s.spokes.get_mut(&spoke_id) {
            if v.record.status == SpokeStatus::Online {
                v.record.status = SpokeStatus::Offline;
            }
            v.record.last_seen_at = Some(now());
        }
        let _ = save(&app.data, &s).await;
    }
    broadcast_spoke(&app, &spoke_id).await;
    writer.abort();
    Ok(())
}

async fn handle_client(socket: WebSocket, app: App) {
    let (mut sink, mut stream) = socket.split();
    let Some(Ok(first)) = stream.next().await else {
        return;
    };
    let hello: ClientHello = match parse_text(first) {
        Ok(v) => v,
        Err(_) => return,
    };
    let ClientHello::Auth {
        protocol_version,
        token,
    } = hello;
    if check_version(protocol_version).is_err()
        || app.store.lock().await.owner_token_hash.as_deref() != Some(hash(&token).as_str())
    {
        let _ = sink
            .send(text_event(ClientEvent {
                protocol_version: PROTOCOL_VERSION,
                id: None,
                created_at: now(),
                payload: ClientPayload::Error {
                    code: "authentication_failed".into(),
                    message: "invalid owner credential".into(),
                },
            }))
            .await;
        return;
    }
    let cid = Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();
    app.clients.lock().await.insert(cid.clone(), tx);
    let writer = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            if sink.send(v).await.is_err() {
                break;
            }
        }
    });
    send_client(
        &app,
        &cid,
        ClientEvent {
            protocol_version: PROTOCOL_VERSION,
            id: None,
            created_at: now(),
            payload: ClientPayload::Authenticated,
        },
    )
    .await;
    while let Some(Ok(msg)) = stream.next().await {
        let req: ClientRequest = match parse_text(msg) {
            Ok(v) => v,
            Err(e) => {
                send_error(&app, &cid, None, "invalid_request", &e.to_string()).await;
                continue;
            }
        };
        if let Err(e) = check_version(req.protocol_version) {
            send_error(&app, &cid, Some(req.id), "protocol_version", &e.to_string()).await;
            continue;
        }
        if let Err(e) = route_client(&app, &cid, req).await {
            send_error(&app, &cid, None, "request_failed", &e.to_string()).await
        }
    }
    app.clients.lock().await.remove(&cid);
    app.subs.lock().await.remove(&cid);
    app.pending.lock().await.retain(|_, v| v != &cid);
    writer.abort()
}
async fn route_client(app: &App, cid: &str, req: ClientRequest) -> Result<()> {
    refresh_admin_state(app).await?;
    match &req.command {
        ClientCommand::HubStatus => {
            send_client(
                app,
                cid,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: Some(req.id),
                    created_at: now(),
                    payload: ClientPayload::HubStatus {
                        version: env!("CARGO_PKG_VERSION").into(),
                    },
                },
            )
            .await
        }
        ClientCommand::SpokeList => {
            let spokes = app
                .store
                .lock()
                .await
                .spokes
                .values()
                .map(|x| x.record.clone())
                .collect();
            send_client(
                app,
                cid,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: Some(req.id),
                    created_at: now(),
                    payload: ClientPayload::SpokeList { spokes },
                },
            )
            .await
        }
        ClientCommand::ProjectList { spoke_id } => {
            let s = app.store.lock().await;
            let projects = s
                .spokes
                .iter()
                .filter(|(id, _)| spoke_id.as_ref().is_none_or(|x| x == *id))
                .flat_map(|(_, x)| x.projects.values().cloned())
                .collect();
            send_client(
                app,
                cid,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: Some(req.id),
                    created_at: now(),
                    payload: ClientPayload::ProjectList { projects },
                },
            )
            .await
        }
        ClientCommand::ProviderList => {
            let accounts = app
                .store
                .lock()
                .await
                .providers
                .iter()
                .filter(|p| p.account.revoked_at.is_none())
                .map(|p| p.account.clone())
                .collect();
            send_client(
                app,
                cid,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: Some(req.id),
                    created_at: now(),
                    payload: ClientPayload::ProviderList { accounts },
                },
            )
            .await
        }
        ClientCommand::ProviderSet {
            provider_id,
            label,
            api_key,
        } => {
            let stored =
                encrypt_provider(&app.master_key, provider_id.clone(), label.clone(), api_key)?;
            let mut store = app.store.lock().await;
            store
                .providers
                .retain(|p| p.account.provider_id != *provider_id || p.account.label != *label);
            store.providers.push(stored);
            let accounts = store
                .providers
                .iter()
                .filter(|p| p.account.revoked_at.is_none())
                .map(|p| p.account.clone())
                .collect();
            save(&app.data, &store).await?;
            drop(store);
            send_client(
                app,
                cid,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: Some(req.id),
                    created_at: now(),
                    payload: ClientPayload::ProviderList { accounts },
                },
            )
            .await
        }
        ClientCommand::ProviderRevoke {
            provider_account_id,
        } => {
            let mut store = app.store.lock().await;
            let provider = store
                .providers
                .iter_mut()
                .find(|p| p.account.provider_account_id == *provider_account_id)
                .context("provider account not found")?;
            provider.account.revoked_at = Some(now());
            let accounts = store
                .providers
                .iter()
                .filter(|p| p.account.revoked_at.is_none())
                .map(|p| p.account.clone())
                .collect();
            save(&app.data, &store).await?;
            drop(store);
            send_client(
                app,
                cid,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: Some(req.id),
                    created_at: now(),
                    payload: ClientPayload::ProviderList { accounts },
                },
            )
            .await
        }
        _ => {
            let spoke = req
                .command
                .spoke_id()
                .context("missing spoke route")?
                .clone();
            let project = project_id(&req.command);
            if let Some(pid) = project
                && matches!(
                    req.command,
                    ClientCommand::IntentSubscribe { .. }
                        | ClientCommand::IntentSend { .. }
                        | ClientCommand::ProjectInitialize { .. }
                )
            {
                app.subs
                    .lock()
                    .await
                    .entry(cid.into())
                    .or_default()
                    .insert(ProjectKey {
                        spoke_id: spoke.clone(),
                        project_id: pid,
                    });
            }
            let provider_env =
                provider_env_for_request(app, &spoke, project_id(&req.command).as_ref()).await?;
            let channel = app.spokes.lock().await.get(&spoke).cloned();
            if let Some(channel) = channel {
                app.pending
                    .lock()
                    .await
                    .insert((spoke, req.id.clone()), cid.into());
                channel.send(Message::Text(
                    serde_json::to_string(&HubToSpoke::Command {
                        request: req,
                        provider_env,
                    })?
                    .into(),
                ))?
            } else if let ClientCommand::IntentSubscribe { project_id, .. } = &req.command {
                let cached = app
                    .store
                    .lock()
                    .await
                    .snapshots
                    .iter()
                    .find(|s| s.project.project_id == *project_id && s.project.spoke_id == spoke)
                    .cloned()
                    .context("spoke is offline and no cached Intent Chat is available")?;
                send_client(
                    app,
                    cid,
                    ClientEvent {
                        protocol_version: PROTOCOL_VERSION,
                        id: Some(req.id),
                        created_at: now(),
                        payload: ClientPayload::ProjectSnapshot {
                            snapshot: Box::new(cached),
                        },
                    },
                )
                .await
            } else {
                return Err(anyhow!("spoke is offline"));
            }
        }
    }
    Ok(())
}
async fn refresh_admin_state(app: &App) -> Result<()> {
    let disk = load(&app.data).await?;
    let mut disconnect = Vec::new();
    {
        let mut live = app.store.lock().await;
        for (id, on_disk) in disk.spokes {
            if let Some(spoke) = live.spokes.get_mut(&id) {
                if matches!(
                    on_disk.record.status,
                    SpokeStatus::Disabled | SpokeStatus::Revoked
                ) && spoke.record.status != on_disk.record.status
                {
                    spoke.record.status = on_disk.record.status;
                    disconnect.push(id.clone());
                }
                if on_disk.setup_key_hash != spoke.setup_key_hash {
                    spoke.setup_key_hash = on_disk.setup_key_hash;
                    spoke.setup_expires_at = on_disk.setup_expires_at;
                }
            }
        }
        save(&app.data, &live).await?;
    }
    for id in disconnect {
        if let Some(tx) = app.spokes.lock().await.remove(&id) {
            let _ = tx.send(Message::Close(None));
        }
    }
    Ok(())
}
fn project_id(c: &ClientCommand) -> Option<ProjectId> {
    match c {
        ClientCommand::ProjectGet { project_id, .. }
        | ClientCommand::ProjectRemove { project_id, .. }
        | ClientCommand::ProjectModelSet { project_id, .. }
        | ClientCommand::IntentSubscribe { project_id, .. }
        | ClientCommand::IntentSend { project_id, .. }
        | ClientCommand::IntentCancel { project_id, .. }
        | ClientCommand::IntentAnswer { project_id, .. }
        | ClientCommand::IntentGetProjection { project_id, .. } => Some(project_id.clone()),
        _ => None,
    }
}
async fn deliver(app: &App, spoke: &SpokeId, event: ClientEvent) {
    cache_event(app, &event).await;
    if let Some(id) = &event.id
        && let Some(cid) = app
            .pending
            .lock()
            .await
            .remove(&(spoke.clone(), id.clone()))
    {
        if let ClientPayload::ProjectSnapshot { snapshot } = &event.payload {
            app.subs
                .lock()
                .await
                .entry(cid.clone())
                .or_default()
                .insert(ProjectKey {
                    spoke_id: spoke.clone(),
                    project_id: snapshot.project.project_id.clone(),
                });
            update_cached_project(app, spoke, &snapshot.project).await
        }
        send_client(app, &cid, event).await;
        return;
    }
    let pid = payload_project(&event.payload);
    if let Some(pid) = pid {
        if let ClientPayload::ProjectUpdated { project } = &event.payload {
            update_cached_project(app, spoke, project).await
        }
        let key = ProjectKey {
            spoke_id: spoke.clone(),
            project_id: pid,
        };
        let recipients = app
            .subs
            .lock()
            .await
            .iter()
            .filter_map(|(c, s)| s.contains(&key).then_some(c.clone()))
            .collect::<Vec<_>>();
        for c in recipients {
            send_client(app, &c, event.clone()).await
        }
    }
}
async fn cache_event(app: &App, event: &ClientEvent) {
    let mut store = app.store.lock().await;
    match &event.payload {
        ClientPayload::ProjectSnapshot { snapshot } => {
            if let Some(old) = store.snapshots.iter_mut().find(|s| {
                s.project.project_id == snapshot.project.project_id
                    && s.project.spoke_id == snapshot.project.spoke_id
            }) {
                old.project = snapshot.project.clone();
                old.source = snapshot.source.clone();
                old.chat = snapshot.chat.clone();
                for item in &snapshot.timeline {
                    if !old
                        .timeline
                        .iter()
                        .any(|x| x.timeline_item_id == item.timeline_item_id)
                    {
                        old.timeline.push(item.clone())
                    }
                }
                old.operations = snapshot.operations.clone();
            } else {
                store.snapshots.push((**snapshot).clone());
            }
        }
        ClientPayload::Timeline { item } => {
            if let Some(s) = store.snapshots.iter_mut().find(|s| {
                s.project.project_id == item.project_id && s.project.spoke_id == item.spoke_id
            }) && !s
                .timeline
                .iter()
                .any(|x| x.timeline_item_id == item.timeline_item_id)
            {
                s.timeline.push(item.clone())
            }
        }
        ClientPayload::Operation { operation } | ClientPayload::Accepted { operation } => {
            if let Some(s) = store.snapshots.iter_mut().find(|s| {
                s.project.project_id == operation.project_id
                    && s.project.spoke_id == operation.spoke_id
            }) {
                if let Some(old) = s
                    .operations
                    .iter_mut()
                    .find(|x| x.operation_id == operation.operation_id)
                {
                    *old = operation.clone()
                } else {
                    s.operations.push(operation.clone())
                }
            }
        }
        ClientPayload::ProjectUpdated { project } => {
            if let Some(s) = store.snapshots.iter_mut().find(|s| {
                s.project.project_id == project.project_id && s.project.spoke_id == project.spoke_id
            }) {
                s.project = project.clone()
            }
        }
        _ => {}
    }
    let _ = save(&app.data, &store).await;
}
fn payload_project(p: &ClientPayload) -> Option<ProjectId> {
    match p {
        ClientPayload::Timeline { item } => Some(item.project_id.clone()),
        ClientPayload::Interaction { project_id, .. } => Some(project_id.clone()),
        ClientPayload::Operation { operation } | ClientPayload::Accepted { operation } => {
            Some(operation.project_id.clone())
        }
        ClientPayload::ProjectUpdated { project } => Some(project.project_id.clone()),
        ClientPayload::ProjectSnapshot { snapshot } => Some(snapshot.project.project_id.clone()),
        ClientPayload::ReplayGap { project_id, .. } => Some(project_id.clone()),
        _ => None,
    }
}
async fn update_cached_project(app: &App, spoke: &SpokeId, p: &ProjectRecord) {
    let mut s = app.store.lock().await;
    if let Some(v) = s.spokes.get_mut(spoke) {
        v.projects.insert(p.project_id.clone(), p.clone());
    }
    let _ = save(&app.data, &s).await;
}
async fn broadcast_spoke(app: &App, id: &SpokeId) {
    let spoke = app
        .store
        .lock()
        .await
        .spokes
        .get(id)
        .map(|x| x.record.clone());
    if let Some(spoke) = spoke {
        let clients = app.clients.lock().await.keys().cloned().collect::<Vec<_>>();
        for c in clients {
            send_client(
                app,
                &c,
                ClientEvent {
                    protocol_version: PROTOCOL_VERSION,
                    id: None,
                    created_at: now(),
                    payload: ClientPayload::SpokeUpdated {
                        spoke: spoke.clone(),
                    },
                },
            )
            .await
        }
    }
}
async fn send_error(app: &App, c: &str, id: Option<RequestId>, code: &str, msg: &str) {
    send_client(
        app,
        c,
        ClientEvent {
            protocol_version: PROTOCOL_VERSION,
            id,
            created_at: now(),
            payload: ClientPayload::Error {
                code: code.into(),
                message: msg.into(),
            },
        },
    )
    .await
}
async fn send_client(app: &App, c: &str, event: ClientEvent) {
    if let Some(tx) = app.clients.lock().await.get(c) {
        let _ = tx.send(text_event(event));
    }
}
fn text_event(e: ClientEvent) -> Message {
    Message::Text(serde_json::to_string(&e).unwrap().into())
}
fn parse_text<T: for<'a> Deserialize<'a>>(m: Message) -> Result<T> {
    let Message::Text(t) = m else {
        return Err(anyhow!("expected text"));
    };
    Ok(serde_json::from_str(&t)?)
}
fn check_version(v: u32) -> Result<()> {
    if v != PROTOCOL_VERSION {
        Err(anyhow!(
            "unsupported protocol {v}; expected {PROTOCOL_VERSION}"
        ))
    } else {
        Ok(())
    }
}
fn encrypt_provider(
    key: &[u8; 32],
    provider_id: String,
    label: String,
    api_key: &str,
) -> Result<StoredProvider> {
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), api_key.as_bytes())
        .map_err(|_| anyhow!("provider encryption failed"))?;
    Ok(StoredProvider {
        account: ProviderAccount {
            provider_account_id: format!("provider_{}", Uuid::new_v4().simple()),
            provider_id,
            label,
            created_at: now(),
            revoked_at: None,
        },
        nonce: BASE64.encode(nonce),
        ciphertext: BASE64.encode(ciphertext),
    })
}
fn decrypt_provider(key: &[u8; 32], provider: &StoredProvider) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = BASE64.decode(&provider.nonce)?;
    let ciphertext = BASE64.decode(&provider.ciphertext)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| anyhow!("provider decryption failed"))?;
    Ok(String::from_utf8(plaintext)?)
}
fn provider_env_name(provider: &str) -> Result<&'static str> {
    match provider {
        "anthropic" => Ok("ANTHROPIC_API_KEY"),
        "openai" => Ok("OPENAI_API_KEY"),
        "google" | "gemini" => Ok("GOOGLE_API_KEY"),
        "xai" => Ok("XAI_API_KEY"),
        "openrouter" => Ok("OPENROUTER_API_KEY"),
        _ => Err(anyhow!("unsupported provider {provider}")),
    }
}
async fn provider_env_for_request(
    app: &App,
    spoke: &SpokeId,
    project: Option<&ProjectId>,
) -> Result<BTreeMap<String, String>> {
    let Some(project) = project else {
        return Ok(BTreeMap::new());
    };
    let store = app.store.lock().await;
    let provider_id = store
        .spokes
        .get(spoke)
        .and_then(|s| s.projects.get(project))
        .and_then(|p| p.default_provider.as_deref());
    let Some(provider_id) = provider_id else {
        return Ok(BTreeMap::new());
    };
    let provider = store
        .providers
        .iter()
        .find(|p| p.account.provider_id == provider_id && p.account.revoked_at.is_none())
        .with_context(|| format!("no active credential for provider {provider_id}"))?;
    Ok(BTreeMap::from([(
        provider_env_name(provider_id)?.into(),
        decrypt_provider(&app.master_key, provider)?,
    )]))
}
async fn load_master_key(dir: &Path) -> Result<[u8; 32]> {
    let path = dir.join("master.key");
    if path.exists() {
        let bytes = BASE64.decode(tokio::fs::read_to_string(path).await?.trim())?;
        return bytes
            .try_into()
            .map_err(|_| anyhow!("invalid Hub master key"));
    }
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    tokio::fs::create_dir_all(dir).await?;
    tokio::fs::write(&path, BASE64.encode(key)).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?
    }
    Ok(key)
}
async fn load(dir: &Path) -> Result<Store> {
    let p = dir.join("hub-v3.json");
    if !p.exists() {
        return Ok(Store::default());
    }
    Ok(serde_json::from_slice(&tokio::fs::read(p).await?)?)
}
async fn save(dir: &Path, s: &Store) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let tmp = dir.join("hub-v3.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(s)?).await?;
    tokio::fs::rename(tmp, dir.join("hub-v3.json")).await?;
    Ok(())
}

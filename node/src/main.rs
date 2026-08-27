use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use clap::Parser;
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::{
    CommandPolicy, CrashInfo, PROTOCOL_VERSION, ProjectRecord, ProjectStatus, SessionRecord,
    SessionStatus, pi_command_policy,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::{fs::MetadataExt, process::ExitStatusExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command as TokioCommand,
    sync::{Mutex, mpsc},
    time::{self, Duration},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use uuid::Uuid;

mod cli;
mod config;
mod enrollment;
mod runtime;
mod store;

use cli::{Cli, Command};
use config::NodeConfig;
use enrollment::EnrollResponse;
use runtime::{ExtensionUiRequest, HubTx, RuntimeCommand, RuntimeHandle, RuntimeMap};
use store::NodeStore;

const VERSION: &str = env!("CARGO_PKG_VERSION");

type LocalSubscriptions = Arc<Mutex<HashMap<String, std::collections::HashSet<(String, String)>>>>;
type LocalClients = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<AxumMessage>>>>;

#[derive(Clone)]
struct LocalState {
    data_dir: PathBuf,
    config: NodeConfig,
    runtimes: RuntimeMap,
    hub_tx: HubTx,
    clients: LocalClients,
    subscriptions: LocalSubscriptions,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Enroll {
            hub,
            setup_key,
            data_dir,
        } => {
            enroll(
                &hub,
                &setup_key,
                &data_dir.unwrap_or_else(default_node_data_dir),
            )
            .await
        }
        Command::Serve {
            hub,
            data_dir,
            local_only,
            listen,
        } => {
            if local_only {
                return serve_local(data_dir.unwrap_or_else(default_node_data_dir), listen).await;
            }
            let _ = listen;
            serve(hub, data_dir.unwrap_or_else(default_node_data_dir)).await
        }
    }
}

async fn enroll(hub: &str, setup_key: &str, data_dir: &Path) -> Result<()> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let public_key = BASE64.encode(signing_key.verifying_key().to_bytes());
    let url = format!("{}/api/nodes/enroll", hub.trim_end_matches('/'));
    let hostname = std::env::var("HOSTNAME").ok().or_else(|| {
        std::fs::read_to_string("/etc/hostname")
            .ok()
            .map(|s| s.trim().to_string())
    });
    let response: EnrollResponse = reqwest::Client::new()
        .post(url)
        .json(&json!({"protocol_version": PROTOCOL_VERSION, "type":"node.enroll", "setup_key": setup_key, "hostname": hostname, "version": VERSION, "public_key": public_key}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if response.kind != "node.enrolled" {
        return Err(anyhow!(
            response.error.unwrap_or_else(|| "enrollment failed".into())
        ));
    }
    let config = NodeConfig {
        node_id: response.node_id.context("hub did not return node_id")?,
        hub_url: response.hub_url.unwrap_or_else(|| hub.to_string()),
        node_token: None,
        trusted_roots: default_trusted_roots()?,
        root_session_user_ids: Vec::new(),
        max_concurrent_sessions: None,
        max_sessions_per_project: None,
    };
    save_json_secure(
        &node_key_path(data_dir),
        &BASE64.encode(signing_key.to_bytes()),
    )
    .await?;
    save_config_secure(&config_path(data_dir), &config).await?;
    if !projects_path(data_dir).exists() {
        save_node_store(data_dir, &NodeStore::default()).await?;
    }
    println!("enrolled node {}", config.node_id);
    Ok(())
}

async fn serve(hub_override: Option<String>, data_dir: PathBuf) -> Result<()> {
    let mut config: NodeConfig = load_config(&data_dir)
        .await
        .context("node is not enrolled; run pumpkinpi-node enroll first")?;
    if let Some(hub) = hub_override {
        config.hub_url = hub;
    }
    if config.trusted_roots.is_empty() {
        config.trusted_roots = default_trusted_roots()?;
        save_config_secure(&config_path(&data_dir), &config).await?;
    }
    let ws_url = hub_ws_url(&config.hub_url);
    info!(%ws_url, node_id = %config.node_id, "connecting to hub");
    let signing_key = load_signing_key(&node_key_path(&data_dir)).await?;
    let (socket, _) = connect_async(&ws_url).await?;
    let (mut socket_write, mut socket_read) = socket.split();

    socket_write
        .send(Message::Text(
            json!({"protocol_version": PROTOCOL_VERSION, "type":"node.hello", "node_id": config.node_id, "version": VERSION})
                .to_string()
                .into(),
        ))
        .await?;
    let Some(challenge_msg) = socket_read.next().await else {
        return Err(anyhow!("hub closed before node challenge"));
    };
    let Message::Text(challenge_text) = challenge_msg? else {
        return Err(anyhow!("hub challenge was not text"));
    };
    let challenge: Value = serde_json::from_str(&challenge_text)?;
    if challenge.get("type").and_then(Value::as_str) != Some("node.challenge") {
        return Err(anyhow!("expected node.challenge"));
    }
    let nonce = challenge
        .get("nonce")
        .and_then(Value::as_str)
        .context("node.challenge nonce is required")?;
    let signature = signing_key.sign(nonce.as_bytes());
    socket_write
        .send(Message::Text(
            json!({"protocol_version": PROTOCOL_VERSION, "type":"node.auth", "node_id": config.node_id, "signature": BASE64.encode(signature.to_bytes())})
                .to_string()
                .into(),
        ))
        .await?;
    let Some(authenticated_msg) = socket_read.next().await else {
        return Err(anyhow!("hub closed before node.authenticated"));
    };
    let Message::Text(authenticated_text) = authenticated_msg? else {
        return Err(anyhow!("hub authentication response was not text"));
    };
    let authenticated: Value = serde_json::from_str(&authenticated_text)?;
    if authenticated.get("type").and_then(Value::as_str) != Some("node.authenticated") {
        return Err(anyhow!("node authentication failed: {authenticated}"));
    }

    let (hub_tx, mut hub_rx) = mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        while let Some(value) = hub_rx.recv().await {
            if socket_write
                .send(Message::Text(value.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    send_inventory(&hub_tx, &data_dir).await?;

    let runtimes: RuntimeMap = Arc::new(Mutex::new(HashMap::new()));
    let mut heartbeat = time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                hub_tx.send(json!({"type":"node.heartbeat", "node_id": config.node_id}))?;
            }
            msg = socket_read.next() => {
                let Some(msg) = msg else { break; };
                let Message::Text(text) = msg? else { continue; };
                let response = handle_command(&data_dir, &config, runtimes.clone(), hub_tx.clone(), &text).await;
                let inventory_changed = response.get("inventory_changed").and_then(Value::as_bool) == Some(true);
                hub_tx.send(response)?;
                if inventory_changed {
                    send_inventory(&hub_tx, &data_dir).await?;
                }
            }
        }
    }
    writer.abort();
    Ok(())
}

async fn send_inventory(hub_tx: &HubTx, data_dir: &Path) -> Result<()> {
    let mut store: NodeStore = load_node_store(data_dir).await?;
    let now = now_secs();
    let mut changed = false;
    for project in store.projects.values_mut() {
        let exists = validate_trusted_project_cwd(&project.cwd, &[PathBuf::from("/")]).is_ok();
        match (project.status.clone(), exists) {
            (ProjectStatus::Active, false) => {
                project.status = ProjectStatus::Missing;
                project.updated_at = now;
                changed = true;
            }
            (ProjectStatus::Missing, true) => {
                project.status = ProjectStatus::Active;
                project.updated_at = now;
                changed = true;
            }
            _ => {}
        }
    }
    if changed {
        save_node_store(data_dir, &store).await?;
    }
    hub_tx.send(json!({"protocol_version": PROTOCOL_VERSION, "type":"node.inventory", "complete": true, "revision": now, "projects": store.projects.values().collect::<Vec<_>>(), "sessions": store.sessions.values().collect::<Vec<_>>() }))?;
    Ok(())
}

async fn serve_local(data_dir: PathBuf, listen: String) -> Result<()> {
    let config = load_config(&data_dir).await.unwrap_or_else(|_| NodeConfig {
        node_id: "local".to_string(),
        hub_url: "local".to_string(),
        node_token: None,
        trusted_roots: default_trusted_roots().unwrap_or_default(),
        root_session_user_ids: Vec::new(),
        max_concurrent_sessions: None,
        max_sessions_per_project: None,
    });
    let runtimes: RuntimeMap = Arc::new(Mutex::new(HashMap::new()));
    let (hub_tx, mut hub_rx) = mpsc::unbounded_channel::<Value>();
    let clients: LocalClients = Arc::new(Mutex::new(HashMap::new()));
    let subscriptions: LocalSubscriptions = Arc::new(Mutex::new(HashMap::new()));
    let state = LocalState {
        data_dir,
        config,
        runtimes,
        hub_tx,
        clients: clients.clone(),
        subscriptions: subscriptions.clone(),
    };
    tokio::spawn(async move {
        while let Some(event) = hub_rx.recv().await {
            let Some(project_id) = event
                .get("project_id")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            let Some(session_id) = event
                .get("session_id")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            let recipients = {
                let subs = subscriptions.lock().await;
                subs.iter()
                    .filter_map(|(client_id, keys)| {
                        keys.contains(&(project_id.clone(), session_id.clone()))
                            .then_some(client_id.clone())
                    })
                    .collect::<Vec<_>>()
            };
            let message = AxumMessage::Text(event.to_string().into());
            let clients = clients.lock().await;
            for client_id in recipients {
                if let Some(tx) = clients.get(&client_id) {
                    let _ = tx.send(message.clone());
                }
            }
        }
    });
    let app = Router::new()
        .route("/health", get(local_health))
        .route("/ws/client", get(local_ws))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    info!(%listen, "starting local-only PumpkinPi node");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn local_health() -> impl IntoResponse {
    Json(
        json!({"ok": true, "service": "pumpkinpi-node-local", "version": VERSION, "protocol_version": PROTOCOL_VERSION}),
    )
}

async fn local_ws(ws: WebSocketUpgrade, State(state): State<LocalState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_local_socket(socket, state))
}

async fn handle_local_socket(socket: WebSocket, state: LocalState) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<AxumMessage>();
    let client_id = Uuid::new_v4().to_string();
    state.clients.lock().await.insert(client_id.clone(), tx);
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(AxumMessage::Text(text))) = stream.next().await {
        let mut value: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(err) => {
                send_local_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "type":"error", "error": err.to_string()})).await;
                continue;
            }
        };
        if value.get("type").and_then(Value::as_str) == Some("client.auth") {
            send_local_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "type":"client.authenticated", "user_id":"local"})).await;
            continue;
        }
        if value.get("node_id").is_none() {
            value["node_id"] = Value::String(state.config.node_id.clone());
        }
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        match value.get("type").and_then(Value::as_str).unwrap_or_default() {
            "hub.status" => send_local_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"hub.status.result", "version": VERSION, "local_only": true})).await,
            "node.list" => send_local_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"node.list.result", "nodes":[{"node_id": state.config.node_id, "name":"local", "status":"online"}]})).await,
            "session.attach" | "session.subscribe" => {
                let project_id = value.get("project_id").and_then(Value::as_str).unwrap_or_default().to_string();
                let session_id = value.get("session_id").and_then(Value::as_str).unwrap_or_default().to_string();
                state.subscriptions.lock().await.entry(client_id.clone()).or_default().insert((project_id.clone(), session_id.clone()));
                let (project, session) = match load_node_store(&state.data_dir).await {
                    Ok(store) => (
                        store.projects.get(&project_id).cloned(),
                        store.sessions.get(&session_id).cloned(),
                    ),
                    Err(_) => (None, None),
                };
                send_local_json(&state, &client_id, json!({
                    "protocol_version": PROTOCOL_VERSION,
                    "id": id,
                    "type":"session.subscribe.result",
                    "node_id": state.config.node_id,
                    "project_id": project_id,
                    "session_id": session_id,
                    "project": project,
                    "session": session,
                    "recent_events": []
                })).await;
            }
            "session.detach" => {
                if let (Some(project_id), Some(session_id)) = (value.get("project_id").and_then(Value::as_str), value.get("session_id").and_then(Value::as_str)) {
                    if let Some(keys) = state.subscriptions.lock().await.get_mut(&client_id) { keys.remove(&(project_id.to_string(), session_id.to_string())); }
                }
                send_local_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"session.detach.result"})).await;
            }
            _ => {
                let response = handle_command(&state.data_dir, &state.config, state.runtimes.clone(), state.hub_tx.clone(), &value.to_string()).await;
                send_local_json(&state, &client_id, response).await;
            }
        }
    }
    state.clients.lock().await.remove(&client_id);
    state.subscriptions.lock().await.remove(&client_id);
    writer.abort();
}

async fn send_local_json(state: &LocalState, client_id: &str, value: Value) {
    if let Some(tx) = state.clients.lock().await.get(client_id) {
        let _ = tx.send(AxumMessage::Text(value.to_string().into()));
    }
}

async fn handle_command(
    data_dir: &Path,
    config: &NodeConfig,
    runtimes: RuntimeMap,
    hub_tx: HubTx,
    text: &str,
) -> Value {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return json!({"type":"error", "error": e.to_string()}),
    };
    if let Err(err) = require_protocol_version(&value) {
        return json!({"protocol_version": PROTOCOL_VERSION, "type":"error", "error": err.to_string()});
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match handle_command_inner(data_dir, config, runtimes, hub_tx, &value, kind).await {
        Ok(mut body) => {
            body["protocol_version"] = json!(PROTOCOL_VERSION);
            body["id"] = id;
            body
        }
        Err(err) => {
            json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error": err.to_string()})
        }
    }
}

async fn handle_command_inner(
    data_dir: &Path,
    config: &NodeConfig,
    runtimes: RuntimeMap,
    hub_tx: HubTx,
    value: &Value,
    kind: &str,
) -> Result<Value> {
    let node_id = &config.node_id;
    let mut store: NodeStore = load_node_store(data_dir).await?;
    match kind {
        "project.list" => Ok(
            json!({"type":"project.list.result", "node_id": node_id, "projects": store.projects.values().collect::<Vec<_>>() }),
        ),
        "project.add" => {
            let cwd_input = value
                .get("cwd")
                .and_then(Value::as_str)
                .context("cwd is required")?;
            let cwd_path = validate_trusted_project_cwd(cwd_input, &config.trusted_roots)?;
            let cwd = cwd_path.to_string_lossy().to_string();
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    Path::new(&cwd)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("project")
                        .to_string()
                });
            let requested_run_as_user = value
                .get("run_as_user")
                .and_then(Value::as_str)
                .map(str::to_string);
            let run_as_user = match requested_run_as_user {
                Some(user) => {
                    passwd_entry_for_user(&user)
                        .with_context(|| format!("run_as_user {user:?} not found"))?;
                    Some(user)
                }
                None => project_owner_user(&cwd_path)?,
            };
            let allow_root_sessions = value
                .get("allow_root_sessions")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let now = now_secs();
            let project = ProjectRecord {
                project_id: format!("proj_{}", Uuid::new_v4().simple()),
                node_id: node_id.to_string(),
                name,
                cwd,
                default_pi_args: Vec::new(),
                default_provider: value
                    .get("default_provider")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                default_model: value
                    .get("default_model")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                run_as_user,
                allow_root_sessions,
                status: ProjectStatus::Active,
                trusted: true,
                created_at: now,
                updated_at: now,
            };
            store
                .projects
                .insert(project.project_id.clone(), project.clone());
            save_node_store(data_dir, &store).await?;
            Ok(
                json!({"type":"project.add.result", "node_id": node_id, "project_id": project.project_id, "project": project, "inventory_changed": true}),
            )
        }
        "project.remove" => {
            let project_id = value
                .get("project_id")
                .and_then(Value::as_str)
                .context("project_id is required")?;
            store.projects.remove(project_id);
            let removed_session_ids = store
                .sessions
                .values()
                .filter(|s| s.project_id == project_id)
                .map(|s| s.session_id.clone())
                .collect::<Vec<_>>();
            for session_id in &removed_session_ids {
                stop_runtime(&runtimes, session_id).await;
            }
            store.sessions.retain(|_, s| s.project_id != project_id);
            save_node_store(data_dir, &store).await?;
            Ok(
                json!({"type":"project.remove.result", "node_id": node_id, "project_id": project_id, "inventory_changed": true}),
            )
        }
        "project.get" => {
            let project_id = value
                .get("project_id")
                .and_then(Value::as_str)
                .context("project_id is required")?;
            let project = store
                .projects
                .get(project_id)
                .context("project not found")?;
            validate_trusted_project_cwd(&project.cwd, &config.trusted_roots)?;
            Ok(
                json!({"type":"project.get.result", "node_id": node_id, "project_id": project_id, "project": project}),
            )
        }
        "session.list" => {
            let project_id = value.get("project_id").and_then(Value::as_str);
            let sessions = store
                .sessions
                .values()
                .filter(|s| project_id.is_none_or(|p| s.project_id == p))
                .collect::<Vec<_>>();
            Ok(
                json!({"type":"session.list.result", "node_id": node_id, "project_id": project_id, "sessions": sessions}),
            )
        }
        "session.create" => {
            let project_id = value
                .get("project_id")
                .and_then(Value::as_str)
                .context("project_id is required")?
                .to_string();
            if !store.projects.contains_key(&project_id) {
                return Err(anyhow!("project not found"));
            }
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("session")
                .to_string();
            let project = store
                .projects
                .get(&project_id)
                .context("project not found")?;
            let run_as_root = value
                .get("run_as_root")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if run_as_root && !project.allow_root_sessions {
                return Err(anyhow!("root sessions are denied by project policy"));
            }
            if run_as_root && !origin_user_may_run_root(config, value) {
                return Err(anyhow!(
                    "root sessions are denied by node policy for this user"
                ));
            }
            let run_as_user = if run_as_root {
                None
            } else if let Some(user) = value.get("run_as_user").and_then(Value::as_str) {
                passwd_entry_for_user(user)
                    .with_context(|| format!("run_as_user {user:?} not found"))?;
                Some(user.to_string())
            } else {
                project.run_as_user.clone()
            };
            if let Some(limit) = config.max_sessions_per_project {
                let count = store
                    .sessions
                    .values()
                    .filter(|session| session.project_id == project_id)
                    .count();
                if count >= limit {
                    return Err(anyhow!("max sessions per project exceeded ({limit})"));
                }
            }
            let now = now_secs();
            let session = SessionRecord {
                session_id: format!("sess_{}", Uuid::new_v4().simple()),
                node_id: node_id.to_string(),
                project_id,
                name,
                cwd: project.cwd.clone(),
                status: SessionStatus::Idle,
                run_as_user,
                run_as_root,
                provider: value
                    .get("provider")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| project.default_provider.clone()),
                model: value
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| project.default_model.clone()),
                pi_session_id: None,
                pi_session_file: None,
                pi_leaf_id: None,
                pi_session_name: None,
                created_at: now,
                updated_at: now,
                last_active_at: None,
                crash: None,
            };
            store
                .sessions
                .insert(session.session_id.clone(), session.clone());
            save_node_store(data_dir, &store).await?;
            Ok(
                json!({"type":"session.create.result", "node_id": node_id, "project_id": session.project_id, "session_id": session.session_id, "session": session, "inventory_changed": true}),
            )
        }
        "session.stop" => {
            let (project_id, session_id) = require_session_route(value)?;
            let session = store
                .sessions
                .get(session_id)
                .context("session not found")?;
            if session.project_id != project_id {
                return Err(anyhow!("session does not belong to project"));
            }
            stop_runtime(&runtimes, session_id).await;
            update_session_status(data_dir, session_id, SessionStatus::Stopped).await?;
            Ok(
                json!({"type":"session.stop.result", "node_id": node_id, "project_id": project_id, "session_id": session_id, "inventory_changed": true}),
            )
        }
        "session.restart" => {
            let project_id = value
                .get("project_id")
                .and_then(Value::as_str)
                .context("project_id is required")?;
            let session_id = value
                .get("session_id")
                .and_then(Value::as_str)
                .context("session_id is required")?;
            stop_runtime(&runtimes, session_id).await;
            let project = store
                .projects
                .get(project_id)
                .context("project not found")?;
            validate_trusted_project_cwd(&project.cwd, &config.trusted_roots)?;
            let session = store
                .sessions
                .get_mut(session_id)
                .context("session not found")?;
            if session.project_id != project_id {
                return Err(anyhow!("session does not belong to project"));
            }
            session.status = SessionStatus::Starting;
            session.updated_at = now_secs();
            session.crash = None;
            let session_name = session.name.clone();
            let run_as_user = session.run_as_user.clone();
            let run_as_root = session.run_as_root;
            if run_as_root && !origin_user_may_run_root(config, value) {
                return Err(anyhow!(
                    "root sessions are denied by node policy for this user"
                ));
            }
            let pi_session_file = session.pi_session_file.clone();
            let selected_provider = value
                .get("provider")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| session.provider.clone())
                .or_else(|| project.default_provider.clone());
            let selected_model = value
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| session.model.clone())
                .or_else(|| project.default_model.clone());
            let provider_env = provider_env_from_value(value)?;
            save_node_store(data_dir, &store).await?;
            ensure_pi_runtime(
                data_dir,
                node_id,
                project,
                session_id,
                &session_name,
                run_as_user,
                run_as_root,
                pi_session_file,
                selected_provider,
                selected_model,
                provider_env,
                runtimes.clone(),
                hub_tx,
            )
            .await?;
            Ok(
                json!({"type":"session.restart.result", "node_id": node_id, "project_id": project_id, "session_id": session_id, "inventory_changed": true}),
            )
        }
        "session.delete" => {
            let (project_id, session_id) = require_session_route(value)?;
            let session = store
                .sessions
                .get(session_id)
                .context("session not found")?;
            if session.project_id != project_id {
                return Err(anyhow!("session does not belong to project"));
            }
            stop_runtime(&runtimes, session_id).await;
            store
                .sessions
                .remove(session_id)
                .context("session not found")?;
            save_node_store(data_dir, &store).await?;
            Ok(
                json!({"type":"session.delete.result", "node_id": node_id, "project_id": project_id, "session_id": session_id, "inventory_changed": true}),
            )
        }
        "session.send" => {
            let project_id = value
                .get("project_id")
                .and_then(Value::as_str)
                .context("project_id is required")?;
            let session_id = value
                .get("session_id")
                .and_then(Value::as_str)
                .context("session_id is required")?;
            let command = value
                .get("command")
                .cloned()
                .context("command is required")?;
            validate_pi_command(&command)?;
            let project = store
                .projects
                .get(project_id)
                .context("project not found")?;
            validate_trusted_project_cwd(&project.cwd, &config.trusted_roots)?;
            let session = store
                .sessions
                .get_mut(session_id)
                .context("session not found")?;
            if session.project_id != project_id {
                return Err(anyhow!("session does not belong to project"));
            }
            let terminal_status = matches!(
                session.status,
                SessionStatus::Crashed
                    | SessionStatus::Missing
                    | SessionStatus::Stopped
                    | SessionStatus::Stale
            );
            if terminal_status && !is_diagnostic_pi_command(&command) {
                return Err(anyhow!(
                    "session is not accepting normal commands while {:?}",
                    session.status
                ));
            }
            if terminal_status && !runtimes.lock().await.contains_key(session_id) {
                return Err(anyhow!(
                    "session runtime is not active while {:?}; restart explicitly before querying Pi",
                    session.status
                ));
            }
            session.status = SessionStatus::Running;
            session.updated_at = now_secs();
            session.last_active_at = Some(session.updated_at);
            let session_name = session.name.clone();
            let run_as_user = session.run_as_user.clone();
            let run_as_root = session.run_as_root;
            if run_as_root && !origin_user_may_run_root(config, value) {
                return Err(anyhow!(
                    "root sessions are denied by node policy for this user"
                ));
            }
            let pi_session_file = session.pi_session_file.clone();
            let selected_provider = value
                .get("provider")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| session.provider.clone())
                .or_else(|| project.default_provider.clone());
            let selected_model = value
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| session.model.clone())
                .or_else(|| project.default_model.clone());
            let provider_env = provider_env_from_value(value)?;
            save_node_store(data_dir, &store).await?;
            ensure_pi_runtime(
                data_dir,
                node_id,
                project,
                session_id,
                &session_name,
                run_as_user,
                run_as_root,
                pi_session_file,
                selected_provider,
                selected_model,
                provider_env,
                runtimes.clone(),
                hub_tx,
            )
            .await?;
            let handle = runtimes
                .lock()
                .await
                .get(session_id)
                .cloned()
                .context("session runtime missing")?;
            validate_extension_ui_response_if_needed(&command, &handle.pending_extension_ui)
                .await?;
            dispatch_runtime_command(
                &handle,
                RuntimeCommand::Pi {
                    command,
                    origin_client_id: value
                        .get("origin_client_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    origin_external_id: value
                        .get("origin_external_id")
                        .cloned()
                        .unwrap_or_else(|| value.get("id").cloned().unwrap_or(Value::Null)),
                },
            )?;
            let recent_event_count = handle.recent_events.lock().await.len();
            Ok(
                json!({"type":"session.send.result", "node_id": node_id, "project_id": project_id, "session_id": session_id, "accepted": true, "recent_event_count": recent_event_count, "inventory_changed": true}),
            )
        }
        _ => Err(anyhow!("unknown command type: {kind}")),
    }
}

fn require_session_route(value: &Value) -> Result<(&str, &str)> {
    let project_id = value
        .get("project_id")
        .and_then(Value::as_str)
        .context("project_id is required")?;
    let session_id = value
        .get("session_id")
        .and_then(Value::as_str)
        .context("session_id is required")?;
    Ok((project_id, session_id))
}

fn is_diagnostic_pi_command(command: &Value) -> bool {
    matches!(
        command.get("type").and_then(Value::as_str),
        Some(
            "get_state"
                | "get_messages"
                | "get_available_models"
                | "get_available_thinking_levels"
                | "get_session_stats"
                | "get_fork_messages"
                | "get_entries"
                | "get_tree"
                | "get_last_assistant_text"
                | "get_commands"
        )
    )
}

async fn ensure_pi_runtime(
    data_dir: &Path,
    node_id: &str,
    project: &ProjectRecord,
    session_id: &str,
    session_name: &str,
    run_as_user: Option<String>,
    run_as_root: bool,
    pi_session_file: Option<String>,
    selected_provider: Option<String>,
    selected_model: Option<String>,
    provider_env: HashMap<String, String>,
    runtimes: RuntimeMap,
    hub_tx: HubTx,
) -> Result<()> {
    if runtimes.lock().await.contains_key(session_id) {
        return Ok(());
    }
    if let Some(limit) = load_config(data_dir)
        .await
        .ok()
        .and_then(|config| config.max_concurrent_sessions)
    {
        let active = runtimes.lock().await.len();
        if active >= limit {
            return Err(anyhow!("max concurrent sessions exceeded ({limit})"));
        }
    }
    if let Some(path) = &pi_session_file {
        if !Path::new(path).exists() {
            update_session_status(data_dir, session_id, SessionStatus::Missing).await?;
            return Err(anyhow!("Pi session file is missing: {path}"));
        }
    }
    let execution_identity = resolve_execution_identity(run_as_user.as_deref(), run_as_root)?;
    let (lifecycle_tx, lifecycle_rx) = mpsc::unbounded_channel::<RuntimeCommand>();
    let (unblock_tx, unblock_rx) = mpsc::unbounded_channel::<RuntimeCommand>();
    let (cancellation_tx, cancellation_rx) = mpsc::unbounded_channel::<RuntimeCommand>();
    let (normal_tx, normal_rx) = mpsc::unbounded_channel::<RuntimeCommand>();
    let pending_extension_ui = Arc::new(Mutex::new(HashMap::new()));
    let current_origin_client_id = Arc::new(Mutex::new(None));
    let response_routes = Arc::new(Mutex::new(HashMap::new()));
    let recent_events = Arc::new(Mutex::new(VecDeque::with_capacity(256)));
    runtimes.lock().await.insert(
        session_id.to_string(),
        RuntimeHandle {
            lifecycle_tx,
            unblock_tx,
            cancellation_tx,
            normal_tx,
            pending_extension_ui: pending_extension_ui.clone(),
            recent_events: recent_events.clone(),
        },
    );
    spawn_pi_runtime(
        data_dir.to_path_buf(),
        node_id.to_string(),
        project.project_id.clone(),
        project.cwd.clone(),
        session_id.to_string(),
        session_name.to_string(),
        execution_identity,
        pi_session_file,
        selected_provider,
        selected_model,
        provider_env,
        pending_extension_ui,
        current_origin_client_id,
        response_routes,
        recent_events,
        runtimes,
        lifecycle_rx,
        unblock_rx,
        cancellation_rx,
        normal_rx,
        hub_tx,
    );
    Ok(())
}

fn spawn_pi_runtime(
    data_dir: PathBuf,
    node_id: String,
    project_id: String,
    cwd: String,
    session_id: String,
    session_name: String,
    execution_identity: ExecutionIdentity,
    pi_session_file: Option<String>,
    selected_provider: Option<String>,
    selected_model: Option<String>,
    provider_env: HashMap<String, String>,
    pending_extension_ui: Arc<Mutex<HashMap<String, ExtensionUiRequest>>>,
    current_origin_client_id: Arc<Mutex<Option<String>>>,
    response_routes: Arc<Mutex<HashMap<String, (Option<String>, Value)>>>,
    recent_events: Arc<Mutex<VecDeque<Value>>>,
    runtimes: RuntimeMap,
    mut lifecycle_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
    mut unblock_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
    mut cancellation_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
    mut normal_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
    hub_tx: HubTx,
) {
    tokio::spawn(async move {
        let result = async {
            hub_tx.send(json!({"protocol_version": PROTOCOL_VERSION, "type":"session.starting", "node_id": node_id, "project_id": project_id, "session_id": session_id}))?;
            let mut command = TokioCommand::new("pi");
            command
                .arg("--mode")
                .arg("rpc")
                .arg("--name")
                .arg(&session_name);
            if let Some(provider) = &selected_provider {
                command.arg("--provider").arg(provider);
            }
            if let Some(model) = &selected_model {
                command.arg("--model").arg(model);
            }
            command
                .current_dir(&cwd)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            apply_execution_identity(&mut command, &execution_identity);
            node_audit_log(&data_dir, json!({
                "event":"pi.spawn",
                "node_id": node_id,
                "project_id": project_id,
                "session_id": session_id,
                "cwd": cwd,
                "execution_identity": execution_identity.label(),
                "provider": selected_provider,
                "model": selected_model,
            })).await;
            command.envs(&provider_env);
            let mut child = command
                .spawn()
                .with_context(|| format!("spawn pi in {cwd} as {}", execution_identity.label()))?;
            let mut stdin = child.stdin.take().context("pi stdin unavailable")?;
            let stdout = child.stdout.take().context("pi stdout unavailable")?;
            let stderr = child.stderr.take().context("pi stderr unavailable")?;

            let stderr_tail = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(100)));

            let out_node = node_id.clone();
            let out_project = project_id.clone();
            let out_session = session_id.clone();
            let out_hub = hub_tx.clone();
            let out_data_dir = data_dir.clone();
            let out_pending_extension_ui = pending_extension_ui.clone();
            let out_current_origin_client_id = current_origin_client_id.clone();
            let out_response_routes = response_routes.clone();
            let out_recent_events = recent_events.clone();
            let stdout_task = tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    match serde_json::from_str::<Value>(line.trim_end_matches('\r')) {
                        Ok(event) => {
                            if let Some(status) = session_status_for_pi_event(&event) {
                                let _ = update_session_status(&out_data_dir, &out_session, status).await;
                            }
                            let _ = update_session_pi_metadata_from_event(&out_data_dir, &out_session, &event).await;
                            let normalized = normalize_pi_event(
                                &out_data_dir,
                                &out_node,
                                &out_project,
                                &out_session,
                                event,
                                &out_pending_extension_ui,
                                &out_current_origin_client_id,
                                &out_response_routes,
                            )
                            .await;
                            remember_recent_event(&out_recent_events, normalized.clone()).await;
                            let _ = out_hub.send(normalized);
                        }
                        Err(err) => warn!(%err, %line, "invalid pi rpc json line"),
                    }
                }
            });
            let err_session = session_id.clone();
            let err_tail = stderr_tail.clone();
            let stderr_task = tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut tail = err_tail.lock().await;
                    if tail.len() == 100 {
                        tail.pop_front();
                    }
                    tail.push_back(line.clone());
                    warn!(session_id = %err_session, stderr = %line, "pi stderr");
                }
            });

            update_session_status(&data_dir, &session_id, SessionStatus::Running).await?;
            hub_tx.send(json!({"protocol_version": PROTOCOL_VERSION, "type":"session.running", "node_id": node_id, "project_id": project_id, "session_id": session_id}))?;
            if let Some(session_path) = &pi_session_file {
                let switch_id = format!("node:{}:switch_session", Uuid::new_v4().simple());
                response_routes.lock().await.insert(switch_id.clone(), (None, Value::String(switch_id.clone())));
                write_pi_command(&mut stdin, &json!({"id": switch_id, "type":"switch_session", "sessionPath": session_path})).await?;
            }
            let state_probe_id = format!("node:{}:get_state", Uuid::new_v4().simple());
            response_routes.lock().await.insert(state_probe_id.clone(), (None, Value::String(state_probe_id.clone())));
            write_pi_command(&mut stdin, &json!({"id": state_probe_id, "type":"get_state"})).await?;
            let entries_probe_id = format!("node:{}:get_entries", Uuid::new_v4().simple());
            response_routes.lock().await.insert(entries_probe_id.clone(), (None, Value::String(entries_probe_id.clone())));
            write_pi_command(&mut stdin, &json!({"id": entries_probe_id, "type":"get_entries"})).await?;
            let mut stop_requested = false;
            let exit_status = loop {
                tokio::select! {
                    biased;
                    command = lifecycle_rx.recv() => {
                        if handle_runtime_command(command, &mut stdin, &mut child, &current_origin_client_id, &response_routes).await? {
                            stop_requested = true;
                            break graceful_stop(&mut child, &mut stdin).await?;
                        }
                    }
                    command = unblock_rx.recv() => {
                        if handle_runtime_command(command, &mut stdin, &mut child, &current_origin_client_id, &response_routes).await? {
                            stop_requested = true;
                            break graceful_stop(&mut child, &mut stdin).await?;
                        }
                    }
                    command = cancellation_rx.recv() => {
                        if handle_runtime_command(command, &mut stdin, &mut child, &current_origin_client_id, &response_routes).await? {
                            stop_requested = true;
                            break graceful_stop(&mut child, &mut stdin).await?;
                        }
                    }
                    command = normal_rx.recv() => {
                        if handle_runtime_command(command, &mut stdin, &mut child, &current_origin_client_id, &response_routes).await? {
                            stop_requested = true;
                            break graceful_stop(&mut child, &mut stdin).await?;
                        }
                    }
                    status = child.wait() => break status.context("wait for pi process")?,
                }
            };
            stdout_task.abort();
            stderr_task.abort();
            if stop_requested || exit_status.success() {
                update_session_status(&data_dir, &session_id, SessionStatus::Stopped).await?;
                Result::<bool>::Ok(false)
            } else {
                let tail = stderr_tail.lock().await.iter().cloned().collect::<Vec<_>>();
                record_session_crash(&data_dir, &session_id, Some(&exit_status), tail).await?;
                Result::<bool>::Ok(true)
            }
        }
        .await;

        runtimes.lock().await.remove(&session_id);
        match result {
            Ok(true) => {
                let _ = hub_tx.send(json!({"protocol_version": PROTOCOL_VERSION, "type":"session.crashed", "node_id": node_id, "project_id": project_id, "session_id": session_id}));
            }
            Ok(false) => {
                let _ = hub_tx.send(json!({"protocol_version": PROTOCOL_VERSION, "type":"session.stopped", "node_id": node_id, "project_id": project_id, "session_id": session_id}));
            }
            Err(err) => {
                error!(%err, %session_id, "pi runtime failed");
                let _ =
                    record_session_crash(&data_dir, &session_id, None, vec![err.to_string()]).await;
                let _ = hub_tx.send(json!({"protocol_version": PROTOCOL_VERSION, "type":"session.crashed", "node_id": node_id, "project_id": project_id, "session_id": session_id, "error": err.to_string()}));
            }
        }
    });
}

#[derive(Debug, Clone)]
struct PasswdEntry {
    name: String,
    uid: u32,
    gid: u32,
}

#[derive(Debug, Clone)]
enum ExecutionIdentity {
    Root,
    User(PasswdEntry),
}

impl ExecutionIdentity {
    fn label(&self) -> String {
        match self {
            ExecutionIdentity::Root => "root".to_string(),
            ExecutionIdentity::User(user) => format!("{}:{}", user.name, user.uid),
        }
    }
}

fn default_trusted_roots() -> Result<Vec<PathBuf>> {
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    Ok(vec![root.canonicalize().unwrap_or(root)])
}

fn validate_trusted_project_cwd(cwd: &str, trusted_roots: &[PathBuf]) -> Result<PathBuf> {
    let canonical = Path::new(cwd)
        .canonicalize()
        .with_context(|| format!("project cwd {cwd:?} must exist and be accessible"))?;
    if !canonical.is_dir() {
        return Err(anyhow!(
            "project cwd must be a directory: {}",
            canonical.display()
        ));
    }
    let trusted = trusted_roots
        .iter()
        .map(|root| root.canonicalize().unwrap_or_else(|_| root.clone()))
        .any(|root| canonical == root || canonical.starts_with(&root));
    if !trusted {
        return Err(anyhow!(
            "project cwd {} is outside trusted roots: {}",
            canonical.display(),
            trusted_roots
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(canonical)
}

#[cfg(unix)]
fn project_owner_user(cwd: &Path) -> Result<Option<String>> {
    let uid = std::fs::metadata(cwd)?.uid();
    Ok(passwd_entry_for_uid(uid).map(|entry| entry.name))
}

#[cfg(not(unix))]
fn project_owner_user(_cwd: &Path) -> Result<Option<String>> {
    Ok(None)
}

fn resolve_execution_identity(
    run_as_user: Option<&str>,
    run_as_root: bool,
) -> Result<ExecutionIdentity> {
    if run_as_root {
        return Ok(ExecutionIdentity::Root);
    }
    let user = run_as_user.context("non-root Pi sessions require run_as_user")?;
    Ok(ExecutionIdentity::User(
        passwd_entry_for_user(user).with_context(|| format!("run_as_user {user:?} not found"))?,
    ))
}

fn passwd_entry_for_user(name: &str) -> Option<PasswdEntry> {
    parse_passwd_entries()
        .ok()?
        .into_iter()
        .find(|entry| entry.name == name)
}

fn passwd_entry_for_uid(uid: u32) -> Option<PasswdEntry> {
    parse_passwd_entries()
        .ok()?
        .into_iter()
        .find(|entry| entry.uid == uid)
}

fn parse_passwd_entries() -> Result<Vec<PasswdEntry>> {
    let passwd = std::fs::read_to_string("/etc/passwd").context("read /etc/passwd")?;
    Ok(passwd
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(':');
            let name = parts.next()?.to_string();
            let _password = parts.next()?;
            let uid = parts.next()?.parse().ok()?;
            let gid = parts.next()?.parse().ok()?;
            Some(PasswdEntry { name, uid, gid })
        })
        .collect())
}

fn apply_execution_identity(command: &mut TokioCommand, identity: &ExecutionIdentity) {
    #[cfg(unix)]
    match identity {
        ExecutionIdentity::Root => {
            command.uid(0).gid(0);
        }
        ExecutionIdentity::User(user) => {
            command.uid(user.uid).gid(user.gid);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = command;
        let _ = identity;
    }
}

fn origin_user_may_run_root(config: &NodeConfig, value: &Value) -> bool {
    value
        .get("origin_user_id")
        .and_then(Value::as_str)
        .is_some_and(|user_id| {
            config
                .root_session_user_ids
                .iter()
                .any(|allowed| allowed == user_id)
        })
}

async fn node_audit_log(data_dir: &Path, mut event: Value) {
    event["at"] = json!(now_secs());
    event = redact_sensitive_value(event);
    let path = data_dir.join("logs").join("audit.log");
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut line = event.to_string();
    line.push('\n');
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        let _ = file.write_all(line.as_bytes()).await;
    }
}

fn require_protocol_version(value: &Value) -> Result<()> {
    let version = value
        .get("protocol_version")
        .and_then(Value::as_u64)
        .context("protocol_version is required")?;
    if version != u64::from(PROTOCOL_VERSION) {
        return Err(anyhow!(
            "unsupported protocol_version {version}; expected {}",
            PROTOCOL_VERSION
        ));
    }
    Ok(())
}

fn redact_sensitive_value(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let sensitive = matches!(
                    key.as_str(),
                    "provider_env"
                        | "api_key"
                        | "token"
                        | "setup_key"
                        | "authorization"
                        | "message"
                        | "command"
                        | "stdout"
                        | "stderr"
                        | "cwd"
                        | "path"
                        | "args"
                ) || key.to_ascii_lowercase().contains("secret");
                if sensitive {
                    *child = Value::String("<redacted>".to_string());
                } else {
                    *child = redact_sensitive_value(std::mem::take(child));
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                *child = redact_sensitive_value(std::mem::take(child));
            }
        }
        _ => {}
    }
    value
}

async fn node_diagnostic_log(data_dir: &Path, mut event: Value) {
    event["at"] = json!(now_secs());
    let path = data_dir.join("logs").join("diagnostics.log");
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut line = redact_sensitive_value(event).to_string();
    line.push('\n');
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        let _ = file.write_all(line.as_bytes()).await;
    }
}

fn provider_env_from_value(value: &Value) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    let Some(object) = value.get("provider_env").and_then(Value::as_object) else {
        return Ok(env);
    };
    for (key, value) in object {
        let secret = value
            .as_str()
            .with_context(|| format!("provider_env.{key} must be a string"))?;
        env.insert(key.clone(), secret.to_string());
    }
    Ok(env)
}

async fn stop_runtime(runtimes: &RuntimeMap, session_id: &str) {
    if let Some(handle) = runtimes.lock().await.remove(session_id) {
        let _ = handle.lifecycle_tx.send(RuntimeCommand::Stop);
    }
}

fn dispatch_runtime_command(handle: &RuntimeHandle, command: RuntimeCommand) -> Result<()> {
    let command_type = match &command {
        RuntimeCommand::Pi { command, .. } => command.get("type").and_then(Value::as_str),
        RuntimeCommand::Stop => None,
    };
    let tx = match command_type {
        Some("extension_ui_response") => &handle.unblock_tx,
        Some("abort") | Some("abort_bash") | Some("abort_retry") | Some("clear_queue") => {
            &handle.cancellation_tx
        }
        _ => &handle.normal_tx,
    };
    tx.send(command)?;
    Ok(())
}

async fn handle_runtime_command(
    command: Option<RuntimeCommand>,
    stdin: &mut tokio::process::ChildStdin,
    _child: &mut tokio::process::Child,
    current_origin_client_id: &Arc<Mutex<Option<String>>>,
    response_routes: &Arc<Mutex<HashMap<String, (Option<String>, Value)>>>,
) -> Result<bool> {
    match command {
        Some(RuntimeCommand::Pi {
            mut command,
            origin_client_id,
            origin_external_id,
        }) => {
            *current_origin_client_id.lock().await = origin_client_id.clone();
            let internal_id = ensure_internal_pi_id(&mut command, origin_client_id.as_deref());
            response_routes
                .lock()
                .await
                .insert(internal_id, (origin_client_id, origin_external_id));
            write_pi_command(stdin, &command).await?;
            Ok(false)
        }
        Some(RuntimeCommand::Stop) | None => Ok(true),
    }
}

fn ensure_internal_pi_id(command: &mut Value, origin_client_id: Option<&str>) -> String {
    let external = command
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("req_{}", Uuid::new_v4().simple()));
    let internal = format!("{}:{external}", origin_client_id.unwrap_or("node"));
    command["id"] = Value::String(internal.clone());
    internal
}

async fn route_for_pi_id(
    routes: &Arc<Mutex<HashMap<String, (Option<String>, Value)>>>,
    id: &Value,
) -> Option<(Option<String>, Value)> {
    routes.lock().await.get(id.as_str()?).cloned()
}

async fn take_route_for_pi_id(
    routes: &Arc<Mutex<HashMap<String, (Option<String>, Value)>>>,
    id: &Value,
) -> Option<(Option<String>, Value)> {
    routes.lock().await.remove(id.as_str()?)
}

async fn remember_recent_event(buffer: &Arc<Mutex<VecDeque<Value>>>, event: Value) {
    let mut buffer = buffer.lock().await;
    if buffer.len() == 256 {
        buffer.pop_front();
    }
    buffer.push_back(event);
}

async fn write_pi_command(stdin: &mut tokio::process::ChildStdin, command: &Value) -> Result<()> {
    stdin.write_all(command.to_string().as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    Ok(())
}

async fn graceful_stop(
    child: &mut tokio::process::Child,
    stdin: &mut tokio::process::ChildStdin,
) -> Result<std::process::ExitStatus> {
    let _ = write_pi_command(stdin, &json!({"type":"clear_queue"})).await;
    let _ = write_pi_command(stdin, &json!({"type":"abort"})).await;
    match time::timeout(Duration::from_secs(5), child.wait()).await {
        Ok(status) => Ok(status?),
        Err(_) => {
            let _ = child.kill().await;
            Ok(child.wait().await?)
        }
    }
}

async fn update_session_status(
    data_dir: &Path,
    session_id: &str,
    status: SessionStatus,
) -> Result<()> {
    let mut store: NodeStore = load_node_store(data_dir).await?;
    if let Some(session) = store.sessions.get_mut(session_id) {
        session.status = status;
        session.updated_at = now_secs();
        if matches!(session.status, SessionStatus::Running | SessionStatus::Idle) {
            session.last_active_at = Some(session.updated_at);
        }
        save_node_store(data_dir, &store).await?;
    }
    Ok(())
}

async fn update_session_pi_metadata_from_event(
    data_dir: &Path,
    session_id: &str,
    event: &Value,
) -> Result<()> {
    if event.get("type").and_then(Value::as_str) != Some("response") {
        return Ok(());
    }
    if event.get("command").and_then(Value::as_str) != Some("get_state") {
        return Ok(());
    }
    let Some(data) = event.get("data") else {
        return Ok(());
    };
    let mut store: NodeStore = load_node_store(data_dir).await?;
    if let Some(session) = store.sessions.get_mut(session_id) {
        if let Some(value) = data.get("sessionId").and_then(Value::as_str) {
            session.pi_session_id = Some(value.to_string());
        }
        if let Some(value) = data.get("sessionFile").and_then(Value::as_str) {
            session.pi_session_file = Some(value.to_string());
        }
        if let Some(value) = data.get("leafId").and_then(Value::as_str) {
            session.pi_leaf_id = Some(value.to_string());
        }
        if let Some(value) = data.get("sessionName").and_then(Value::as_str) {
            session.pi_session_name = Some(value.to_string());
        }
        session.updated_at = now_secs();
        save_node_store(data_dir, &store).await?;
    }
    Ok(())
}

async fn record_session_crash(
    data_dir: &Path,
    session_id: &str,
    exit_status: Option<&std::process::ExitStatus>,
    stderr_tail: Vec<String>,
) -> Result<()> {
    let mut store: NodeStore = load_node_store(data_dir).await?;
    if let Some(session) = store.sessions.get_mut(session_id) {
        let now = now_secs();
        session.status = SessionStatus::Crashed;
        session.updated_at = now;
        session.crash = Some(CrashInfo {
            exit_status: exit_status.and_then(std::process::ExitStatus::code),
            signal: exit_status.and_then(exit_signal),
            stderr_tail,
            crashed_at: now,
        });
        save_node_store(data_dir, &store).await?;
    }
    Ok(())
}

fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        status.signal()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

fn session_status_for_pi_event(event: &Value) -> Option<SessionStatus> {
    match event.get("type").and_then(Value::as_str) {
        Some("agent_start") | Some("turn_start") => Some(SessionStatus::Running),
        Some("agent_settled") => Some(SessionStatus::Idle),
        _ => None,
    }
}

async fn validate_extension_ui_response_if_needed(
    command: &Value,
    pending: &Arc<Mutex<HashMap<String, ExtensionUiRequest>>>,
) -> Result<()> {
    if command.get("type").and_then(Value::as_str) != Some("extension_ui_response") {
        return Ok(());
    }
    let request_id =
        extension_request_id(command).context("extension_ui_response requires request_id or id")?;
    let request = pending
        .lock()
        .await
        .remove(&request_id)
        .with_context(|| format!("no pending extension UI request {request_id:?}"))?;
    info!(
        request_id = %request.request_id,
        method = %request.method,
        origin_client_id = ?request.origin_client_id,
        created_at = request.created_at,
        "accepted extension UI response"
    );
    Ok(())
}

fn validate_pi_command(command: &Value) -> Result<()> {
    let command_type = command
        .get("type")
        .and_then(Value::as_str)
        .context("command.type is required")?;
    match pi_command_policy(command_type) {
        CommandPolicy::Allowed => Ok(()),
        CommandPolicy::DeniedSessionBinding => Err(anyhow!(
            "Pi command {command_type:?} is denied until PumpkinPi can update session bindings atomically"
        )),
        CommandPolicy::Unknown => Err(anyhow!("unknown Pi command type: {command_type}")),
    }
}

async fn normalize_pi_event(
    data_dir: &Path,
    node_id: &str,
    project_id: &str,
    session_id: &str,
    event: Value,
    pending_extension_ui: &Arc<Mutex<HashMap<String, ExtensionUiRequest>>>,
    current_origin_client_id: &Arc<Mutex<Option<String>>>,
    response_routes: &Arc<Mutex<HashMap<String, (Option<String>, Value)>>>,
) -> Value {
    let pi_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let base = |kind: &str| {
        json!({
            "protocol_version": PROTOCOL_VERSION,
            "type": kind,
            "node_id": node_id,
            "project_id": project_id,
            "session_id": session_id,
        })
    };

    match pi_type {
        "response" => {
            let mut value = base("session.command_response");
            let internal_id = event.get("id").cloned().unwrap_or(Value::Null);
            if let Some((target_client_id, external_id)) =
                take_route_for_pi_id(response_routes, &internal_id).await
            {
                value["id"] = external_id;
                if let Some(target_client_id) = target_client_id {
                    value["target_client_id"] = Value::String(target_client_id);
                }
            } else {
                value["id"] = internal_id;
            }
            if let Some(ok) = event.get("ok") {
                value["ok"] = ok.clone();
            }
            if let Some(error) = event.get("error") {
                value["error"] = error.clone();
            }
            if let Some(command) = event.get("command") {
                value["command"] = command.clone();
            }
            if let Some(data) = event.get("data") {
                value["data"] = data.clone();
            }
            value
        }
        "agent_start" | "turn_start" => base("session.running"),
        "agent_settled" => base("session.idle"),
        "agent_end" | "turn_end" => base("session.turn_ended"),
        "message_start" => {
            let mut value = base("session.message_start");
            copy_field(&event, &mut value, "message_id");
            copy_field(&event, &mut value, "role");
            value
        }
        "message_update" => {
            let mut value = base("session.output_delta");
            copy_field(&event, &mut value, "message_id");
            if let Some(delta) = event.pointer("/assistantMessageEvent/delta") {
                value["delta"] = delta.clone();
            }
            value
        }
        "message_end" => {
            let mut value = base("session.message_end");
            copy_field(&event, &mut value, "message_id");
            if let Some(message) = event.get("message") {
                value["message"] = message.clone();
            }
            value
        }
        "bash_execution_update" => {
            let mut value = base("session.bash_update");
            if let Some(update) = event.get("bashExecutionUpdate") {
                if let Some(internal_id) = update.get("id") {
                    if let Some((target_client_id, external_id)) =
                        route_for_pi_id(response_routes, internal_id).await
                    {
                        value["id"] = external_id;
                        if let Some(target_client_id) = target_client_id {
                            value["target_client_id"] = Value::String(target_client_id);
                        }
                    } else {
                        value["id"] = internal_id.clone();
                    }
                }
                copy_field(update, &mut value, "status");
                copy_field(update, &mut value, "stdout");
                copy_field(update, &mut value, "stderr");
                copy_field(update, &mut value, "exitCode");
            }
            value
        }
        "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => {
            let kind = match pi_type {
                "tool_execution_start" => "session.tool_start",
                "tool_execution_update" => "session.tool_update",
                _ => "session.tool_end",
            };
            let mut value = base(kind);
            if let Some(tool) = event
                .get("toolExecution")
                .or_else(|| event.get("toolExecutionStart"))
                .or_else(|| event.get("toolExecutionUpdate"))
                .or_else(|| event.get("toolExecutionEnd"))
            {
                value["tool"] = tool.clone();
            }
            value
        }
        "queue_update" => {
            let mut value = base("session.queue_update");
            copy_field(&event, &mut value, "queue");
            copy_field(&event, &mut value, "queueLength");
            value
        }
        "compaction_start" => base("session.compaction_start"),
        "compaction_end" => base("session.compaction_end"),
        "auto_retry_start" => base("session.auto_retry_start"),
        "auto_retry_end" => base("session.auto_retry_end"),
        "summarization_retry_scheduled" => base("session.summarization_retry_scheduled"),
        "summarization_retry_attempt_start" => base("session.summarization_retry_attempt_start"),
        "summarization_retry_finished" => base("session.summarization_retry_finished"),
        "extension_ui_request" => {
            let method = extension_method(&event).unwrap_or_else(|| "unknown".to_string());
            let request_id = extension_request_id(&event)
                .unwrap_or_else(|| format!("extui_{}", Uuid::new_v4().simple()));
            let origin_client_id = current_origin_client_id.lock().await.clone();
            let mut value = base("session.extension_ui_request");
            value["request_id"] = Value::String(request_id.clone());
            value["method"] = Value::String(method.clone());
            value["expects_response"] = Value::Bool(extension_method_expects_response(&method));
            value["target_client_id"] = origin_client_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null);
            copy_field(&event, &mut value, "timeout_ms");
            copy_field(&event, &mut value, "timeoutMs");
            copy_field(&event, &mut value, "title");
            copy_field(&event, &mut value, "message");
            copy_field(&event, &mut value, "prompt");
            copy_field(&event, &mut value, "options");
            if extension_method_expects_response(&method) {
                pending_extension_ui.lock().await.insert(
                    request_id.clone(),
                    ExtensionUiRequest {
                        request_id,
                        method,
                        origin_client_id,
                        created_at: now_secs(),
                    },
                );
            }
            value
        }
        "extension_error" => {
            let mut value = base("session.extension_error");
            copy_field(&event, &mut value, "error");
            value
        }
        other => {
            node_diagnostic_log(
                data_dir,
                json!({"event":"unknown_pi_event", "pi_type": other}),
            )
            .await;
            base("session.event_ignored")
        }
    }
}

fn extension_request_id(value: &Value) -> Option<String> {
    value
        .get("request_id")
        .or_else(|| value.get("requestId"))
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extension_method(value: &Value) -> Option<String> {
    value
        .get("method")
        .or_else(|| value.pointer("/extensionUiRequest/method"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extension_method_expects_response(method: &str) -> bool {
    matches!(method, "select" | "confirm" | "input" | "editor")
}

fn copy_field(from: &Value, to: &mut Value, field: &str) {
    if let Some(value) = from.get(field) {
        to[field] = value.clone();
    }
}

async fn load_config(data_dir: &Path) -> Result<NodeConfig> {
    let path = config_path(data_dir);
    if path.exists() {
        let data = tokio::fs::read(&path)
            .await
            .with_context(|| format!("read {}", path.display()))?;
        let text = String::from_utf8(data).context("config.toml is not utf-8")?;
        return toml::from_str(&text).with_context(|| format!("parse {}", path.display()));
    }
    load_json(&legacy_config_path(data_dir)).await
}

async fn save_config_secure(path: &Path, value: &NodeConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let data = toml::to_string_pretty(value)?.into_bytes();
    tokio::fs::write(path, data).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

async fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let data = tokio::fs::read(path)
        .await
        .with_context(|| format!("read {}", path.display()))?;
    Ok(serde_json::from_slice(&data)?)
}

async fn load_json_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    load_json(path).await
}

async fn load_node_store(data_dir: &Path) -> Result<NodeStore> {
    let projects_path = projects_path(data_dir);
    let sessions_path = sessions_path(data_dir);
    if projects_path.exists() || sessions_path.exists() {
        return Ok(NodeStore {
            projects: load_json_or_default(&projects_path).await?,
            sessions: load_json_or_default(&sessions_path).await?,
        });
    }
    let legacy_path = legacy_store_path(data_dir);
    if legacy_path.exists() {
        let store: NodeStore = load_json(&legacy_path).await?;
        save_node_store(data_dir, &store).await?;
        return Ok(store);
    }
    Ok(NodeStore::default())
}

async fn save_node_store(data_dir: &Path, store: &NodeStore) -> Result<()> {
    save_json_secure(&projects_path(data_dir), &store.projects).await?;
    save_json_secure(&sessions_path(data_dir), &store.sessions).await?;
    Ok(())
}

async fn save_json_secure<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let data = serde_json::to_vec_pretty(value)?;
    tokio::fs::write(path, data).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn default_node_data_dir() -> PathBuf {
    std::env::var_os("PUMPKINPI_NODE_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".pumpkinpi-node")))
        .unwrap_or_else(|| PathBuf::from(".pumpkinpi-node"))
}
fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.toml")
}
fn legacy_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}
fn node_key_path(data_dir: &Path) -> PathBuf {
    data_dir.join("node.key")
}
fn projects_path(data_dir: &Path) -> PathBuf {
    data_dir.join("projects.json")
}
fn sessions_path(data_dir: &Path) -> PathBuf {
    data_dir.join("sessions.json")
}
fn legacy_store_path(data_dir: &Path) -> PathBuf {
    data_dir.join("store.json")
}

async fn load_signing_key(path: &Path) -> Result<SigningKey> {
    let encoded: String = load_json(path)
        .await
        .with_context(|| format!("read node private key from {}", path.display()))?;
    let bytes = BASE64
        .decode(encoded.trim())
        .context("invalid node.key base64")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| anyhow!("node.key must contain 32 bytes, got {}", bytes.len()))?;
    Ok(SigningKey::from_bytes(&bytes))
}
fn hub_ws_url(hub: &str) -> String {
    format!(
        "{}/ws/node",
        hub.trim_end_matches('/')
            .replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1)
    )
}
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{
        Path as AxumPath, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use clap::Parser;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use pumpkinpi_protocol::PROTOCOL_VERSION;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;

mod cli;
mod node;
mod provider;
mod state;
mod user;

use cli::{
    Cli, HubSubcommand, NodeCommand, NodeSubcommand, ProviderCommand, ProviderSubcommand,
    ServeArgs, UserCommand, UserSubcommand,
};
use node::{
    ChallengeMessage, CreateNodeHttp, EnrollRequest, EnrollResponse, NodeRecord, NodeStatus,
};
use provider::{EncryptedSecret, ProviderAccountRecord};
use state::{AppState, PendingRequest, SessionKey};
use user::{NodeAccessGrant, UserRecord};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SETUP_KEY_TTL_SECS: u64 = 30 * 60;

#[derive(Debug, Serialize, Deserialize, Default)]
struct HubStore {
    #[serde(default)]
    nodes: HashMap<String, NodeRecord>,
    #[serde(default)]
    users: HashMap<String, UserRecord>,
    #[serde(default)]
    node_access_grants: Vec<NodeAccessGrant>,
    #[serde(default)]
    provider_accounts: HashMap<String, ProviderAccountRecord>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        HubSubcommand::Serve(args) => serve_hub(args).await,
        HubSubcommand::Node(node) => hub_node_cli(node).await,
        HubSubcommand::User(user) => hub_user_cli(user).await,
        HubSubcommand::Provider(provider) => hub_provider_cli(provider).await,
    }
}

async fn serve_hub(args: ServeArgs) -> Result<()> {
    let data_dir = args.data_dir.unwrap_or_else(default_hub_data_dir);
    let store = HubStore::load(&data_dir).await?;
    let state = AppState {
        store: Arc::new(Mutex::new(store)),
        node_channels: Arc::new(Mutex::new(HashMap::new())),
        client_channels: Arc::new(Mutex::new(HashMap::new())),
        client_users: Arc::new(Mutex::new(HashMap::new())),
        in_flight: Arc::new(Mutex::new(HashMap::new())),
        subscriptions: Arc::new(Mutex::new(HashMap::new())),
        recent_events: Arc::new(Mutex::new(HashMap::new())),
        data_dir,
        public_url: args.public_url,
        admin_token: args.admin_token,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/nodes", get(list_nodes_http).post(create_node_http))
        .route("/api/nodes/{node_id}/setup-key", post(issue_setup_key_http))
        .route("/api/nodes/{node_id}/revoke", post(revoke_node_http))
        .route("/api/nodes/{node_id}/disable", post(disable_node_http))
        .route(
            "/api/nodes/{node_id}/rotate-key",
            post(rotate_node_key_http),
        )
        .route("/api/nodes/enroll", post(enroll_node_http))
        .route("/ws/node", get(node_ws))
        .route("/ws/client", get(client_ws))
        .with_state(state);

    info!(listen = %args.listen, "starting PumpkinPi hub");
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn hub_node_cli(args: NodeCommand) -> Result<()> {
    let data_dir = args.data_dir.unwrap_or_else(default_hub_data_dir);
    let mut store = HubStore::load(&data_dir).await?;
    match args.command {
        NodeSubcommand::Create { name } => {
            let (node, setup_key) = store.create_node(name);
            store.save(&data_dir).await?;
            println!("node_id: {}", node.node_id);
            println!("setup_key: {setup_key}");
            println!("\nRun on node:");
            println!("pumpkinpi node enroll --hub <hub-url> --setup-key {setup_key}");
        }
        NodeSubcommand::List => {
            for node in store.nodes.values() {
                println!(
                    "{}\t{}\t{:?}\tlast_seen={}",
                    node.node_id,
                    node.name,
                    node.status,
                    node.last_seen_at.map_or("-".into(), |t| t.to_string())
                );
            }
        }
        NodeSubcommand::Revoke { node_id } => {
            store.revoke_node(&node_id)?;
            store.save(&data_dir).await?;
            println!("revoked {node_id}");
        }
        NodeSubcommand::IssueSetupKey { node_id } => {
            let setup_key = store.issue_setup_key(&node_id)?;
            store.save(&data_dir).await?;
            println!("node_id: {node_id}");
            println!("setup_key: {setup_key}");
        }
        NodeSubcommand::Disable { node_id } => {
            store.disable_node(&node_id)?;
            store.save(&data_dir).await?;
            println!("disabled {node_id}");
        }
        NodeSubcommand::RotateKey { node_id } => {
            let setup_key = store.rotate_node_key(&node_id)?;
            store.save(&data_dir).await?;
            println!("node_id: {node_id}");
            println!("setup_key: {setup_key}");
        }
    }
    Ok(())
}

async fn hub_user_cli(args: UserCommand) -> Result<()> {
    let data_dir = args.data_dir.unwrap_or_else(default_hub_data_dir);
    let mut store = HubStore::load(&data_dir).await?;
    match args.command {
        UserSubcommand::Create { username } => {
            let (user, token) = store.create_user(username);
            store.save(&data_dir).await?;
            println!("user_id: {}", user.user_id);
            println!("token: {token}");
            println!();
            println!("Client login:");
            println!("pumpkinpi login --hub ws://127.0.0.1:8080/ws/client --token {token}");
        }
        UserSubcommand::List => {
            for user in store.users.values() {
                println!("{}\t{}", user.user_id, user.username);
            }
        }
        UserSubcommand::GrantNode { user_id, node_id } => {
            store.grant_node_access(&user_id, &node_id)?;
            store.save(&data_dir).await?;
            println!("granted user {user_id} access to node {node_id}");
        }
    }
    Ok(())
}

async fn hub_provider_cli(args: ProviderCommand) -> Result<()> {
    let data_dir = args.data_dir.unwrap_or_else(default_hub_data_dir);
    let mut store = HubStore::load(&data_dir).await?;
    match args.command {
        ProviderSubcommand::AddApiKey {
            user_id,
            provider_id,
            display_name,
            api_key,
        } => {
            let account =
                store.add_provider_api_key(user_id, provider_id, display_name, api_key)?;
            store.save(&data_dir).await?;
            println!("provider_account_id: {}", account.provider_account_id);
        }
        ProviderSubcommand::List { user_id } => {
            for account in store
                .provider_accounts
                .values()
                .filter(|account| account.user_id == user_id && account.revoked_at.is_none())
            {
                println!(
                    "{}\t{}\t{}\t{}",
                    account.provider_account_id,
                    account.provider_id,
                    account.display_name,
                    account.auth_type
                );
            }
        }
        ProviderSubcommand::Revoke {
            provider_account_id,
        } => {
            store.revoke_provider_account(&provider_account_id)?;
            store.save(&data_dir).await?;
            println!("revoked provider account {provider_account_id}");
        }
    }
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(
        json!({"ok": true, "service": "pumpkinpi-hub", "version": VERSION, "protocol_version": PROTOCOL_VERSION}),
    )
}

async fn list_nodes_http(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(err) = require_admin_http(&state, &headers) {
        return Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()}));
    }
    let store = state.store.lock().await;
    Json(
        json!({"protocol_version": PROTOCOL_VERSION, "nodes": store.nodes.values().collect::<Vec<_>>() }),
    )
}

async fn create_node_http(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateNodeHttp>,
) -> impl IntoResponse {
    if let Err(err) = require_admin_http(&state, &headers) {
        return Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()}));
    }
    let mut store = state.store.lock().await;
    let (node, setup_key) = store.create_node(body.name);
    if let Err(err) = store.save(&state.data_dir).await {
        error!(?err, "failed to save hub store");
    }
    Json(json!({"node": node, "setup_key": setup_key}))
}

async fn issue_setup_key_http(
    State(state): State<AppState>,
    AxumPath(node_id): AxumPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = require_admin_http(&state, &headers) {
        return Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()}));
    }
    let mut store = state.store.lock().await;
    match store.issue_setup_key(&node_id) {
        Ok(setup_key) => {
            if let Err(err) = store.save(&state.data_dir).await {
                error!(?err, "failed to save hub store");
            }
            Json(json!({"node_id": node_id, "setup_key": setup_key}))
        }
        Err(err) => Json(json!({"error": err.to_string()})),
    }
}

async fn revoke_node_http(
    State(state): State<AppState>,
    AxumPath(node_id): AxumPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = require_admin_http(&state, &headers) {
        return Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()}));
    }
    let result = {
        let mut store = state.store.lock().await;
        let result = store.revoke_node(&node_id);
        if result.is_ok() {
            if let Err(err) = store.save(&state.data_dir).await {
                error!(?err, "failed to save hub store");
            }
        }
        result
    };
    match result {
        Ok(()) => {
            disconnect_node(&state, &node_id).await;
            Json(json!({"protocol_version": PROTOCOL_VERSION, "node_id": node_id, "revoked": true}))
        }
        Err(err) => Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()})),
    }
}

async fn disable_node_http(
    State(state): State<AppState>,
    AxumPath(node_id): AxumPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = require_admin_http(&state, &headers) {
        return Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()}));
    }
    let result = {
        let mut store = state.store.lock().await;
        let result = store.disable_node(&node_id);
        if result.is_ok() {
            if let Err(err) = store.save(&state.data_dir).await {
                error!(?err, "failed to save hub store");
            }
        }
        result
    };
    match result {
        Ok(()) => {
            disconnect_node(&state, &node_id).await;
            Json(
                json!({"protocol_version": PROTOCOL_VERSION, "node_id": node_id, "disabled": true}),
            )
        }
        Err(err) => Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()})),
    }
}

async fn rotate_node_key_http(
    State(state): State<AppState>,
    AxumPath(node_id): AxumPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = require_admin_http(&state, &headers) {
        return Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()}));
    }
    let result = {
        let mut store = state.store.lock().await;
        let result = store.rotate_node_key(&node_id);
        if result.is_ok() {
            if let Err(err) = store.save(&state.data_dir).await {
                error!(?err, "failed to save hub store");
            }
        }
        result
    };
    match result {
        Ok(setup_key) => {
            disconnect_node(&state, &node_id).await;
            Json(
                json!({"protocol_version": PROTOCOL_VERSION, "node_id": node_id, "setup_key": setup_key}),
            )
        }
        Err(err) => Json(json!({"protocol_version": PROTOCOL_VERSION, "error": err.to_string()})),
    }
}

async fn enroll_node_http(
    State(state): State<AppState>,
    Json(body): Json<EnrollRequest>,
) -> impl IntoResponse {
    if body.protocol_version != PROTOCOL_VERSION {
        return Json(
            json!({"type": "error", "error": format!("unsupported protocol_version {}; expected {}", body.protocol_version, PROTOCOL_VERSION)}),
        );
    }
    let mut store = state.store.lock().await;
    match store.enroll_node(body, &state.public_url) {
        Ok(response) => {
            if let Err(err) = store.save(&state.data_dir).await {
                error!(?err, "failed to save hub store");
            }
            Json(json!(response))
        }
        Err(err) => Json(json!({"type": "error", "error": err.to_string()})),
    }
}

async fn node_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_node_socket(socket, state))
}

async fn client_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_client_socket(socket, state))
}

async fn handle_node_socket(socket: WebSocket, state: AppState) {
    let (mut sink, mut stream) = socket.split();
    let Some(Ok(Message::Text(text))) = stream.next().await else {
        return;
    };
    let hello: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return,
    };
    if let Err(err) = require_protocol_version(&hello) {
        warn!(%err, "node websocket protocol mismatch");
        return;
    }
    let node_id = hello
        .get("node_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if hello.get("type").and_then(Value::as_str) != Some("node.hello") {
        warn!(%node_id, "node websocket expected node.hello");
        return;
    }

    let nonce = new_nonce();
    let expires_at = now_secs() + 60;
    let challenge = ChallengeMessage {
        kind: "node.challenge",
        protocol_version: PROTOCOL_VERSION,
        nonce: nonce.clone(),
        expires_at,
    };
    if sink
        .send(Message::Text(json!(challenge).to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    let Some(Ok(Message::Text(auth_text))) = stream.next().await else {
        return;
    };
    if let Err(err) = authenticate_node(&state, &node_id, &nonce, expires_at, &auth_text).await {
        warn!(%node_id, %err, "node websocket auth failed");
        return;
    }
    if sink
        .send(Message::Text(
            json!({"protocol_version": PROTOCOL_VERSION, "type":"node.authenticated", "heartbeat_interval_ms":30000})
                .to_string()
                .into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    state.node_channels.lock().await.insert(node_id.clone(), tx);
    let writer = tokio::spawn(write_socket(sink, rx));

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(text)) => handle_node_message(&state, &node_id, &text).await,
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    state.node_channels.lock().await.remove(&node_id);
    {
        let mut store = state.store.lock().await;
        if let Some(node) = store.nodes.get_mut(&node_id) {
            if node.status == NodeStatus::Online {
                node.status = NodeStatus::Offline;
            }
        }
        let _ = store.save(&state.data_dir).await;
    }
    writer.abort();
}

async fn handle_client_socket(socket: WebSocket, state: AppState) {
    let (sink, mut stream) = socket.split();
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let client_id = Uuid::new_v4().to_string();
    state
        .client_channels
        .lock()
        .await
        .insert(client_id.clone(), tx);
    let writer = tokio::spawn(write_socket(sink, rx));
    let Some(Ok(Message::Text(auth_text))) = stream.next().await else {
        state.client_channels.lock().await.remove(&client_id);
        writer.abort();
        return;
    };
    let user_id = match authenticate_client(&state, &auth_text).await {
        Ok(user_id) => {
            state
                .client_users
                .lock()
                .await
                .insert(client_id.clone(), user_id.clone());
            send_client_json(
                &state,
                &client_id,
                json!({"protocol_version": PROTOCOL_VERSION, "type":"client.authenticated", "user_id": user_id}),
            )
            .await;
            user_id
        }
        Err(err) => {
            send_client_json(
                &state,
                &client_id,
                json!({"protocol_version": PROTOCOL_VERSION, "type":"error", "error": err.to_string()}),
            )
            .await;
            state.client_channels.lock().await.remove(&client_id);
            writer.abort();
            return;
        }
    };
    while let Some(msg) = stream.next().await {
        let Ok(Message::Text(text)) = msg else {
            continue;
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(err) => {
                send_client_json(
                    &state,
                    &client_id,
                    json!({"protocol_version": PROTOCOL_VERSION, "type":"error", "error": err.to_string()}),
                )
                .await;
                continue;
            }
        };
        if let Err(err) = require_protocol_version(&value) {
            send_client_json(
                &state,
                &client_id,
                json!({"protocol_version": PROTOCOL_VERSION, "type":"error", "error": err.to_string()}),
            )
            .await;
            continue;
        }
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        match value.get("type").and_then(Value::as_str).unwrap_or_default() {
            "hub.status" => send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"hub.status.result", "version": VERSION})).await,
            "node.list" => {
                let store = state.store.lock().await;
                let nodes = store
                    .nodes
                    .values()
                    .filter(|node| store.user_can_access_node(&user_id, &node.node_id))
                    .filter(|node| node.status != NodeStatus::Revoked && node.status != NodeStatus::Disabled)
                    .collect::<Vec<_>>();
                send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"node.list.result", "nodes": nodes })).await;
            }
            "node.get" => {
                let Some(node_id) = value.get("node_id").and_then(Value::as_str) else {
                    send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error":"node_id is required"})).await;
                    continue;
                };
                let store = state.store.lock().await;
                let node = store
                    .nodes
                    .get(node_id)
                    .filter(|node| store.user_can_access_node(&user_id, &node.node_id));
                if let Some(node) = node {
                    send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"node.get.result", "node": node})).await;
                } else {
                    send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error":"node not found or access denied"})).await;
                }
            }
            "session.attach" | "session.subscribe" => {
                match session_key_from_value(&value) {
                    Ok(key) => {
                        if !user_can_access_node(&state, &user_id, &key.node_id).await {
                            send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error":"access denied for node"})).await;
                            continue;
                        }
                        state
                            .subscriptions
                            .lock()
                            .await
                            .entry(client_id.clone())
                            .or_default()
                            .insert(key.clone());
                        send_subscription_snapshot(&state, &client_id, &id, &key).await;
                    }
                    Err(err) => send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error": err.to_string()})).await,
                }
            }
            "session.detach" => {
                match session_key_from_value(&value) {
                    Ok(key) => {
                        if let Some(keys) = state.subscriptions.lock().await.get_mut(&client_id) {
                            keys.remove(&key);
                        }
                        send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"session.detach.result", "node_id": key.node_id, "project_id": key.project_id, "session_id": key.session_id})).await;
                    }
                    Err(err) => send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error": err.to_string()})).await,
                }
            }
            command_type if command_type.starts_with("session.") || command_type.starts_with("project.") => {
                route_client_request_to_node(&state, &client_id, &user_id, id, value).await;
            }
            other => send_client_json(&state, &client_id, json!({"protocol_version": PROTOCOL_VERSION, "id": id, "type":"error", "error": format!("unknown command type: {other}")})).await,
        }
    }
    state.client_channels.lock().await.remove(&client_id);
    state.client_users.lock().await.remove(&client_id);
    state.subscriptions.lock().await.remove(&client_id);
    state
        .in_flight
        .lock()
        .await
        .retain(|_, pending| pending.client_id != client_id);
    writer.abort();
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

fn require_admin_http(state: &AppState, headers: &HeaderMap) -> Result<()> {
    let Some(expected) = &state.admin_token else {
        return Err(anyhow!(
            "admin HTTP API disabled; start hub with --admin-token or PUMPKINPI_ADMIN_TOKEN"
        ));
    };
    let presented = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| {
            headers
                .get("x-pumpkinpi-admin-token")
                .and_then(|value| value.to_str().ok())
        })
        .context("admin token is required")?;
    if hash_secret(presented) != hash_secret(expected) {
        return Err(anyhow!("invalid admin token"));
    }
    Ok(())
}

async fn authenticate_client(state: &AppState, text: &str) -> Result<String> {
    let value: Value = serde_json::from_str(text).context("invalid client auth json")?;
    require_protocol_version(&value)?;
    if value.get("type").and_then(Value::as_str) != Some("client.auth") {
        return Err(anyhow!("first client message must be client.auth"));
    }
    let token = value
        .get("token")
        .and_then(Value::as_str)
        .context("client auth token is required")?;
    let store = state.store.lock().await;
    store
        .users
        .values()
        .find(|user| user.token_hash == hash_secret(token))
        .map(|user| user.user_id.clone())
        .context("invalid client auth token")
}

async fn user_can_access_node(state: &AppState, user_id: &str, node_id: &str) -> bool {
    state
        .store
        .lock()
        .await
        .user_can_access_node(user_id, node_id)
}

async fn route_client_request_to_node(
    state: &AppState,
    client_id: &str,
    user_id: &str,
    external_id: Value,
    mut value: Value,
) {
    let Some(node_id) = value
        .get("node_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        send_client_json(
            state,
            client_id,
            json!({"protocol_version": PROTOCOL_VERSION, "id": external_id, "type":"error", "error":"node_id is required"}),
        )
        .await;
        return;
    };
    if !user_can_access_node(state, user_id, &node_id).await {
        send_client_json(
            state,
            client_id,
            json!({"protocol_version": PROTOCOL_VERSION, "id": external_id, "type":"error", "error":"access denied for node"}),
        )
        .await;
        return;
    }
    if let Some(provider_account_id) = value
        .get("provider_account_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        match provider_env_for_request(state, user_id, &provider_account_id).await {
            Ok(provider_env) => value["provider_env"] = provider_env,
            Err(err) => {
                send_client_json(
                    state,
                    client_id,
                    json!({"protocol_version": PROTOCOL_VERSION, "id": external_id, "type":"error", "error": err.to_string()}),
                )
                .await;
                return;
            }
        }
    }

    let hub_id = format!("hub_{}", Uuid::new_v4().simple());
    value["id"] = Value::String(hub_id.clone());
    value["protocol_version"] = json!(PROTOCOL_VERSION);
    value["origin_client_id"] = Value::String(client_id.to_string());
    value["origin_user_id"] = Value::String(user_id.to_string());
    value["origin_external_id"] = external_id.clone();

    let tx = state.node_channels.lock().await.get(&node_id).cloned();
    if let Some(tx) = tx {
        audit_log(state, json!({"event":"client.request.routed", "user_id": user_id, "client_id": client_id, "node_id": node_id, "type": value.get("type").and_then(Value::as_str)})).await;
        state.in_flight.lock().await.insert(
            hub_id.clone(),
            PendingRequest {
                client_id: client_id.to_string(),
                external_id: external_id.clone(),
            },
        );
        if tx.send(Message::Text(value.to_string().into())).is_err() {
            state.in_flight.lock().await.remove(&hub_id);
            send_client_json(
                state,
                client_id,
                json!({"protocol_version": PROTOCOL_VERSION, "id": external_id, "type":"error", "error":"failed to route request to node"}),
            )
            .await;
        }
    } else {
        send_client_json(
            state,
            client_id,
            json!({"protocol_version": PROTOCOL_VERSION, "id": external_id, "type":"error", "error":"node is offline"}),
        )
        .await;
    }
}

async fn provider_env_for_request(
    state: &AppState,
    user_id: &str,
    provider_account_id: &str,
) -> Result<Value> {
    let store = state.store.lock().await;
    let (provider_id, secret) = store.provider_account_secret(user_id, provider_account_id)?;
    let env_name = provider_api_key_env_name(&provider_id)?;
    Ok(json!({env_name: secret}))
}

fn provider_api_key_env_name(provider_id: &str) -> Result<&'static str> {
    match provider_id {
        "anthropic" => Ok("ANTHROPIC_API_KEY"),
        "openai" => Ok("OPENAI_API_KEY"),
        "google" | "gemini" => Ok("GOOGLE_API_KEY"),
        "xai" => Ok("XAI_API_KEY"),
        "openrouter" => Ok("OPENROUTER_API_KEY"),
        other => Err(anyhow!("unsupported provider API-key env mapping: {other}")),
    }
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

fn merge_inventory_records(existing: &mut Vec<Value>, incoming: Vec<Value>, id_field: &str) {
    for record in incoming {
        let Some(id) = record
            .get(id_field)
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        if let Some(slot) = existing
            .iter_mut()
            .find(|candidate| candidate.get(id_field).and_then(Value::as_str) == Some(id.as_str()))
        {
            let incoming_updated_at = record
                .get("updated_at")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let existing_updated_at = slot.get("updated_at").and_then(Value::as_u64).unwrap_or(0);
            if incoming_updated_at >= existing_updated_at {
                *slot = record;
            }
        } else {
            existing.push(record);
        }
    }
}

fn session_key_from_value(value: &Value) -> Result<SessionKey> {
    Ok(SessionKey {
        node_id: value
            .get("node_id")
            .and_then(Value::as_str)
            .context("node_id is required")?
            .to_string(),
        project_id: value
            .get("project_id")
            .and_then(Value::as_str)
            .context("project_id is required")?
            .to_string(),
        session_id: value
            .get("session_id")
            .and_then(Value::as_str)
            .context("session_id is required")?
            .to_string(),
    })
}

async fn authenticate_node(
    state: &AppState,
    node_id: &str,
    nonce: &str,
    expires_at: u64,
    auth_text: &str,
) -> Result<()> {
    if now_secs() > expires_at {
        return Err(anyhow!("node challenge expired"));
    }
    let auth: Value = serde_json::from_str(auth_text).context("invalid node.auth json")?;
    require_protocol_version(&auth)?;
    if auth.get("type").and_then(Value::as_str) != Some("node.auth") {
        return Err(anyhow!("expected node.auth"));
    }
    if auth.get("node_id").and_then(Value::as_str) != Some(node_id) {
        return Err(anyhow!("node_id mismatch in node.auth"));
    }
    let signature_bytes = decode_base64_array::<64>(
        auth.get("signature")
            .and_then(Value::as_str)
            .context("signature is required")?,
    )?;
    let public_key = {
        let store = state.store.lock().await;
        let node = store
            .nodes
            .get(node_id)
            .filter(|node| {
                node.status != NodeStatus::Revoked && node.status != NodeStatus::Disabled
            })
            .context("node not found or disabled")?;
        node.public_key
            .clone()
            .context("node has no enrolled public key")?
    };
    let public_key_bytes = decode_base64_array::<32>(&public_key)?;
    let verifying_key =
        VerifyingKey::from_bytes(&public_key_bytes).context("invalid node public key")?;
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key
        .verify(nonce.as_bytes(), &signature)
        .context("invalid node signature")?;

    let mut store = state.store.lock().await;
    if let Some(node) = store.nodes.get_mut(node_id) {
        node.status = NodeStatus::Online;
        node.last_seen_at = Some(now_secs());
        for project in &mut node.projects {
            project["status"] = Value::String("stale".to_string());
        }
        for session in &mut node.sessions {
            session["status"] = Value::String("stale".to_string());
        }
    }
    store.save(&state.data_dir).await?;
    audit_log(
        state,
        json!({"event":"node.authenticated", "node_id": node_id}),
    )
    .await;
    Ok(())
}

async fn handle_node_message(state: &AppState, node_id: &str, text: &str) {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if let Err(err) = require_protocol_version(&value) {
        warn!(%node_id, %err, "dropping node message with protocol mismatch");
        return;
    }
    match value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "node.inventory" => {
            let mut terminal_events = Vec::new();
            {
                let mut store = state.store.lock().await;
                if let Some(node) = store.nodes.get_mut(node_id) {
                    let complete = value
                        .get("complete")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let revision = value
                        .get("revision")
                        .and_then(Value::as_u64)
                        .unwrap_or_else(now_secs);
                    let projects = value
                        .get("projects")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let sessions = value
                        .get("sessions")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    if complete {
                        let incoming_project_ids = projects
                            .iter()
                            .filter_map(|project| {
                                project
                                    .get("project_id")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .collect::<std::collections::HashSet<_>>();
                        let incoming_session_ids = sessions
                            .iter()
                            .filter_map(|session| {
                                session
                                    .get("session_id")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .collect::<std::collections::HashSet<_>>();

                        let mut reconciled_projects = projects;
                        for old in &node.projects {
                            let Some(project_id) = old.get("project_id").and_then(Value::as_str)
                            else {
                                continue;
                            };
                            if !incoming_project_ids.contains(project_id) {
                                let mut stale = old.clone();
                                stale["status"] = Value::String("stale".to_string());
                                stale["updated_at"] = json!(revision);
                                reconciled_projects.push(stale);
                                terminal_events.push(json!({
                                    "protocol_version": PROTOCOL_VERSION,
                                    "type":"project.stale",
                                    "node_id": node_id,
                                    "project_id": project_id,
                                    "reason":"absent_from_complete_inventory"
                                }));
                            } else if let Some(new_project) =
                                reconciled_projects.iter().find(|project| {
                                    project.get("project_id").and_then(Value::as_str)
                                        == Some(project_id)
                                })
                            {
                                if old.get("cwd") != new_project.get("cwd")
                                    || old.get("name") != new_project.get("name")
                                {
                                    terminal_events.push(json!({
                                        "protocol_version": PROTOCOL_VERSION,
                                        "type":"project.updated",
                                        "node_id": node_id,
                                        "project_id": project_id,
                                        "project": new_project
                                    }));
                                }
                            }
                        }

                        let mut reconciled_sessions = sessions;
                        for old in &node.sessions {
                            let Some(session_id) = old.get("session_id").and_then(Value::as_str)
                            else {
                                continue;
                            };
                            if !incoming_session_ids.contains(session_id) {
                                let project_id = old
                                    .get("project_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();
                                let mut stale = old.clone();
                                stale["status"] = Value::String("stale".to_string());
                                stale["updated_at"] = json!(revision);
                                reconciled_sessions.push(stale);
                                terminal_events.push(json!({
                                    "protocol_version": PROTOCOL_VERSION,
                                    "type":"session.stale",
                                    "node_id": node_id,
                                    "project_id": project_id,
                                    "session_id": session_id,
                                    "reason":"absent_from_complete_inventory"
                                }));
                            }
                        }
                        node.projects = reconciled_projects;
                        node.sessions = reconciled_sessions;
                    } else {
                        merge_inventory_records(&mut node.projects, projects, "project_id");
                        merge_inventory_records(&mut node.sessions, sessions, "session_id");
                    }
                    node.inventory_revision = Some(revision);
                    node.last_seen_at = Some(now_secs());
                }
                let _ = store.save(&state.data_dir).await;
            }
            for event in terminal_events {
                let key = session_key_from_value(&event).ok();
                if key.is_some() {
                    deliver_session_event(state, event).await;
                } else {
                    deliver_node_event(state, event).await;
                }
                if let Some(key) = key {
                    state.subscriptions.lock().await.retain(|_, keys| {
                        keys.remove(&key);
                        !keys.is_empty()
                    });
                }
            }
        }
        "node.heartbeat" => {
            let mut store = state.store.lock().await;
            if let Some(node) = store.nodes.get_mut(node_id) {
                node.last_seen_at = Some(now_secs());
            }
            let _ = store.save(&state.data_dir).await;
        }
        _ => {
            let mut event = value;
            if event.get("node_id").is_none() {
                event["node_id"] = Value::String(node_id.to_string());
            }
            if let Some(hub_id) = event.get("id").and_then(Value::as_str).map(str::to_string) {
                if let Some(pending) = state.in_flight.lock().await.remove(&hub_id) {
                    event["id"] = pending.external_id;
                    send_client_json(state, &pending.client_id, event).await;
                    return;
                }
            }
            if let Some(target_client_id) = event
                .get("target_client_id")
                .and_then(Value::as_str)
                .map(str::to_string)
            {
                if state
                    .client_channels
                    .lock()
                    .await
                    .contains_key(&target_client_id)
                {
                    send_client_json(state, &target_client_id, event).await;
                    return;
                }
            }
            deliver_session_event(state, event).await;
        }
    }
}

async fn write_socket(
    mut sink: SplitSink<WebSocket, Message>,
    mut rx: mpsc::UnboundedReceiver<Message>,
) {
    while let Some(msg) = rx.recv().await {
        if sink.send(msg).await.is_err() {
            break;
        }
    }
}

async fn disconnect_node(state: &AppState, node_id: &str) {
    if let Some(tx) = state.node_channels.lock().await.remove(node_id) {
        let _ = tx.send(Message::Close(None));
    }
    let affected = {
        let subscriptions = state.subscriptions.lock().await;
        subscriptions
            .iter()
            .flat_map(|(_, keys)| keys.iter().filter(|key| key.node_id == node_id).cloned())
            .collect::<std::collections::HashSet<_>>()
    };
    for key in affected {
        deliver_session_event(
            state,
            json!({
                "protocol_version": PROTOCOL_VERSION,
                "type":"session.revoked",
                "node_id": key.node_id,
                "project_id": key.project_id,
                "session_id": key.session_id,
                "reason":"node_disconnected_or_revoked"
            }),
        )
        .await;
    }
    state.subscriptions.lock().await.retain(|_, keys| {
        keys.retain(|key| key.node_id != node_id);
        !keys.is_empty()
    });
}

async fn send_client_json(state: &AppState, client_id: &str, value: Value) {
    if let Some(tx) = state.client_channels.lock().await.get(client_id) {
        let _ = tx.send(Message::Text(value.to_string().into()));
    }
}

async fn audit_log(state: &AppState, mut event: Value) {
    event["at"] = json!(now_secs());
    let redacted = redact_audit_event(event);
    let path = state.data_dir.join("audit.log");
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut line = redacted.to_string();
    line.push('\n');
    use tokio::io::AsyncWriteExt;
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        let _ = file.write_all(line.as_bytes()).await;
    }
}

fn redact_audit_event(event: Value) -> Value {
    redact_sensitive_value(event)
}

async fn send_subscription_snapshot(
    state: &AppState,
    client_id: &str,
    id: &Value,
    key: &SessionKey,
) {
    let (project, session) = {
        let store = state.store.lock().await;
        let node = store.nodes.get(&key.node_id);
        let project = node.and_then(|node| {
            node.projects
                .iter()
                .find(|project| {
                    project.get("project_id").and_then(Value::as_str)
                        == Some(key.project_id.as_str())
                })
                .cloned()
        });
        let session = node.and_then(|node| {
            node.sessions
                .iter()
                .find(|session| {
                    session.get("session_id").and_then(Value::as_str)
                        == Some(key.session_id.as_str())
                })
                .cloned()
        });
        (project, session)
    };
    let recent_events = match read_recent_session_events(&state.data_dir, key, 256).await {
        Ok(events) if !events.is_empty() => events,
        _ => state
            .recent_events
            .lock()
            .await
            .get(key)
            .map(|events| events.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    };
    send_client_json(
        state,
        client_id,
        json!({
            "protocol_version": PROTOCOL_VERSION,
            "id": id,
            "type":"session.subscribe.result",
            "node_id": key.node_id,
            "project_id": key.project_id,
            "session_id": key.session_id,
            "project": project,
            "session": session,
            "recent_events": recent_events,
        }),
    )
    .await;
}

async fn deliver_node_event(state: &AppState, value: Value) {
    let Some(node_id) = value
        .get("node_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return;
    };
    let recipients = {
        let client_users = state.client_users.lock().await;
        let store = state.store.lock().await;
        client_users
            .iter()
            .filter_map(|(client_id, user_id)| {
                store
                    .user_can_access_node(user_id, &node_id)
                    .then_some(client_id.clone())
            })
            .collect::<Vec<_>>()
    };
    let clients = state.client_channels.lock().await;
    let message = Message::Text(value.to_string().into());
    for client_id in recipients {
        if let Some(tx) = clients.get(&client_id) {
            let _ = tx.send(message.clone());
        }
    }
}

async fn deliver_session_event(state: &AppState, value: Value) {
    let Ok(key) = session_key_from_value(&value) else {
        return;
    };
    {
        let mut recent = state.recent_events.lock().await;
        let buffer = recent.entry(key.clone()).or_default();
        if buffer.len() == 256 {
            buffer.pop_front();
        }
        buffer.push_back(value.clone());
    }
    append_session_event_log(&state.data_dir, &key, &value).await;
    let recipient_ids = {
        let subscriptions = state.subscriptions.lock().await;
        subscriptions
            .iter()
            .filter_map(|(client_id, keys)| keys.contains(&key).then_some(client_id.clone()))
            .collect::<Vec<_>>()
    };
    let message = Message::Text(value.to_string().into());
    let clients = state.client_channels.lock().await;
    for client_id in recipient_ids {
        if let Some(tx) = clients.get(&client_id) {
            let _ = tx.send(message.clone());
        }
    }
}

async fn append_session_event_log(data_dir: &Path, key: &SessionKey, value: &Value) {
    let path = session_event_log_path(data_dir, key);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut line = redact_sensitive_value(value.clone()).to_string();
    line.push('\n');
    use tokio::io::AsyncWriteExt;
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        let _ = file.write_all(line.as_bytes()).await;
    }
}

async fn read_recent_session_events(
    data_dir: &Path,
    key: &SessionKey,
    limit: usize,
) -> Result<Vec<Value>> {
    let path = session_event_log_path(data_dir, key);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = tokio::fs::read_to_string(path).await?;
    let mut events = VecDeque::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        if events.len() == limit {
            events.pop_front();
        }
        events.push_back(value);
    }
    Ok(events.into_iter().collect())
}

fn session_event_log_path(data_dir: &Path, key: &SessionKey) -> PathBuf {
    data_dir
        .join("events")
        .join(safe_path_component(&key.node_id))
        .join(safe_path_component(&key.project_id))
        .join(format!("{}.jsonl", safe_path_component(&key.session_id)))
}

fn safe_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

impl HubStore {
    async fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("hub-state.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = tokio::fs::read(&path)
            .await
            .with_context(|| format!("read {}", path.display()))?;
        Ok(serde_json::from_slice(&data).with_context(|| format!("parse {}", path.display()))?)
    }

    async fn save(&self, data_dir: &Path) -> Result<()> {
        tokio::fs::create_dir_all(data_dir).await?;
        let path = data_dir.join("hub-state.json");
        let data = serde_json::to_vec_pretty(self)?;
        tokio::fs::write(&path, data)
            .await
            .with_context(|| format!("write {}", path.display()))
    }

    fn create_user(&mut self, username: String) -> (UserRecord, String) {
        let user_id = format!("user_{}", Uuid::new_v4().simple());
        let token = new_secret("ppu");
        let now = now_secs();
        let user = UserRecord {
            user_id: user_id.clone(),
            username,
            token_hash: hash_secret(&token),
            auth_identities: Vec::new(),
            client_preferences: serde_json::Map::new(),
            recently_used: serde_json::Map::new(),
            provider_preferences: serde_json::Map::new(),
            default_session_settings: serde_json::Map::new(),
            audit_metadata: serde_json::Map::new(),
            created_at: now,
            updated_at: now,
        };
        self.users.insert(user_id, user.clone());
        (user, token)
    }

    fn grant_node_access(&mut self, user_id: &str, node_id: &str) -> Result<()> {
        self.users.get(user_id).context("user not found")?;
        self.nodes.get(node_id).context("node not found")?;
        if !self.user_can_access_node(user_id, node_id) {
            self.node_access_grants.push(NodeAccessGrant {
                user_id: user_id.to_string(),
                node_id: node_id.to_string(),
                created_at: now_secs(),
            });
        }
        Ok(())
    }

    fn user_can_access_node(&self, user_id: &str, node_id: &str) -> bool {
        let node_accessible = self.nodes.get(node_id).is_some_and(|node| {
            node.status != NodeStatus::Revoked && node.status != NodeStatus::Disabled
        });
        node_accessible
            && self
                .node_access_grants
                .iter()
                .any(|grant| grant.user_id == user_id && grant.node_id == node_id)
    }

    fn add_provider_api_key(
        &mut self,
        user_id: String,
        provider_id: String,
        display_name: String,
        api_key: String,
    ) -> Result<ProviderAccountRecord> {
        self.users.get(&user_id).context("user not found")?;
        let now = now_secs();
        let account = ProviderAccountRecord {
            provider_account_id: format!("provacct_{}", Uuid::new_v4().simple()),
            user_id,
            provider_id,
            display_name,
            auth_type: "api_key".to_string(),
            encrypted_secret: encrypt_secret(&api_key)?,
            available_models: Vec::new(),
            default_model: None,
            created_at: now,
            updated_at: now,
            revoked_at: None,
        };
        self.provider_accounts
            .insert(account.provider_account_id.clone(), account.clone());
        Ok(account)
    }

    fn revoke_provider_account(&mut self, provider_account_id: &str) -> Result<()> {
        let account = self
            .provider_accounts
            .get_mut(provider_account_id)
            .context("provider account not found")?;
        account.revoked_at = Some(now_secs());
        account.updated_at = now_secs();
        Ok(())
    }

    fn provider_account_secret(
        &self,
        user_id: &str,
        provider_account_id: &str,
    ) -> Result<(String, String)> {
        let account = self
            .provider_accounts
            .get(provider_account_id)
            .context("provider account not found")?;
        if account.user_id != user_id {
            return Err(anyhow!("provider account does not belong to user"));
        }
        if account.revoked_at.is_some() {
            return Err(anyhow!("provider account is revoked"));
        }
        Ok((
            account.provider_id.clone(),
            decrypt_secret(&account.encrypted_secret)?,
        ))
    }

    fn create_node(&mut self, name: String) -> (NodeRecord, String) {
        let node_id = format!("node_{}", Uuid::new_v4().simple());
        let setup_key = new_secret("ppn_setup");
        let now = now_secs();
        let node = NodeRecord {
            node_id: node_id.clone(),
            name,
            hostname: None,
            version: None,
            status: NodeStatus::Offline,
            setup_key_hash: Some(hash_secret(&setup_key)),
            setup_key_expires_at: Some(now + SETUP_KEY_TTL_SECS),
            token_hash: None,
            public_key: None,
            capabilities: vec!["ed25519-challenge-auth".into()],
            projects: vec![],
            sessions: vec![],
            inventory_revision: None,
            created_at: now,
            enrolled_at: None,
            last_seen_at: None,
            revoked_at: None,
        };
        self.nodes.insert(node_id, node.clone());
        (node, setup_key)
    }

    fn issue_setup_key(&mut self, node_id: &str) -> Result<String> {
        let node = self.nodes.get_mut(node_id).context("node not found")?;
        if node.status == NodeStatus::Revoked {
            return Err(anyhow!("node is revoked"));
        }
        let setup_key = new_secret("ppn_setup");
        node.setup_key_hash = Some(hash_secret(&setup_key));
        node.setup_key_expires_at = Some(now_secs() + SETUP_KEY_TTL_SECS);
        Ok(setup_key)
    }

    fn revoke_node(&mut self, node_id: &str) -> Result<()> {
        let node = self.nodes.get_mut(node_id).context("node not found")?;
        node.status = NodeStatus::Revoked;
        node.revoked_at = Some(now_secs());
        node.setup_key_hash = None;
        node.setup_key_expires_at = None;
        node.token_hash = None;
        node.public_key = None;
        Ok(())
    }

    fn disable_node(&mut self, node_id: &str) -> Result<()> {
        let node = self.nodes.get_mut(node_id).context("node not found")?;
        if node.status != NodeStatus::Revoked {
            node.status = NodeStatus::Disabled;
        }
        Ok(())
    }

    fn rotate_node_key(&mut self, node_id: &str) -> Result<String> {
        let setup_key = self.issue_setup_key(node_id)?;
        let node = self.nodes.get_mut(node_id).context("node not found")?;
        node.public_key = None;
        node.token_hash = None;
        if node.status != NodeStatus::Revoked {
            node.status = NodeStatus::Offline;
        }
        Ok(setup_key)
    }

    fn enroll_node(&mut self, body: EnrollRequest, hub_url: &str) -> Result<EnrollResponse> {
        if let Some(kind) = &body.kind {
            if kind != "node.enroll" {
                return Err(anyhow!("expected type node.enroll"));
            }
        }
        let now = now_secs();
        let node = self
            .nodes
            .values_mut()
            .find(|node| {
                node.setup_key_hash.as_deref() == Some(hash_secret(&body.setup_key).as_str())
                    && node
                        .setup_key_expires_at
                        .is_some_and(|expires| expires >= now)
            })
            .context("invalid or expired setup key")?;
        if node.status == NodeStatus::Revoked || node.status == NodeStatus::Disabled {
            return Err(anyhow!("node is not enrollable"));
        }
        let public_key = body.public_key.context("public_key is required")?;
        decode_base64_array::<32>(&public_key)
            .context("public_key must be base64 ed25519 public key bytes")?;
        node.hostname = body.hostname;
        node.version = body.version;
        node.public_key = Some(public_key);
        node.token_hash = None;
        node.setup_key_hash = None;
        node.setup_key_expires_at = None;
        node.enrolled_at = Some(now);
        node.last_seen_at = Some(now);
        Ok(EnrollResponse {
            kind: "node.enrolled",
            node_id: node.node_id.clone(),
            hub_url: hub_url.to_string(),
        })
    }
}

fn default_hub_data_dir() -> PathBuf {
    std::env::var_os("PUMPKINPI_HUB_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".pumpkinpi-hub")))
        .unwrap_or_else(|| PathBuf::from(".pumpkinpi-hub"))
}

fn new_secret(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn new_nonce() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    BASE64.encode(bytes)
}

fn master_cipher() -> Result<Aes256Gcm> {
    let encoded = std::env::var("PUMPKINPI_MASTER_KEY").context(
        "PUMPKINPI_MASTER_KEY must be set to base64 32-byte key for provider secret encryption",
    )?;
    let key = decode_base64_array::<32>(&encoded)?;
    Ok(Aes256Gcm::new_from_slice(&key).expect("32-byte AES key"))
}

fn encrypt_secret(secret: &str) -> Result<EncryptedSecret> {
    let cipher = master_cipher()?;
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), secret.as_bytes())
        .map_err(|_| anyhow!("provider secret encryption failed"))?;
    Ok(EncryptedSecret {
        nonce: BASE64.encode(nonce),
        ciphertext: BASE64.encode(ciphertext),
    })
}

fn decrypt_secret(secret: &EncryptedSecret) -> Result<String> {
    let cipher = master_cipher()?;
    let nonce = decode_base64_array::<12>(&secret.nonce)?;
    let ciphertext = BASE64
        .decode(&secret.ciphertext)
        .context("invalid ciphertext base64")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| anyhow!("provider secret decryption failed"))?;
    String::from_utf8(plaintext).context("provider secret is not utf-8")
}

fn decode_base64_array<const N: usize>(value: &str) -> Result<[u8; N]> {
    let bytes = BASE64.decode(value).context("invalid base64")?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| anyhow!("expected {N} bytes, got {}", bytes.len()))
}

fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

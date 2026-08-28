use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local, TimeZone, Utc};
use dioxus::prelude::*;
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::*;
use serde_json::Value;
use tokio::sync::watch;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::client_config;

const STYLES: &str = include_str!("../assets/styles.css");

#[derive(Debug, Clone)]
enum ConnectionKind {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Reconnecting,
    Degraded,
}

impl ConnectionKind {
    fn class(&self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Authenticating => "authenticating",
            Self::Connected => "connected",
            Self::Reconnecting => "reconnecting",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Debug, Clone)]
struct ConnectionView {
    kind: ConnectionKind,
    label: String,
    attempt: u32,
    reason: Option<String>,
}

impl ConnectionView {
    fn disconnected() -> Self {
        Self {
            kind: ConnectionKind::Disconnected,
            label: "Disconnected".into(),
            attempt: 0,
            reason: None,
        }
    }

    fn simple(kind: ConnectionKind, label: &str) -> Self {
        Self {
            kind,
            label: label.into(),
            attempt: 0,
            reason: None,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingMessage {
    spoke_id: SpokeId,
    project_id: ProjectId,
    local_id: String,
    message: String,
    created_at: u64,
}

#[derive(Debug, Clone)]
struct DiagnosticView {
    message: String,
    created_at: u64,
    sequence: u64,
}

#[derive(Debug, Clone)]
struct InteractionView {
    spoke_id: SpokeId,
    project_id: ProjectId,
    operation_id: OperationId,
    request_id: String,
    method: String,
    payload: Value,
    created_at: u64,
}

#[derive(Debug, Clone)]
struct UiState {
    connection: ConnectionView,
    configured: bool,
    hub_url: String,
    spokes: Vec<SpokeRecord>,
    projects: Vec<ProjectRecord>,
    selected: Option<ProjectKey>,
    selected_snapshot: Option<ProjectSnapshot>,
    pending_messages: Vec<PendingMessage>,
    interactions: Vec<InteractionView>,
    path_suggestions: BTreeMap<SpokeId, Vec<String>>,
    diagnostics: Vec<DiagnosticView>,
}

struct AppStore {
    connection: ConnectionView,
    configured: bool,
    hub_url: String,
    spokes: BTreeMap<SpokeId, SpokeRecord>,
    projects: BTreeMap<ProjectKey, ProjectRecord>,
    snapshots: BTreeMap<ProjectKey, ProjectSnapshot>,
    selected: Option<ProjectKey>,
    pending_messages: Vec<PendingMessage>,
    interactions: BTreeMap<(OperationId, String), InteractionView>,
    path_suggestions: BTreeMap<SpokeId, Vec<String>>,
    diagnostics: Vec<DiagnosticView>,
    next_diagnostic_sequence: u64,
    pending_local_project: Option<(String, Option<String>)>,
}

impl Default for AppStore {
    fn default() -> Self {
        let config = client_config::load().unwrap_or_default();
        Self {
            connection: ConnectionView::disconnected(),
            configured: client_config::resolve_token().ok().flatten().is_some(),
            hub_url: config.hub,
            spokes: BTreeMap::new(),
            projects: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            selected: None,
            pending_messages: Vec::new(),
            interactions: BTreeMap::new(),
            path_suggestions: BTreeMap::new(),
            diagnostics: Vec::new(),
            next_diagnostic_sequence: 0,
            pending_local_project: None,
        }
    }
}

impl AppStore {
    fn view(&self) -> UiState {
        let mut selected_snapshot = self
            .selected
            .as_ref()
            .and_then(|key| self.snapshots.get(key).cloned());
        if let Some(snapshot) = &mut selected_snapshot {
            order_snapshot_collections(snapshot);
        }
        let mut interactions: Vec<_> = self.interactions.values().cloned().collect();
        interactions.sort_by(|a, b| {
            (a.created_at, &a.operation_id, &a.request_id).cmp(&(
                b.created_at,
                &b.operation_id,
                &b.request_id,
            ))
        });
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.sort_by(|a, b| {
            (a.created_at, &a.message, a.sequence).cmp(&(b.created_at, &b.message, b.sequence))
        });
        if diagnostics.len() > 100 {
            diagnostics.drain(..diagnostics.len() - 100);
        }
        UiState {
            connection: self.connection.clone(),
            configured: self.configured,
            hub_url: self.hub_url.clone(),
            spokes: self.spokes.values().cloned().collect(),
            projects: self.projects.values().cloned().collect(),
            selected: self.selected.clone(),
            selected_snapshot,
            pending_messages: self.pending_messages.clone(),
            interactions,
            path_suggestions: self.path_suggestions.clone(),
            diagnostics,
        }
    }

    fn diagnostic(&mut self, message: impl Into<String>) {
        let created_at = Utc::now().timestamp().try_into().unwrap_or_default();
        self.diagnostic_at(created_at, message);
    }

    fn diagnostic_at(&mut self, created_at: u64, message: impl Into<String>) {
        let sequence = self.next_diagnostic_sequence;
        self.next_diagnostic_sequence = self.next_diagnostic_sequence.saturating_add(1);
        self.diagnostics.push(DiagnosticView {
            message: message.into(),
            created_at,
            sequence,
        });
        if self.diagnostics.len() > 500 {
            self.diagnostics.drain(..100);
        }
    }
}

enum ActorCommand {
    Request(ClientCommand),
    Stop,
}

#[derive(Clone)]
struct GuiRuntime {
    store: Arc<Mutex<AppStore>>,
    actor: Arc<Mutex<Option<mpsc::Sender<ActorCommand>>>>,
    updates: watch::Sender<UiState>,
}

impl Default for GuiRuntime {
    fn default() -> Self {
        let store = AppStore::default();
        let (updates, _) = watch::channel(store.view());
        Self {
            store: Arc::new(Mutex::new(store)),
            actor: Arc::new(Mutex::new(None)),
            updates,
        }
    }
}

impl GuiRuntime {
    fn view(&self) -> UiState {
        self.store.lock().expect("store lock poisoned").view()
    }

    fn subscribe(&self) -> watch::Receiver<UiState> {
        self.updates.subscribe()
    }

    fn emit(&self) {
        self.updates.send_replace(self.view());
    }

    fn diagnostic(&self, message: impl Into<String>) {
        self.store
            .lock()
            .expect("store lock poisoned")
            .diagnostic(message);
        self.emit();
    }

    fn set_connection(&self, connection: ConnectionView) {
        self.store.lock().expect("store lock poisoned").connection = connection;
        self.emit();
    }

    fn request(&self, command: ClientCommand) -> std::result::Result<(), String> {
        self.actor
            .lock()
            .map_err(|_| "protocol actor lock poisoned".to_string())?
            .as_ref()
            .ok_or_else(|| "not connected".to_string())?
            .send(ActorCommand::Request(command))
            .map_err(|_| "protocol actor stopped".to_string())
    }

    fn dispatch(&self, command: ClientCommand) {
        if let Err(error) = self.request(command) {
            self.diagnostic(error);
        }
    }

    fn connect(&self) {
        if let Some(actor) = self.actor.lock().expect("actor lock poisoned").take() {
            let _ = actor.send(ActorCommand::Stop);
        }
        let (tx, rx) = mpsc::channel();
        *self.actor.lock().expect("actor lock poisoned") = Some(tx);
        let runtime = self.clone();
        thread::spawn(move || match tokio::runtime::Runtime::new() {
            Ok(tokio) => tokio.block_on(protocol_actor(runtime, rx)),
            Err(error) => runtime.set_connection(ConnectionView {
                kind: ConnectionKind::Degraded,
                label: "Runtime failed".into(),
                attempt: 0,
                reason: Some(error.to_string()),
            }),
        });
    }

    fn login(&self, hub: String, token: String) {
        match client_config::login(hub.clone(), token) {
            Ok(_) => {
                let mut store = self.store.lock().expect("store lock poisoned");
                store.configured = true;
                store.hub_url = hub;
                drop(store);
                self.emit();
                self.connect();
            }
            Err(error) => self.diagnostic(format!("Login failed: {error}")),
        }
    }

    fn open_local_project(&self, path: PathBuf) {
        let cwd = path.to_string_lossy().into_owned();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned);
        self.store
            .lock()
            .expect("store lock poisoned")
            .pending_local_project = Some((cwd, name));
        self.set_connection(ConnectionView::simple(
            ConnectionKind::Connecting,
            "Starting local workspace",
        ));

        let runtime = self.clone();
        thread::spawn(move || match bootstrap_local_services() {
            Ok((hub, token)) => {
                if let Err(error) = client_config::login(hub.clone(), token) {
                    runtime.diagnostic(format!("Could not save local connection: {error}"));
                    return;
                }
                {
                    let mut store = runtime.store.lock().expect("store lock poisoned");
                    store.configured = true;
                    store.hub_url = hub;
                    store.diagnostic("Local Hub and Spoke are ready");
                }
                runtime.emit();
                runtime.connect();
            }
            Err(error) => runtime.set_connection(ConnectionView {
                kind: ConnectionKind::Degraded,
                label: "Local setup failed".into(),
                attempt: 0,
                reason: Some(error.to_string()),
            }),
        });
    }

    fn refresh(&self) {
        self.dispatch(ClientCommand::SpokeList);
        self.dispatch(ClientCommand::ProjectList { spoke_id: None });
    }

    fn select_project(&self, key: ProjectKey) {
        let cursor = {
            let mut store = self.store.lock().expect("store lock poisoned");
            store.selected = Some(key.clone());
            store
                .snapshots
                .get(&key)
                .and_then(|snapshot| snapshot.timeline.iter().map(|item| item.cursor).max())
        };
        self.emit();
        self.dispatch(ClientCommand::IntentSubscribe {
            spoke_id: key.spoke_id,
            project_id: key.project_id,
            cursor,
        });
    }

    fn initialize_project(&self, spoke_id: SpokeId, cwd: String, name: String) {
        if cwd.trim().is_empty() {
            self.diagnostic("A project path is required");
            return;
        }
        self.dispatch(ClientCommand::ProjectInitialize {
            spoke_id,
            cwd,
            name: (!name.trim().is_empty()).then_some(name),
        });
    }

    fn send_intent(&self, key: ProjectKey, message: String) {
        if message.trim().is_empty() {
            return;
        }
        let revision = {
            let mut store = self.store.lock().expect("store lock poisoned");
            let revision = store
                .snapshots
                .get(&key)
                .map(|snapshot| snapshot.source.revision);
            store.pending_messages.push(PendingMessage {
                spoke_id: key.spoke_id.clone(),
                project_id: key.project_id.clone(),
                local_id: Uuid::new_v4().to_string(),
                message: message.clone(),
                created_at: Utc::now().timestamp().try_into().unwrap_or_default(),
            });
            revision
        };
        self.emit();
        self.dispatch(ClientCommand::IntentSend {
            spoke_id: key.spoke_id,
            project_id: key.project_id,
            message,
            expected_revision: revision,
        });
    }

    fn cancel(&self, operation: &OperationRecord) {
        self.dispatch(ClientCommand::IntentCancel {
            spoke_id: operation.spoke_id.clone(),
            project_id: operation.project_id.clone(),
            operation_id: operation.operation_id.clone(),
        });
    }

    fn answer(&self, interaction: &InteractionView, response: Value) {
        self.dispatch(ClientCommand::IntentAnswer {
            spoke_id: interaction.spoke_id.clone(),
            project_id: interaction.project_id.clone(),
            operation_id: interaction.operation_id.clone(),
            request_id: interaction.request_id.clone(),
            response,
        });
        self.store
            .lock()
            .expect("store lock poisoned")
            .interactions
            .remove(&(
                interaction.operation_id.clone(),
                interaction.request_id.clone(),
            ));
        self.emit();
    }

    fn projection(&self, key: &ProjectKey) {
        self.dispatch(ClientCommand::IntentGetProjection {
            spoke_id: key.spoke_id.clone(),
            project_id: key.project_id.clone(),
        });
    }

    fn apply(&self, event: ClientEvent) {
        let mut store = self.store.lock().expect("store lock poisoned");
        let mut follow_up = None;
        let event_created_at = event.created_at;
        match event.payload {
            ClientPayload::Authenticated => {}
            ClientPayload::SpokeList { spokes } => {
                store.spokes = spokes
                    .into_iter()
                    .map(|spoke| (spoke.spoke_id.clone(), spoke))
                    .collect();
                if let Some((cwd, name)) = store.pending_local_project.clone()
                    && let Some(spoke) = store
                        .spokes
                        .values()
                        .find(|spoke| spoke.status == SpokeStatus::Online)
                {
                    follow_up = Some(ClientCommand::ProjectInitialize {
                        spoke_id: spoke.spoke_id.clone(),
                        cwd,
                        name,
                    });
                    store.pending_local_project = None;
                }
            }
            ClientPayload::ProjectList { projects } => {
                store.projects.clear();
                for project in projects {
                    store.projects.insert(project_key(&project), project);
                }
            }
            ClientPayload::ProjectSnapshot { snapshot } => {
                let snapshot = *snapshot;
                let key = project_key(&snapshot.project);
                store.projects.insert(key.clone(), snapshot.project.clone());
                store
                    .snapshots
                    .entry(key.clone())
                    .and_modify(|old| merge_snapshot(old, &snapshot))
                    .or_insert(snapshot);
                store.selected = Some(key.clone());
                store.pending_messages.retain(|pending| {
                    pending.spoke_id != key.spoke_id || pending.project_id != key.project_id
                });
            }
            ClientPayload::Timeline { item } => {
                let key = ProjectKey {
                    spoke_id: item.spoke_id.clone(),
                    project_id: item.project_id.clone(),
                };
                if let Some(snapshot) = store.snapshots.get_mut(&key)
                    && !snapshot
                        .timeline
                        .iter()
                        .any(|existing| existing.timeline_item_id == item.timeline_item_id)
                {
                    snapshot.timeline.push(item);
                    order_snapshot_collections(snapshot);
                }
                store.pending_messages.retain(|pending| {
                    pending.spoke_id != key.spoke_id || pending.project_id != key.project_id
                });
            }
            ClientPayload::Accepted { operation } | ClientPayload::Operation { operation } => {
                let key = ProjectKey {
                    spoke_id: operation.spoke_id.clone(),
                    project_id: operation.project_id.clone(),
                };
                if let Some(snapshot) = store.snapshots.get_mut(&key) {
                    if let Some(existing) = snapshot
                        .operations
                        .iter_mut()
                        .find(|existing| existing.operation_id == operation.operation_id)
                    {
                        *existing = operation;
                    } else {
                        snapshot.operations.push(operation);
                    }
                }
            }
            ClientPayload::Interaction {
                spoke_id,
                project_id,
                operation_id,
                request_id,
                method,
                payload,
            } => {
                let interaction = InteractionView {
                    spoke_id,
                    project_id,
                    operation_id: operation_id.clone(),
                    request_id: request_id.clone(),
                    method,
                    payload,
                    created_at: event_created_at,
                };
                store
                    .interactions
                    .insert((operation_id, request_id), interaction);
            }
            ClientPayload::ProjectUpdated { project } => {
                let key = project_key(&project);
                if project.status == ProjectStatus::Removed {
                    store.projects.remove(&key);
                    store.snapshots.remove(&key);
                    if store.selected.as_ref() == Some(&key) {
                        store.selected = None;
                    }
                } else {
                    store.projects.insert(key, project);
                }
            }
            ClientPayload::SpokeUpdated { spoke } => {
                store.spokes.insert(spoke.spoke_id.clone(), spoke);
            }
            ClientPayload::ProjectPathList {
                spoke_id,
                directories,
                ..
            } => {
                store.path_suggestions.insert(spoke_id, directories);
            }
            ClientPayload::Projection {
                revision, content, ..
            } => store.diagnostic_at(
                event_created_at,
                format!("Intent projection r{revision}: {content}"),
            ),
            ClientPayload::ReplayGap {
                requested,
                available,
                ..
            } => store.diagnostic_at(
                event_created_at,
                format!(
                    "Timeline replay gap: requested {requested}, earliest available {available}"
                ),
            ),
            ClientPayload::Error { code, message } => {
                store.diagnostic_at(event_created_at, format!("{code}: {message}"));
                store.connection = ConnectionView {
                    kind: ConnectionKind::Degraded,
                    label: "Attention required".into(),
                    attempt: 0,
                    reason: Some(message),
                };
            }
            ClientPayload::ProviderList { .. } | ClientPayload::HubStatus { .. } => {}
        }
        drop(store);
        self.emit();
        if let Some(command) = follow_up {
            self.dispatch(command);
        }
    }
}

const LOCAL_PORT: u16 = 43_123;

fn choose_local_project(runtime: GuiRuntime) {
    if let Some(path) = rfd::FileDialog::new()
        .set_title("Open local project")
        .pick_folder()
    {
        runtime.open_local_project(path);
    }
}

fn local_root() -> Result<PathBuf> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(state).join("pumpkinpi/local-v3"));
    }
    let home = std::env::var_os("HOME").context("HOME is required for local mode")?;
    Ok(PathBuf::from(home).join(".local/state/pumpkinpi/local-v3"))
}

fn sibling_binary(name: &str) -> Result<PathBuf> {
    let mut path = std::env::current_exe().context("could not locate PumpkinPi binaries")?;
    path.set_file_name(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(anyhow!(
            "{name} was not found beside {}",
            std::env::current_exe()?.display()
        ))
    }
}

fn run_output(binary: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {}", binary.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "{} failed: {}",
            binary.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn process_alive(pid_file: &Path) -> bool {
    fs::read_to_string(pid_file)
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(|pid| Path::new(&format!("/proc/{pid}")).exists())
}

fn local_port_open() -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], LOCAL_PORT));
    TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok()
}

fn wait_for_local_hub() -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(8) {
        if local_port_open() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!("local Hub did not start within 8 seconds"))
}

fn spawn_service(binary: &Path, args: &[&str], log: &Path, pid_file: &Path) -> Result<()> {
    let stdout = fs::File::create(log)?;
    let stderr = stdout.try_clone()?;
    let child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("failed to start {}", binary.display()))?;
    fs::write(pid_file, child.id().to_string())?;
    Ok(())
}

fn write_local_token(path: &Path, token: &str) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(token.as_bytes())?;
    Ok(())
}

fn bootstrap_local_services() -> Result<(String, String)> {
    let root = local_root()?;
    let hub_data = root.join("hub");
    let spoke_data = root.join("spoke");
    let logs = root.join("logs");
    fs::create_dir_all(&hub_data)?;
    fs::create_dir_all(&spoke_data)?;
    fs::create_dir_all(&logs)?;

    let hub_binary = sibling_binary("pumpkinpi-hub")?;
    let spoke_binary = sibling_binary("pumpkinpi-spoke")?;
    let hub_data_text = hub_data.to_string_lossy().into_owned();
    let spoke_data_text = spoke_data.to_string_lossy().into_owned();
    let listen = format!("127.0.0.1:{LOCAL_PORT}");
    let http_url = format!("http://{listen}");
    let websocket_url = format!("ws://{listen}/ws/client");
    let token_file = root.join("owner.token");
    let spoke_config = spoke_data.join("config.json");

    let token = if token_file.is_file() {
        fs::read_to_string(&token_file)?.trim().to_owned()
    } else {
        if local_port_open() {
            return Err(anyhow!(
                "port {LOCAL_PORT} is already in use and no local credentials exist"
            ));
        }
        let token = run_output(&hub_binary, &["--data-dir", &hub_data_text, "owner-token"])?;
        write_local_token(&token_file, &token)?;
        token
    };

    let mut setup_key = None;
    if !spoke_config.is_file() {
        if local_port_open() {
            return Err(anyhow!(
                "local Hub is already running but local Spoke enrollment is incomplete"
            ));
        }
        let created = run_output(
            &hub_binary,
            &[
                "--data-dir",
                &hub_data_text,
                "spoke",
                "create",
                "Local machine",
            ],
        )?;
        setup_key = created.lines().find_map(|line| {
            line.strip_prefix("setup_key:")
                .map(|value| value.trim().to_owned())
        });
        if setup_key.is_none() {
            return Err(anyhow!("local Hub did not return a Spoke setup key"));
        }
    }

    let hub_pid = root.join("hub.pid");
    if !local_port_open() {
        spawn_service(
            &hub_binary,
            &[
                "--data-dir",
                &hub_data_text,
                "serve",
                "--listen",
                &listen,
                "--public-url",
                &http_url,
            ],
            &logs.join("hub.log"),
            &hub_pid,
        )?;
        wait_for_local_hub()?;
    }

    if let Some(setup_key) = setup_key {
        run_output(
            &spoke_binary,
            &[
                "--data-dir",
                &spoke_data_text,
                "enroll",
                "--hub",
                &http_url,
                "--setup-key",
                &setup_key,
            ],
        )?;
    }

    let spoke_pid = root.join("spoke.pid");
    if !process_alive(&spoke_pid) {
        spawn_service(
            &spoke_binary,
            &["--data-dir", &spoke_data_text, "serve", "--hub", &http_url],
            &logs.join("spoke.log"),
            &spoke_pid,
        )?;
    }

    Ok((websocket_url, token))
}

fn project_key(project: &ProjectRecord) -> ProjectKey {
    ProjectKey {
        spoke_id: project.spoke_id.clone(),
        project_id: project.project_id.clone(),
    }
}

fn order_snapshot_collections(snapshot: &mut ProjectSnapshot) {
    snapshot.timeline.sort_by(|a, b| {
        (a.created_at, a.cursor, &a.timeline_item_id).cmp(&(
            b.created_at,
            b.cursor,
            &b.timeline_item_id,
        ))
    });
    snapshot
        .operations
        .sort_by(|a, b| (a.updated_at, &a.operation_id).cmp(&(b.updated_at, &b.operation_id)));
    snapshot
        .reviews
        .sort_by(|a, b| (a.created_at, &a.review_id).cmp(&(b.created_at, &b.review_id)));
    snapshot
        .divergences
        .sort_by(|a, b| a.divergence_id.cmp(&b.divergence_id));
    snapshot
        .iteration_telemetry
        .sort_by_key(|item| (item.recorded_at, item.iteration, item.operation_id.clone()));
}

fn merge_snapshot(old: &mut ProjectSnapshot, new: &ProjectSnapshot) {
    old.project = new.project.clone();
    old.source = new.source.clone();
    old.chat = new.chat.clone();
    for item in &new.timeline {
        if !old
            .timeline
            .iter()
            .any(|existing| existing.timeline_item_id == item.timeline_item_id)
        {
            old.timeline.push(item.clone());
        }
    }
    old.operations = new.operations.clone();
    old.reviews = new.reviews.clone();
    old.divergences = new.divergences.clone();
    old.requirement_index = new.requirement_index.clone();
    old.iteration_telemetry = new.iteration_telemetry.clone();
    old.gap_before = new.gap_before;
    order_snapshot_collections(old);
}

fn enum_class(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}").to_ascii_lowercase()
}

fn initials(name: &str) -> String {
    name.split(|ch: char| !ch.is_alphanumeric())
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Context,
    Activity,
    Diagnostics,
}

fn is_active(status: &OperationStatus) -> bool {
    matches!(
        status,
        OperationStatus::Queued
            | OperationStatus::Accepted
            | OperationStatus::Running
            | OperationStatus::Blocked
    )
}

fn app() -> Element {
    let runtime = use_hook(GuiRuntime::default);
    let mut state = use_signal(|| runtime.view());
    let mut filter = use_signal(String::new);
    let composer = use_signal(String::new);
    let interaction_answer = use_signal(String::new);
    let mut sidebar_open = use_signal(|| true);
    let mut inspector_open = use_signal(|| true);
    let mut inspector_tab = use_signal(|| InspectorTab::Context);
    let mut show_initialize = use_signal(|| false);
    let mut show_remote_login = use_signal(|| false);
    let mut login_hub = use_signal(|| runtime.view().hub_url);
    let mut login_token = use_signal(String::new);
    let mut init_spoke = use_signal(String::new);
    let mut init_cwd = use_signal(String::new);
    let mut init_name = use_signal(String::new);

    {
        let runtime = runtime.clone();
        use_future(move || {
            let mut receiver = runtime.subscribe();
            async move {
                while receiver.changed().await.is_ok() {
                    let next = receiver.borrow().clone();
                    state.set(next);
                }
            }
        });
    }
    {
        let runtime = runtime.clone();
        use_effect(move || runtime.connect());
    }

    let view = state.read().clone();
    let selected = view.selected.clone();
    let snapshot = view.selected_snapshot.clone();
    let shell_class = match (sidebar_open(), inspector_open()) {
        (true, true) => "app-shell",
        (false, true) => "app-shell sidebar-closed",
        (true, false) => "app-shell inspector-closed",
        (false, false) => "app-shell sidebar-closed inspector-closed",
    };
    let connection_class = format!("connection-pill {}", view.connection.kind.class());
    let connection_title = view.connection.reason.clone().unwrap_or_else(|| {
        if view.connection.attempt > 0 {
            format!("Reconnect attempt {}", view.connection.attempt)
        } else {
            view.hub_url.clone()
        }
    });
    let filter_text = filter().to_ascii_lowercase();
    let directory_options = view
        .path_suggestions
        .get(&SpokeId(init_spoke()))
        .cloned()
        .unwrap_or_default();

    rsx! {
        style { {STYLES} }
        div { class: shell_class,
            header { class: "topbar",
                div { class: "brand",
                    div { class: "brand-mark", "P" }
                    div { strong { "PumpkinPi" } }
                }
                button { class: "command-search",
                    span { "Search projects and intents" }
                    kbd { "⌘ K" }
                }
                div { class: "top-actions",
                    button {
                        class: "icon-button",
                        title: if sidebar_open() { "Collapse projects" } else { "Show projects" },
                        onclick: move |_| sidebar_open.toggle(),
                        if sidebar_open() { "◧" } else { "▧" }
                    }
                    button {
                        class: "icon-button",
                        title: "Refresh",
                        onclick: { let runtime = runtime.clone(); move |_| runtime.refresh() },
                        "↻"
                    }
                    button {
                        class: connection_class,
                        title: connection_title,
                        onclick: { let runtime = runtime.clone(); move |_| runtime.connect() },
                        i {}
                        span { "{view.connection.label}" }
                    }
                    button {
                        class: "icon-button",
                        title: "Toggle inspector",
                        onclick: move |_| inspector_open.toggle(),
                        "◫"
                    }
                }
            }

            aside { class: "sidebar",
                div { class: "pane-heading",
                    div { h1 { "Projects" } }
                    button { class: "small-icon-button", onclick: move |_| show_initialize.set(true), "+" }
                }
                div { class: "sidebar-filter",
                    input {
                        r#type: "search",
                        placeholder: "Filter projects…",
                        value: "{filter}",
                        oninput: move |event| filter.set(event.value()),
                    }
                }
                div { class: "project-list",
                    if view.projects.is_empty() {
                        div { class: "sidebar-empty",
                            div { class: "empty-orbit", "P" }
                            h2 { "No projects yet" }
                            p { "Open a directory on this machine, or initialize one on a remote Spoke." }
                            button { class: "primary-button", onclick: { let runtime = runtime.clone(); move |_| choose_local_project(runtime.clone()) }, "Open local project" }
                            button { class: "ghost-button", onclick: move |_| show_initialize.set(true), "Open on a remote Spoke" }
                        }
                    }
                    for spoke in &view.spokes {
                        div { class: "spoke-group", key: "{spoke.spoke_id}",
                            div { class: "spoke-heading",
                                i { class: "presence-dot {enum_class(&spoke.status)}" }
                                "{spoke.name}"
                            }
                            for project in view.projects.iter().filter(|project| {
                                project.spoke_id == spoke.spoke_id
                                    && (filter_text.is_empty()
                                        || project.name.to_ascii_lowercase().contains(&filter_text)
                                        || project.cwd.to_ascii_lowercase().contains(&filter_text))
                            }) {
                                {
                                    let key = project_key(project);
                                    let active = selected.as_ref() == Some(&key);
                                    let class = if active { "project-row active" } else { "project-row" };
                                    let runtime = runtime.clone();
                                    let click_key = key.clone();
                                    rsx! {
                                        button {
                                            class,
                                            key: "{project.project_id}",
                                            onclick: move |_| runtime.select_project(click_key.clone()),
                                            div { class: "project-icon", "{initials(&project.name)}" }
                                            div { class: "project-copy",
                                                b { "{project.name}" }
                                                span { "{project.cwd}" }
                                            }
                                            i { class: "project-badge {enum_class(&project.initialization_status)}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "spoke-summary", "{view.spokes.len()} Spokes · {view.projects.len()} Projects" }
            }

            main { class: "workspace",
                if let Some(snapshot) = &snapshot {
                    {render_chat(
                        snapshot,
                        &view,
                        &runtime,
                        composer,
                        interaction_answer,
                    )}
                } else {
                    section { class: "welcome-view",
                        div { class: "welcome-visual",
                            div { class: "visual-core", "P" }
                            i {} i {} i {}
                        }
                        h1 { "What do you want to work on?" }
                        p { class: "welcome-copy", "Open a project to start or continue its chat." }
                        button { class: "primary-button", onclick: { let runtime = runtime.clone(); move |_| choose_local_project(runtime.clone()) }, "Open local project" }
                    }
                }
            }

            aside { class: "inspector",
                div { class: "inspector-tabs",
                    button {
                        class: if inspector_tab() == InspectorTab::Context { "active" } else { "" },
                        onclick: move |_| inspector_tab.set(InspectorTab::Context),
                        "Context"
                    }
                    button {
                        class: if inspector_tab() == InspectorTab::Activity { "active" } else { "" },
                        onclick: move |_| inspector_tab.set(InspectorTab::Activity),
                        "Activity"
                    }
                    button {
                        class: if inspector_tab() == InspectorTab::Diagnostics { "active" } else { "" },
                        onclick: move |_| inspector_tab.set(InspectorTab::Diagnostics),
                        "Diagnostics"
                    }
                }
                div { class: "inspector-content", {render_inspector(snapshot.as_ref(), &view, &runtime, inspector_tab())} }
            }
        }

        if !view.configured {
            div { class: "modal-layer",
                div { class: "modal",
                    if show_remote_login() {
                        form { onsubmit: { let runtime = runtime.clone(); move |event| { event.prevent_default(); runtime.login(login_hub(), login_token()); } },
                            div { class: "modal-mark", "P" }
                            p { class: "eyebrow", "Remote workspace" }
                            h1 { "Connect to a Hub" }
                            p { "Use this for projects on another machine or a shared PumpkinPi installation." }
                            label { "Hub WebSocket URL"
                                input { value: "{login_hub}", oninput: move |event| login_hub.set(event.value()) }
                            }
                            label { "Owner token"
                                input { r#type: "password", value: "{login_token}", oninput: move |event| login_token.set(event.value()) }
                            }
                            div { class: "modal-actions",
                                button { class: "secondary-button", r#type: "button", onclick: move |_| show_remote_login.set(false), "Back" }
                                button { class: "primary-button", r#type: "submit", "Connect" }
                            }
                        }
                    } else {
                        div { class: "onboarding-content",
                            div { class: "modal-mark", "P" }
                            p { class: "eyebrow", "Start locally" }
                            h1 { "Open a local project" }
                            p { "Choose a directory. PumpkinPi will start its local services and open the project automatically—no Hub setup or enrollment required." }
                            button { class: "primary-button local-open-button", onclick: { let runtime = runtime.clone(); move |_| choose_local_project(runtime.clone()) }, "Choose project directory…" }
                            button { class: "ghost-button", onclick: move |_| show_remote_login.set(true), "Connect to a remote Hub instead" }
                        }
                    }
                }
            }
        }

        if show_initialize() {
            div { class: "modal-layer",
                div { class: "modal",
                    form { onsubmit: { let runtime = runtime.clone(); move |event| { event.prevent_default(); runtime.initialize_project(SpokeId(init_spoke()), init_cwd(), init_name()); show_initialize.set(false); } },
                        p { class: "eyebrow", "New workspace" }
                        h1 { "Initialize a project" }
                        p { "Choose a Spoke and an existing directory. PumpkinPi will inspect it before establishing the Source of Intent." }
                        label { "Spoke"
                            select { value: "{init_spoke}", oninput: { let runtime = runtime.clone(); move |event| { let spoke = event.value(); init_spoke.set(spoke.clone()); if !spoke.is_empty() { runtime.dispatch(ClientCommand::ProjectPathList { spoke_id: SpokeId(spoke), path: String::new() }); } } },
                                option { value: "", "Select a Spoke" }
                                for spoke in view.spokes.iter().filter(|spoke| spoke.status == SpokeStatus::Online) {
                                    option { value: "{spoke.spoke_id}", "{spoke.name} · {spoke.hostname}" }
                                }
                            }
                        }
                        label { "Directory"
                            div { class: "directory-combobox",
                                input { list: "project-directory-options", placeholder: "/home/me/project", value: "{init_cwd}", oninput: { let runtime = runtime.clone(); move |event| { let path = event.value(); init_cwd.set(path.clone()); let spoke = init_spoke(); if !spoke.is_empty() { runtime.dispatch(ClientCommand::ProjectPathList { spoke_id: SpokeId(spoke), path }); } } } }
                                datalist { id: "project-directory-options",
                                    for path in &directory_options { option { value: "{path}" } }
                                }
                                select { class: "directory-options", value: "", disabled: init_spoke().is_empty(), oninput: { let runtime = runtime.clone(); move |event| { let path = event.value(); if !path.is_empty() { init_cwd.set(path.clone()); let spoke = init_spoke(); runtime.dispatch(ClientCommand::ProjectPathList { spoke_id: SpokeId(spoke), path }); } } },
                                    option { value: "",
                                        if init_spoke().is_empty() { "Select a Spoke to browse…" }
                                        else if directory_options.is_empty() { "No matching child directories" }
                                        else { "Choose from {directory_options.len()} directories…" }
                                    }
                                    for path in &directory_options { option { value: "{path}", "{path}" } }
                                }
                            }
                        }
                        label { "Display name " span { "(optional)" }
                            input { value: "{init_name}", oninput: move |event| init_name.set(event.value()) }
                        }
                        div { class: "modal-actions",
                            button { class: "secondary-button", r#type: "button", onclick: move |_| show_initialize.set(false), "Cancel" }
                            button { class: "primary-button", r#type: "submit", "Initialize" }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ChatLogEntry {
    Timeline(TimelineItem),
    Pending(PendingMessage),
}

fn ordered_chat_entries(
    snapshot: &ProjectSnapshot,
    pending_messages: &[PendingMessage],
    key: &ProjectKey,
) -> Vec<ChatLogEntry> {
    let mut entries: Vec<_> = snapshot
        .timeline
        .iter()
        .cloned()
        .map(ChatLogEntry::Timeline)
        .chain(
            pending_messages
                .iter()
                .filter(|message| {
                    message.spoke_id == key.spoke_id && message.project_id == key.project_id
                })
                .cloned()
                .map(ChatLogEntry::Pending),
        )
        .collect();
    entries.sort_by(|a, b| {
        let created_at = |entry: &ChatLogEntry| match entry {
            ChatLogEntry::Timeline(item) => item.created_at,
            ChatLogEntry::Pending(message) => message.created_at,
        };
        created_at(a)
            .cmp(&created_at(b))
            .then_with(|| match (a, b) {
                (ChatLogEntry::Timeline(a), ChatLogEntry::Timeline(b)) => {
                    (a.cursor, &a.timeline_item_id).cmp(&(b.cursor, &b.timeline_item_id))
                }
                (ChatLogEntry::Timeline(_), ChatLogEntry::Pending(_)) => std::cmp::Ordering::Less,
                (ChatLogEntry::Pending(_), ChatLogEntry::Timeline(_)) => {
                    std::cmp::Ordering::Greater
                }
                (ChatLogEntry::Pending(a), ChatLogEntry::Pending(b)) => a.local_id.cmp(&b.local_id),
            })
    });
    entries
}

fn render_chat_log_entry(entry: &ChatLogEntry) -> Element {
    match entry {
        ChatLogEntry::Timeline(item) => render_timeline_item(item),
        ChatLogEntry::Pending(message) => {
            let timestamp = format_local_timestamp(message.created_at);
            rsx! {
                article { class: "timeline-item user_intent pending-item", key: "{message.local_id}",
                    div { class: "timeline-body",
                        div { class: "timeline-meta",
                            b { "You" }
                            time { datetime: "{timestamp}", "{timestamp}" }
                            span { class: "pending-dot" }
                        }
                        div { class: "timeline-content", "{message.message}" }
                    }
                }
            }
        }
    }
}

fn render_chat(
    snapshot: &ProjectSnapshot,
    view: &UiState,
    runtime: &GuiRuntime,
    mut composer: Signal<String>,
    interaction_answer: Signal<String>,
) -> Element {
    let key = project_key(&snapshot.project);
    let initials = initials(&snapshot.project.name);
    let chat_status = enum_class(&snapshot.chat.status);
    let realization_status = enum_class(&snapshot.project.realization_status);
    let status = if realization_status == "inactive" {
        chat_status
    } else {
        realization_status
    };
    let status_label = status.replace('_', " ");
    let active_interactions = view.interactions.iter().filter(|interaction| {
        interaction.spoke_id == key.spoke_id && interaction.project_id == key.project_id
    });
    let chat_entries = ordered_chat_entries(snapshot, &view.pending_messages, &key);
    let send_disabled = composer().trim().is_empty();
    let send_runtime = runtime.clone();
    let send_key = key.clone();
    let keyboard_runtime = runtime.clone();
    let keyboard_key = key.clone();
    let projection_runtime = runtime.clone();
    let projection_key = key.clone();
    let path = snapshot.project.cwd.clone();

    rsx! {
        section { class: "chat-view",
            header { class: "chat-header",
                div { class: "chat-identity",
                    div { class: "project-avatar", "{initials}" }
                    div {
                        div { class: "title-line",
                            h1 { "{snapshot.project.name}" }
                            span { class: "status-chip {status}", "{status_label}" }
                        }
                        p { "{snapshot.project.cwd}" }
                    }
                }
                div { class: "chat-actions",
                    button { class: "secondary-button", onclick: move |_| projection_runtime.projection(&projection_key), "View intent" }
                }
            }
            if let Some(gap) = snapshot.gap_before {
                div { class: "context-warning", "Some earlier timeline detail is unavailable before cursor {gap}. The current project snapshot remains authoritative." }
            }
            div { class: "timeline",
                if chat_entries.is_empty() {
                    div { class: "timeline-empty", "Describe what should change to begin this Intent Chat." }
                }
                for entry in &chat_entries {
                    {render_chat_log_entry(entry)}
                }
            }
            div { class: "interaction-stack",
                for interaction in active_interactions {
                    {render_interaction(interaction, runtime, interaction_answer)}
                }
            }
            div { class: "composer-wrap",
                form { class: "composer", onsubmit: move |event| { event.prevent_default(); let message = composer(); if !message.trim().is_empty() { send_runtime.send_intent(send_key.clone(), message); composer.set(String::new()); } },
                    textarea {
                        rows: "2",
                        placeholder: "Describe what should change…",
                        value: "{composer}",
                        oninput: move |event| composer.set(event.value()),
                        onkeydown: move |event| {
                            if event.key() == Key::Enter
                                && !event.modifiers().contains(Modifiers::SHIFT)
                                && !event.is_composing()
                            {
                                event.prevent_default();
                                let message = composer();
                                if !message.trim().is_empty() {
                                    keyboard_runtime.send_intent(keyboard_key.clone(), message);
                                    composer.set(String::new());
                                }
                            }
                        },
                    }
                    div { class: "composer-footer",
                        div { class: "composer-context",
                            span { "{path}" }
                            span { class: "risk-note", "Consequential actions require confirmation" }
                        }
                        div { class: "composer-actions",
                            span { class: "shortcut-hint", "Enter to send" }
                            button { class: "send-button", r#type: "submit", disabled: send_disabled, "↑" }
                        }
                    }
                }
            }
        }
    }
}

fn format_timestamp<Tz: TimeZone>(timestamp: u64, timezone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let instant = i64::try_from(timestamp)
        .ok()
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
    instant
        .with_timezone(timezone)
        .format("%Y-%m-%d %H:%M:%S %:z")
        .to_string()
}

pub(crate) fn format_local_timestamp(timestamp: u64) -> String {
    format_timestamp(timestamp, &Local)
}

fn render_timeline_item(item: &TimelineItem) -> Element {
    let kind = enum_class(&item.kind);
    let actor = if item.kind == TimelineKind::UserIntent {
        "You"
    } else {
        "PumpkinPi"
    };
    let icon = match item.kind {
        TimelineKind::UserIntent => "U",
        TimelineKind::Question | TimelineKind::ConsequentialPrompt => "?",
        TimelineKind::Outcome | TimelineKind::Evidence | TimelineKind::IntentUpdate => "✓",
        TimelineKind::Error => "!",
        TimelineKind::Progress => "…",
        _ => "P",
    };
    let content = item
        .content
        .as_deref()
        .or(item.summary.as_deref())
        .unwrap_or("Update");
    let revision = item.source_of_intent_revision;
    let timestamp = format_local_timestamp(item.created_at);
    let cursor_diagnostic = format!("Timeline cursor {}", item.cursor);

    rsx! {
        article { class: "timeline-item {kind}", key: "{item.timeline_item_id}",
            div { class: "timeline-icon", "{icon}" }
            div { class: "timeline-body",
                div { class: "timeline-meta",
                    b { "{actor}" }
                    time { datetime: "{timestamp}", title: "{cursor_diagnostic}", "{timestamp}" }
                    if let Some(revision) = revision { span { class: "revision-tag", "intent r{revision}" } }
                }
                if item.summary.is_some() && item.content.is_some() {
                    div { class: "timeline-summary", "{item.summary.as_deref().unwrap_or_default()}" }
                }
                div { class: "timeline-content", "{content}" }
            }
        }
    }
}

fn render_interaction(
    interaction: &InteractionView,
    runtime: &GuiRuntime,
    mut answer: Signal<String>,
) -> Element {
    let prompt = interaction
        .payload
        .get("message")
        .or_else(|| interaction.payload.get("prompt"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| interaction.payload.to_string());
    let approve_runtime = runtime.clone();
    let approve_interaction = interaction.clone();
    let deny_runtime = runtime.clone();
    let deny_interaction = interaction.clone();
    let answer_runtime = runtime.clone();
    let answer_interaction = interaction.clone();
    let timestamp = format_local_timestamp(interaction.created_at);

    rsx! {
        article { class: "interaction-card", key: "{interaction.operation_id}-{interaction.request_id}",
            div { class: "interaction-meta",
                h3 { "Action required · {interaction.method}" }
                time { datetime: "{timestamp}", "{timestamp}" }
            }
            p { "{prompt}" }
            div { class: "interaction-actions",
                input { placeholder: "Type a response…", value: "{answer}", oninput: move |event| answer.set(event.value()) }
                button { class: "secondary-button", onclick: move |_| deny_runtime.answer(&deny_interaction, Value::Bool(false)), "Deny" }
                button { class: "secondary-button", onclick: move |_| { let response = Value::String(answer()); answer_runtime.answer(&answer_interaction, response); answer.set(String::new()); }, "Reply" }
                button { class: "primary-button", onclick: move |_| approve_runtime.answer(&approve_interaction, Value::Bool(true)), "Approve" }
            }
        }
    }
}

fn render_inspector(
    snapshot: Option<&ProjectSnapshot>,
    view: &UiState,
    runtime: &GuiRuntime,
    tab: InspectorTab,
) -> Element {
    if tab == InspectorTab::Diagnostics {
        let connection = &view.connection;
        return rsx! {
            div {
                section { class: "inspector-section",
                    h2 { "Connection" }
                    dl { class: "context-card",
                        div { class: "context-row", dt { "State" } dd { "{connection.label}" } }
                        div { class: "context-row", dt { "Hub" } dd { "{view.hub_url}" } }
                        if let Some(reason) = &connection.reason {
                            div { class: "context-row", dt { "Detail" } dd { "{reason}" } }
                        }
                    }
                }
                section { class: "inspector-section",
                    h2 { "Event log" }
                    if view.diagnostics.is_empty() {
                        div { class: "inspector-empty compact", "No diagnostics have been reported." }
                    }
                    for (index, diagnostic) in view.diagnostics.iter().take(100).enumerate() {
                        div { class: "diagnostic-line", key: "{index}",
                            time { datetime: "{format_local_timestamp(diagnostic.created_at)}", "{format_local_timestamp(diagnostic.created_at)}" }
                            span { "{diagnostic.message}" }
                        }
                    }
                }
            }
        };
    }

    let Some(snapshot) = snapshot else {
        return rsx! { div { class: "inspector-empty", "Select a project to inspect its context and activity." } };
    };

    if tab == InspectorTab::Activity {
        return rsx! {
            div {
                section { class: "inspector-section",
                    h2 { "Operations · {snapshot.operations.len()}" }
                    if snapshot.operations.is_empty() {
                        div { class: "inspector-empty compact", "No operations yet. Send an intent to begin work." }
                    }
                    for operation in &snapshot.operations {
                        {
                            let runtime = runtime.clone();
                            let cancel_operation = operation.clone();
                            let status = enum_class(&operation.status);
                            let active = is_active(&operation.status);
                            let timestamp = format_local_timestamp(operation.updated_at);
                            rsx! {
                                div { class: "operation-row", key: "{operation.operation_id}",
                                    header {
                                        b { "{operation.kind}" }
                                        div { class: "operation-meta",
                                            time { datetime: "{timestamp}", "{timestamp}" }
                                            span { class: "operation-state {status}", "{status}" }
                                        }
                                    }
                                    p { class: "operation-id", "{operation.operation_id}" }
                                    if let Some(error) = &operation.error {
                                        p { class: "operation-error", "{error}" }
                                    }
                                    if active {
                                        button { class: "text-button", onclick: move |_| runtime.cancel(&cancel_operation), "Cancel operation" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    let source_status = enum_class(&snapshot.source.status);
    let initialization = enum_class(&snapshot.project.initialization_status);
    let realization = enum_class(&snapshot.project.realization_status);
    let latest_review = snapshot
        .reviews
        .iter()
        .max_by(|a, b| (a.created_at, &a.review_id).cmp(&(b.created_at, &b.review_id)));
    let latest_iteration = snapshot
        .iteration_telemetry
        .iter()
        .max_by_key(|item| (item.recorded_at, item.iteration));
    let provider = snapshot
        .project
        .default_provider
        .as_deref()
        .unwrap_or("Not set");
    let model = snapshot
        .project
        .default_model
        .as_deref()
        .unwrap_or("Not set");

    rsx! {
        div {
            section { class: "inspector-section",
                h2 { "Source of Intent" }
                div { class: "intent-meter",
                    header { b { "Revision {snapshot.source.revision}" } span { "{source_status}" } }
                    p { "{snapshot.source.format} · {snapshot.source.content_hash}" }
                    if let Some(bundle_hash) = &snapshot.source.authoritative_bundle_hash {
                        p { {format!("{} exact authoritative documents · bundle {}", snapshot.source.authoritative_document_count, bundle_hash)} }
                    }
                }
            }
            if let Some(review) = latest_review {
                section { class: "inspector-section",
                    h2 { "Latest independent review" }
                    div { class: "context-card",
                        p { b { "{enum_class(&review.verdict)}" } " · revision {review.source_of_intent_revision}" }
                        p { {format!("{} finding(s) · {} required scope item(s) unreviewed", review.findings.len(), review.unreviewed_required_scope.len())} }
                    }
                }
            }
            section { class: "inspector-section",
                h2 { "Convergence" }
                div { class: "context-card",
                    p { b { {format!("{} open divergence(s)", snapshot.divergences.len())} } }
                    if let Some(index) = &snapshot.requirement_index {
                        p { {format!("{} indexed requirement section(s) · revision {}", index.nodes.len(), index.source_of_intent_revision)} }
                    }
                    if let Some(iteration) = latest_iteration {
                        p { {format!("Iteration {} · implementation {:.1}s · validation {:.1}s · review {:.1}s · {} new / {} verified / {} reopened",
                            iteration.iteration,
                            iteration.implementation_ms as f64 / 1000.0,
                            iteration.validation_ms as f64 / 1000.0,
                            iteration.review_ms as f64 / 1000.0,
                            iteration.divergence_transitions.opened,
                            iteration.divergence_transitions.verified,
                            iteration.divergence_transitions.reopened,
                        )} }
                    }
                }
            }
            section { class: "inspector-section",
                h2 { "Project context" }
                dl { class: "context-card",
                    div { class: "context-row", dt { "Spoke" } dd { "{snapshot.project.spoke_id}" } }
                    div { class: "context-row", dt { "Path" } dd { "{snapshot.project.cwd}" } }
                    div { class: "context-row", dt { "Initialized" } dd { "{initialization}" } }
                    div { class: "context-row", dt { "Realization" } dd { "{realization}" } }
                    div { class: "context-row", dt { "Provider" } dd { "{provider}" } }
                    div { class: "context-row", dt { "Model" } dd { "{model}" } }
                    div { class: "context-row", dt { "Trust" } dd { if snapshot.project.trusted { "Trusted" } else { "Untrusted" } } }
                }
            }
        }
    }
}

async fn protocol_actor(runtime: GuiRuntime, commands: mpsc::Receiver<ActorCommand>) {
    let mut attempt = 0u32;
    let mut subscriptions = BTreeMap::<ProjectKey, u64>::new();

    loop {
        let config = match client_config::load() {
            Ok(config) => config,
            Err(error) => {
                runtime.set_connection(ConnectionView {
                    kind: ConnectionKind::Degraded,
                    label: "Configuration error".into(),
                    attempt,
                    reason: Some(error.to_string()),
                });
                return;
            }
        };
        let Some(token) = client_config::resolve_token().ok().flatten() else {
            runtime.set_connection(ConnectionView {
                kind: ConnectionKind::Disconnected,
                label: "Login required".into(),
                attempt,
                reason: Some("Enter your personal Hub URL and owner token.".into()),
            });
            return;
        };

        runtime.set_connection(if attempt == 0 {
            ConnectionView::simple(ConnectionKind::Connecting, "Connecting")
        } else {
            ConnectionView {
                kind: ConnectionKind::Reconnecting,
                label: format!("Reconnecting · attempt {attempt}"),
                attempt,
                reason: None,
            }
        });

        match connect_async(&config.hub).await {
            Ok((socket, _)) => {
                let (mut write, mut read) = socket.split();
                runtime.set_connection(ConnectionView::simple(
                    ConnectionKind::Authenticating,
                    "Authenticating",
                ));
                let auth = ClientHello::Auth {
                    protocol_version: PROTOCOL_VERSION,
                    token: token.clone(),
                };
                if write
                    .send(Message::Text(
                        serde_json::to_string(&auth)
                            .expect("auth serializes")
                            .into(),
                    ))
                    .await
                    .is_err()
                {
                    attempt += 1;
                    continue;
                }
                let Some(Ok(Message::Text(first))) = read.next().await else {
                    attempt += 1;
                    continue;
                };
                let Ok(first) = serde_json::from_str::<ClientEvent>(&first) else {
                    attempt += 1;
                    continue;
                };
                if !matches!(first.payload, ClientPayload::Authenticated) {
                    runtime.apply(first);
                    return;
                }

                attempt = 0;
                runtime.set_connection(ConnectionView::simple(
                    ConnectionKind::Connected,
                    "Connected",
                ));
                let _ = send_command(&mut write, ClientCommand::SpokeList).await;
                let _ =
                    send_command(&mut write, ClientCommand::ProjectList { spoke_id: None }).await;
                for (key, cursor) in &subscriptions {
                    let _ = send_command(
                        &mut write,
                        ClientCommand::IntentSubscribe {
                            spoke_id: key.spoke_id.clone(),
                            project_id: key.project_id.clone(),
                            cursor: Some(*cursor),
                        },
                    )
                    .await;
                }

                'connected: loop {
                    tokio::select! {
                        incoming = read.next() => {
                            let Some(Ok(Message::Text(text))) = incoming else { break 'connected };
                            match serde_json::from_str::<ClientEvent>(&text) {
                                Ok(event) => {
                                    if let ClientPayload::Timeline { item } = &event.payload {
                                        subscriptions.insert(ProjectKey { spoke_id: item.spoke_id.clone(), project_id: item.project_id.clone() }, item.cursor);
                                    }
                                    runtime.apply(event);
                                }
                                Err(error) => runtime.diagnostic(format!("Invalid Hub event: {error}")),
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_millis(20)) => {
                            loop {
                                match commands.try_recv() {
                                    Ok(ActorCommand::Request(command)) => {
                                        if let ClientCommand::IntentSubscribe { spoke_id, project_id, cursor } = &command {
                                            subscriptions.insert(ProjectKey { spoke_id: spoke_id.clone(), project_id: project_id.clone() }, cursor.unwrap_or(0));
                                        }
                                        if send_command(&mut write, command).await.is_err() { break 'connected }
                                    }
                                    Ok(ActorCommand::Stop) => return,
                                    Err(mpsc::TryRecvError::Empty) => break,
                                    Err(mpsc::TryRecvError::Disconnected) => return,
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                runtime.set_connection(ConnectionView {
                    kind: ConnectionKind::Degraded,
                    label: "Hub unavailable".into(),
                    attempt,
                    reason: Some(error.to_string()),
                });
            }
        }

        attempt += 1;
        tokio::time::sleep(Duration::from_secs(u64::from(attempt.min(5)))).await;
    }
}

async fn send_command<W>(write: &mut W, command: ClientCommand) -> std::result::Result<(), W::Error>
where
    W: SinkExt<Message> + Unpin,
{
    let request = ClientRequest {
        protocol_version: PROTOCOL_VERSION,
        id: RequestId(Uuid::new_v4().to_string()),
        command,
    };
    write
        .send(Message::Text(
            serde_json::to_string(&request)
                .expect("client request serializes")
                .into(),
        ))
        .await
}

pub(crate) fn run() -> Result<()> {
    let window = dioxus_native::WindowAttributes::default()
        .with_title("PumpkinPi")
        .with_surface_size(dioxus_native::LogicalSize::new(1440.0, 900.0))
        .with_min_surface_size(dioxus_native::LogicalSize::new(900.0, 620.0));
    let config = dioxus_native::Config::new().with_window_attributes(window);
    dioxus_native::launch_cfg(app, vec![], vec![Box::new(config)]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    const FIXED_INSTANT: u64 = 1_704_164_645; // 2024-01-02 03:04:05 UTC

    #[test]
    fn timestamp_format_includes_positive_utc_offset() {
        let timezone = FixedOffset::east_opt(5 * 60 * 60 + 30 * 60).unwrap();

        assert_eq!(
            format_timestamp(FIXED_INSTANT, &timezone),
            "2024-01-02 08:34:05 +05:30"
        );
    }

    #[test]
    fn timestamp_format_includes_negative_utc_offset() {
        let timezone = FixedOffset::west_opt(7 * 60 * 60).unwrap();

        assert_eq!(
            format_timestamp(FIXED_INSTANT, &timezone),
            "2024-01-01 20:04:05 -07:00"
        );
    }

    fn timeline(id: &str, cursor: u64, created_at: u64) -> TimelineItem {
        TimelineItem {
            timeline_item_id: TimelineItemId(id.into()),
            spoke_id: SpokeId("spoke".into()),
            project_id: ProjectId("project".into()),
            intent_chat_id: IntentChatId("chat".into()),
            operation_id: None,
            session_id: None,
            run_id: None,
            source_of_intent_revision: Some(1),
            kind: TimelineKind::Progress,
            visibility: Visibility::Primary,
            status: None,
            summary: None,
            content: Some(id.into()),
            cursor,
            created_at,
            updated_at: created_at,
            completed_at: None,
        }
    }

    fn operation(id: &str, updated_at: u64) -> OperationRecord {
        OperationRecord {
            operation_id: OperationId(id.into()),
            request_id: None,
            spoke_id: SpokeId("spoke".into()),
            project_id: ProjectId("project".into()),
            intent_chat_id: IntentChatId("chat".into()),
            source_of_intent_revision: Some(1),
            kind: "test".into(),
            status: OperationStatus::Running,
            error: None,
            created_at: 1,
            updated_at,
            completed_at: None,
        }
    }

    fn snapshot() -> ProjectSnapshot {
        let project_id = ProjectId("project".into());
        let spoke_id = SpokeId("spoke".into());
        let chat_id = IntentChatId("chat".into());
        ProjectSnapshot {
            project: ProjectRecord {
                project_id: project_id.clone(),
                spoke_id: spoke_id.clone(),
                name: "project".into(),
                cwd: "/tmp/project".into(),
                source_of_intent_id: SourceOfIntentId("source".into()),
                intent_chat_id: chat_id.clone(),
                initialization_status: InitializationStatus::Ready,
                default_provider: None,
                default_model: None,
                run_as_user: None,
                allow_root_sessions: false,
                status: ProjectStatus::Active,
                trusted: true,
                realization_status: RealizationStatus::Inactive,
                created_at: 1,
                updated_at: 1,
            },
            source: SourceOfIntentMetadata {
                source_of_intent_id: SourceOfIntentId("source".into()),
                spoke_id: spoke_id.clone(),
                project_id: project_id.clone(),
                format: "markdown.v1".into(),
                revision: 1,
                content_hash: "hash".into(),
                authoritative_bundle_hash: None,
                authoritative_document_count: 0,
                status: SourceStatus::Active,
                created_at: 1,
                updated_at: 1,
            },
            chat: IntentChatRecord {
                intent_chat_id: chat_id,
                spoke_id,
                project_id,
                source_of_intent_revision: 1,
                status: IntentStatus::Ready,
                next_cursor: 1,
                created_at: 1,
                updated_at: 1,
                last_active_at: 1,
            },
            timeline: vec![],
            operations: vec![],
            reviews: vec![],
            divergences: vec![],
            requirement_index: None,
            iteration_telemetry: vec![],
            gap_before: None,
        }
    }

    #[test]
    fn snapshot_logs_sort_by_authoritative_timestamp_then_stable_id() {
        let mut snapshot = snapshot();
        snapshot.timeline = vec![
            timeline("item_z", 4, 20),
            timeline("item_b", 3, 10),
            timeline("item_a", 3, 10),
        ];
        snapshot.operations = vec![
            operation("operation_z", 20),
            operation("operation_b", 10),
            operation("operation_a", 10),
        ];

        order_snapshot_collections(&mut snapshot);

        assert_eq!(
            snapshot
                .timeline
                .iter()
                .map(|item| item.timeline_item_id.0.as_str())
                .collect::<Vec<_>>(),
            ["item_a", "item_b", "item_z"]
        );
        assert_eq!(
            snapshot
                .operations
                .iter()
                .map(|operation| operation.operation_id.0.as_str())
                .collect::<Vec<_>>(),
            ["operation_a", "operation_b", "operation_z"]
        );
    }

    #[test]
    fn view_sorts_interactions_and_diagnostics_independent_of_map_or_arrival_order() {
        let mut store = AppStore::default();
        for (operation_id, request_id, created_at) in [
            ("operation_z", "request_z", 20),
            ("operation_a", "request_b", 10),
            ("operation_a", "request_a", 10),
        ] {
            let operation_id = OperationId(operation_id.into());
            store.interactions.insert(
                (operation_id.clone(), request_id.into()),
                InteractionView {
                    spoke_id: SpokeId("spoke".into()),
                    project_id: ProjectId("project".into()),
                    operation_id,
                    request_id: request_id.into(),
                    method: "confirm".into(),
                    payload: Value::Null,
                    created_at,
                },
            );
        }
        store.diagnostic_at(20, "later");
        store.diagnostic_at(10, "same-z");
        store.diagnostic_at(10, "same-a");

        let view = store.view();

        assert_eq!(
            view.interactions
                .iter()
                .map(|interaction| interaction.request_id.as_str())
                .collect::<Vec<_>>(),
            ["request_a", "request_b", "request_z"]
        );
        assert_eq!(
            view.diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            ["same-a", "same-z", "later"]
        );
    }

    #[test]
    fn intent_log_merges_pending_messages_into_timestamp_order() {
        let mut snapshot = snapshot();
        snapshot.timeline = vec![timeline("item_later", 2, 20), timeline("item_first", 1, 10)];
        let key = project_key(&snapshot.project);
        let pending = vec![PendingMessage {
            spoke_id: key.spoke_id.clone(),
            project_id: key.project_id.clone(),
            local_id: "pending_middle".into(),
            message: "middle".into(),
            created_at: 15,
        }];

        let entries = ordered_chat_entries(&snapshot, &pending, &key);
        let labels = entries
            .iter()
            .map(|entry| match entry {
                ChatLogEntry::Timeline(item) => item.timeline_item_id.0.as_str(),
                ChatLogEntry::Pending(message) => message.local_id.as_str(),
            })
            .collect::<Vec<_>>();

        assert_eq!(labels, ["item_first", "pending_middle", "item_later"]);
    }
}

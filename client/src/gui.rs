use std::{
    sync::mpsc::{self, TryRecvError},
    thread,
};

use anyhow::{Result, anyhow};
use eframe::egui;
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::PROTOCOL_VERSION;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client_config;

enum WsCommand {
    ListNodes,
    ListProjects {
        node_id: String,
    },
    ListSessions {
        node_id: String,
        project_id: String,
    },
    Subscribe {
        node_id: String,
        project_id: String,
        session_id: String,
    },
    SendPrompt {
        node_id: String,
        project_id: String,
        session_id: String,
        message: String,
    },
}

enum WsEvent {
    Connected,
    Incoming(Value),
    Error(String),
    Disconnected,
}

pub(crate) fn run() -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "PumpkinPi",
        options,
        Box::new(|_cc| Ok(Box::<PumpkinPiApp>::default())),
    )
    .map_err(|err| anyhow!(err.to_string()))
}

struct PumpkinPiApp {
    hub: String,
    token: Option<String>,
    login_token: String,
    status: String,
    tx: Option<mpsc::Sender<WsCommand>>,
    rx: Option<mpsc::Receiver<WsEvent>>,
    nodes: Vec<Value>,
    projects: Vec<Value>,
    sessions: Vec<Value>,
    selected_node: Option<String>,
    selected_project: Option<String>,
    selected_session: Option<String>,
    transcript: Vec<String>,
    prompt: String,
}

impl Default for PumpkinPiApp {
    fn default() -> Self {
        let config = client_config::load().unwrap_or_default();
        let token = client_config::resolve_token().unwrap_or(config.token.clone());
        Self {
            hub: config.hub,
            token,
            login_token: String::new(),
            status: "Disconnected".to_string(),
            tx: None,
            rx: None,
            nodes: Vec::new(),
            projects: Vec::new(),
            sessions: Vec::new(),
            selected_node: None,
            selected_project: None,
            selected_session: None,
            transcript: Vec::new(),
            prompt: String::new(),
        }
    }
}

impl eframe::App for PumpkinPiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();

        egui::TopBottomPanel::top("connection").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Hub");
                ui.text_edit_singleline(&mut self.hub);
                if self.token.is_some() {
                    ui.label("Logged in");
                } else {
                    ui.label("Login token");
                    ui.add(egui::TextEdit::singleline(&mut self.login_token).password(true));
                    if ui.button("Save login").clicked() {
                        match client_config::login(self.hub.clone(), self.login_token.clone()) {
                            Ok(_) => {
                                self.token = Some(self.login_token.clone());
                                self.login_token.clear();
                                self.status = "Login saved".to_string();
                            }
                            Err(err) => self.status = format!("Login failed: {err}"),
                        }
                    }
                }
                if ui.button("Connect").clicked() {
                    self.connect();
                }
                if ui.button("Refresh").clicked() {
                    self.send(WsCommand::ListNodes);
                }
                ui.label(&self.status);
            });
        });

        egui::SidePanel::left("inventory")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Nodes");
                for node in self.nodes.clone() {
                    let id = field(&node, "node_id");
                    let selected = self.selected_node.as_deref() == Some(id.as_str());
                    if ui
                        .selectable_label(selected, label(&node, "node_id"))
                        .clicked()
                    {
                        self.selected_node = Some(id.clone());
                        self.selected_project = None;
                        self.selected_session = None;
                        self.projects.clear();
                        self.sessions.clear();
                        self.transcript.clear();
                        self.send(WsCommand::ListProjects { node_id: id });
                    }
                }

                ui.separator();
                ui.heading("Projects");
                for project in self.projects.clone() {
                    let id = field(&project, "project_id");
                    let selected = self.selected_project.as_deref() == Some(id.as_str());
                    if ui
                        .selectable_label(selected, label(&project, "project_id"))
                        .clicked()
                    {
                        self.selected_project = Some(id.clone());
                        self.selected_session = None;
                        self.sessions.clear();
                        self.transcript.clear();
                        if let Some(node_id) = self.selected_node.clone() {
                            self.send(WsCommand::ListSessions {
                                node_id,
                                project_id: id,
                            });
                        }
                    }
                }

                ui.separator();
                ui.heading("Sessions");
                for session in self.sessions.clone() {
                    let id = field(&session, "session_id");
                    let selected = self.selected_session.as_deref() == Some(id.as_str());
                    if ui
                        .selectable_label(selected, label(&session, "session_id"))
                        .clicked()
                    {
                        self.selected_session = Some(id.clone());
                        self.transcript.clear();
                        if let (Some(node_id), Some(project_id)) =
                            (self.selected_node.clone(), self.selected_project.clone())
                        {
                            self.send(WsCommand::Subscribe {
                                node_id,
                                project_id,
                                session_id: id,
                            });
                        }
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Session");
            egui::ScrollArea::vertical()
                .id_source("transcript")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.transcript {
                        ui.label(line);
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                let response = ui.add_sized(
                    [ui.available_width() - 80.0, 28.0],
                    egui::TextEdit::singleline(&mut self.prompt).hint_text("Send a prompt..."),
                );
                let pressed_enter =
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if ui.button("Send").clicked() || pressed_enter {
                    self.send_prompt();
                }
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl PumpkinPiApp {
    fn connect(&mut self) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let hub = self.hub.clone();
        let Some(token) = self.token.clone() else {
            self.status =
                "Not logged in; run `pumpkinpi login --token ...` or save a token here".to_string();
            return;
        };
        let _ = client_config::save(&client_config::ClientConfig {
            hub: hub.clone(),
            token: Some(token.clone()),
        });
        self.status = "Connecting...".to_string();
        self.tx = Some(cmd_tx);
        self.rx = Some(event_rx);
        self.nodes.clear();
        self.projects.clear();
        self.sessions.clear();
        self.transcript.clear();
        thread::spawn(move || match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime.block_on(ws_actor(hub, token, cmd_rx, event_tx)),
            Err(err) => {
                let _ = event_tx.send(WsEvent::Error(err.to_string()));
            }
        });
    }

    fn drain_events(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                WsEvent::Connected => self.status = "Connected".to_string(),
                WsEvent::Incoming(value) => self.handle_value(value),
                WsEvent::Error(err) => {
                    self.status = format!("Error: {err}");
                    self.transcript.push(format!("error: {err}"));
                }
                WsEvent::Disconnected => self.status = "Disconnected".to_string(),
            }
        }
    }

    fn handle_value(&mut self, value: Value) {
        match value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "node.list.result" => self.nodes = array(&value, "nodes"),
            "project.list.result" => self.projects = array(&value, "projects"),
            "session.list.result" => self.sessions = array(&value, "sessions"),
            "session.output_delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    self.transcript.push(delta.to_string());
                }
            }
            "session.message_end" => self.transcript.push(format!("\n{}", compact(&value))),
            "session.running" | "session.idle" | "session.turn_ended" => {
                self.transcript.push(format!("[{}]", field(&value, "type")));
            }
            "error" => self
                .transcript
                .push(format!("error: {}", field(&value, "error"))),
            _ => self.transcript.push(compact(&value)),
        }
    }

    fn send(&mut self, command: WsCommand) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(command);
        }
    }

    fn send_prompt(&mut self) {
        let message = self.prompt.trim().to_string();
        if message.is_empty() {
            return;
        }
        if let (Some(node_id), Some(project_id), Some(session_id)) = (
            self.selected_node.clone(),
            self.selected_project.clone(),
            self.selected_session.clone(),
        ) {
            self.transcript.push(format!("You: {message}"));
            self.prompt.clear();
            self.send(WsCommand::SendPrompt {
                node_id,
                project_id,
                session_id,
                message,
            });
        }
    }
}

async fn ws_actor(
    hub: String,
    token: String,
    cmd_rx: mpsc::Receiver<WsCommand>,
    event_tx: mpsc::Sender<WsEvent>,
) {
    if let Err(err) = ws_actor_inner(hub, token, cmd_rx, event_tx.clone()).await {
        let _ = event_tx.send(WsEvent::Error(err.to_string()));
    }
    let _ = event_tx.send(WsEvent::Disconnected);
}

async fn ws_actor_inner(
    hub: String,
    token: String,
    cmd_rx: mpsc::Receiver<WsCommand>,
    event_tx: mpsc::Sender<WsEvent>,
) -> Result<()> {
    let (socket, _) = connect_async(&hub).await?;
    let (mut write, mut read) = socket.split();
    write
        .send(Message::Text(
            json!({"protocol_version": PROTOCOL_VERSION, "type":"client.auth", "token": token})
                .to_string()
                .into(),
        ))
        .await?;

    loop {
        let Some(msg) = read.next().await else {
            return Err(anyhow!("connection closed before authentication"));
        };
        let Message::Text(text) = msg? else { continue };
        let value: Value = serde_json::from_str(&text)?;
        match value.get("type").and_then(Value::as_str) {
            Some("client.authenticated") => break,
            Some("error") => return Err(anyhow!(field(&value, "error"))),
            _ => {}
        }
    }

    let _ = event_tx.send(WsEvent::Connected);
    let mut next_id = 1_u64;
    send_request(&mut write, &mut next_id, json!({"type":"node.list"})).await?;

    let mut command_tick = tokio::time::interval(std::time::Duration::from_millis(50));
    loop {
        tokio::select! {
            msg = read.next() => {
                let Some(msg) = msg else { break; };
                let Message::Text(text) = msg? else { continue; };
                let value: Value = serde_json::from_str(&text)?;
                let _ = event_tx.send(WsEvent::Incoming(value));
            }
            _ = command_tick.tick() => {
                loop {
                    let command = match cmd_rx.try_recv() {
                        Ok(command) => command,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return Ok(()),
                    };
                    match command {
                        WsCommand::ListNodes => send_request(&mut write, &mut next_id, json!({"type":"node.list"})).await?,
                        WsCommand::ListProjects { node_id } => send_request(&mut write, &mut next_id, json!({"type":"project.list", "node_id": node_id})).await?,
                        WsCommand::ListSessions { node_id, project_id } => send_request(&mut write, &mut next_id, json!({"type":"session.list", "node_id": node_id, "project_id": project_id})).await?,
                        WsCommand::Subscribe { node_id, project_id, session_id } => send_request(&mut write, &mut next_id, json!({"type":"session.subscribe", "node_id": node_id, "project_id": project_id, "session_id": session_id})).await?,
                        WsCommand::SendPrompt { node_id, project_id, session_id, message } => send_request(&mut write, &mut next_id, json!({"type":"session.send", "node_id": node_id, "project_id": project_id, "session_id": session_id, "command":{"type":"prompt", "message": message}})).await?,
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_request(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    next_id: &mut u64,
    mut value: Value,
) -> Result<()> {
    value["protocol_version"] = json!(PROTOCOL_VERSION);
    value["id"] = json!(format!("gui-{next_id}"));
    *next_id += 1;
    write.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

fn array(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| value.get(key).map(Value::to_string).unwrap_or_default())
}

fn label(value: &Value, id_key: &str) -> String {
    let id = field(value, id_key);
    let name = field(value, "name");
    let status = field(value, "status");
    match (name.is_empty(), status.is_empty()) {
        (false, false) => format!("{name} ({id}) · {status}"),
        (false, true) => format!("{name} ({id})"),
        (true, false) => format!("{id} · {status}"),
        (true, true) => id,
    }
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<invalid json>".to_string())
}

use std::{ffi::OsString, process::Command as StdCommand};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::PROTOCOL_VERSION;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

mod cli;
mod client_config;
mod gui;

use cli::{Cli, Command, NodeCommand, ProjectCommand, SessionCommand};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Status { hub } => {
            let hub = client_config::resolve_hub(hub.as_deref())?;
            request_print(
                &hub,
                json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"hub.status"}),
                false,
            )
            .await
        }
        Command::Login(args) => {
            let path = client_config::login(args.hub, args.token)?;
            println!("saved client login to {}", path.display());
            Ok(())
        }
        Command::Logout => {
            let path = client_config::logout()?;
            println!("removed saved client token from {}", path.display());
            Ok(())
        }
        Command::Config => {
            let path = client_config::config_path()?;
            let config = client_config::load()?;
            println!("config: {}", path.display());
            println!("hub: {}", config.hub);
            println!(
                "token: {}",
                if config.token.is_some() {
                    "saved"
                } else {
                    "not saved"
                }
            );
            Ok(())
        }
        Command::Gui => gui::run(),
        Command::Hub(args) => exec_external("pumpkinpi-hub", args.args),
        Command::Node(args) => exec_external("pumpkinpi-node", args.args),
        Command::Nodes { command, hub } => {
            let hub = client_config::resolve_hub(hub.as_deref())?;
            match command {
            NodeCommand::List => {
                request_print(
                    &hub,
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"node.list"}),
                    false,
                )
                .await
            }
            NodeCommand::Get { node_id } => {
                request_print(
                    &hub,
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"node.get", "node_id": node_id}),
                    false,
                )
                .await
            }
        }
        }
        Command::Project {
            command,
            hub,
            node_id,
        } => {
            let hub = client_config::resolve_hub(hub.as_deref())?;
            let msg = match command {
                ProjectCommand::List => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"project.list", "node_id": node_id})
                }
                ProjectCommand::Add {
                    cwd,
                    name,
                    run_as_user,
                    allow_root_sessions,
                    default_provider,
                    default_model,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"project.add", "node_id": node_id, "cwd": cwd, "name": name, "run_as_user": run_as_user, "allow_root_sessions": allow_root_sessions, "default_provider": default_provider, "default_model": default_model})
                }
                ProjectCommand::Get { project_id } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"project.get", "node_id": node_id, "project_id": project_id})
                }
                ProjectCommand::Remove { project_id } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"project.remove", "node_id": node_id, "project_id": project_id})
                }
            };
            request_print(&hub, msg, true).await
        }
        Command::Session {
            command,
            hub,
            node_id,
        } => {
            let hub = client_config::resolve_hub(hub.as_deref())?;
            let msg = match command {
                SessionCommand::List { project_id } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.list", "node_id": node_id, "project_id": project_id})
                }
                SessionCommand::Create {
                    project_id,
                    name,
                    run_as_user,
                    run_as_root,
                    provider,
                    model,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.create", "node_id": node_id, "project_id": project_id, "name": name, "run_as_user": run_as_user, "run_as_root": run_as_root, "provider": provider, "model": model})
                }
                SessionCommand::Attach {
                    project_id,
                    session_id,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.attach", "node_id": node_id, "project_id": project_id, "session_id": session_id})
                }
                SessionCommand::Subscribe {
                    project_id,
                    session_id,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.subscribe", "node_id": node_id, "project_id": project_id, "session_id": session_id})
                }
                SessionCommand::Detach {
                    project_id,
                    session_id,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.detach", "node_id": node_id, "project_id": project_id, "session_id": session_id})
                }
                SessionCommand::Send {
                    project_id,
                    session_id,
                    provider_account_id,
                    message,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.send", "node_id": node_id, "project_id": project_id, "session_id": session_id, "provider_account_id": provider_account_id, "command":{"type":"prompt", "message": message}})
                }
                SessionCommand::Stop {
                    project_id,
                    session_id,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.stop", "node_id": node_id, "project_id": project_id, "session_id": session_id})
                }
                SessionCommand::Restart {
                    project_id,
                    session_id,
                    provider_account_id,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.restart", "node_id": node_id, "project_id": project_id, "session_id": session_id, "provider_account_id": provider_account_id})
                }
                SessionCommand::Delete {
                    project_id,
                    session_id,
                } => {
                    json!({"protocol_version": PROTOCOL_VERSION, "id":"1", "type":"session.delete", "node_id": node_id, "project_id": project_id, "session_id": session_id})
                }
            };
            request_print(&hub, msg, true).await
        }
    }
}

fn exec_external(binary: &str, args: Vec<OsString>) -> Result<()> {
    let status = StdCommand::new(binary)
        .args(args)
        .status()
        .with_context(|| {
            format!(
                "failed to execute {binary}; ensure it is installed next to pumpkinpi or on PATH"
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{binary} exited with {status}"))
    }
}

async fn request_print(hub: &str, message: Value, wait_for_forwarded: bool) -> Result<()> {
    let (socket, _) = connect_async(hub).await?;
    let (mut write, mut read) = socket.split();
    let token = client_config::resolve_token()?.ok_or_else(|| {
        anyhow!("not logged in; run `pumpkinpi login --token <token>` or set PUMPKINPI_TOKEN")
    })?;
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
        let Message::Text(text) = msg? else {
            continue;
        };
        let value: Value = serde_json::from_str(&text)?;
        match value.get("type").and_then(Value::as_str) {
            Some("client.authenticated") => break,
            Some("error") => {
                return Err(anyhow!(
                    value
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("authentication failed")
                        .to_string()
                ));
            }
            _ => continue,
        }
    }
    write
        .send(Message::Text(message.to_string().into()))
        .await?;
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    while let Some(msg) = read.next().await {
        let Message::Text(text) = msg? else {
            continue;
        };
        let value: Value = serde_json::from_str(&text)?;
        if wait_for_forwarded && value.get("type").and_then(Value::as_str) == Some("accepted") {
            continue;
        }
        if value.get("id") == Some(&id) {
            println!("{}", serde_json::to_string_pretty(&value)?);
            return Ok(());
        }
    }
    Err(anyhow!("connection closed before response"))
}

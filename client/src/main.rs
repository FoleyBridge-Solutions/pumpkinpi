use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use pumpkinpi_protocol::*;
use std::{
    ffi::OsString,
    io::{self, Write},
    process::Command as ProcessCommand,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;
mod client_config;
mod gui;

#[derive(Parser)]
#[command(name = "pumpkinpi", version, about = "Intent-first PumpkinPi client")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Login {
        #[arg(long, default_value = "ws://127.0.0.1:8080/ws/client")]
        hub: String,
        token: String,
    },
    Logout,
    Gui,
    Hub {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    Spoke {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    Status,
    Spokes,
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    Intent {
        #[command(subcommand)]
        cmd: IntentCmd,
    },
    Provider {
        #[command(subcommand)]
        cmd: ProviderCmd,
    },
    Chat {
        spoke_id: String,
        project_id: String,
    },
}
#[derive(Subcommand)]
enum ProjectCmd {
    List {
        #[arg(long)]
        spoke_id: Option<String>,
    },
    Init {
        spoke_id: String,
        cwd: String,
        #[arg(long)]
        name: Option<String>,
    },
    Status {
        spoke_id: String,
        project_id: String,
    },
    Remove {
        spoke_id: String,
        project_id: String,
    },
    Model {
        spoke_id: String,
        project_id: String,
        provider: String,
        model: String,
    },
}
#[derive(Subcommand)]
enum ProviderCmd {
    List,
    Set {
        provider_id: String,
        api_key: String,
        #[arg(long, default_value = "default")]
        label: String,
    },
    Revoke {
        provider_account_id: String,
    },
}
#[derive(Subcommand)]
enum IntentCmd {
    Send {
        spoke_id: String,
        project_id: String,
        message: String,
    },
    Summarize {
        spoke_id: String,
        project_id: String,
    },
    Cancel {
        spoke_id: String,
        project_id: String,
        operation_id: String,
    },
    Answer {
        spoke_id: String,
        project_id: String,
        operation_id: String,
        request_id: String,
        response_json: String,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Login { hub, token } => {
            println!("saved {}", client_config::login(hub, token)?.display());
            Ok(())
        }
        Cmd::Logout => {
            client_config::logout()?;
            Ok(())
        }
        Cmd::Gui => gui::run(),
        Cmd::Hub { args } => exec("pumpkinpi-hub", args),
        Cmd::Spoke { args } => exec("pumpkinpi-spoke", args),
        Cmd::Status => one(ClientCommand::HubStatus).await,
        Cmd::Spokes => one(ClientCommand::SpokeList).await,
        Cmd::Project { cmd } => {
            one(match cmd {
                ProjectCmd::List { spoke_id } => ClientCommand::ProjectList {
                    spoke_id: spoke_id.map(SpokeId),
                },
                ProjectCmd::Init {
                    spoke_id,
                    cwd,
                    name,
                } => ClientCommand::ProjectInitialize {
                    spoke_id: SpokeId(spoke_id),
                    cwd,
                    name,
                },
                ProjectCmd::Status {
                    spoke_id,
                    project_id,
                } => ClientCommand::ProjectGet {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                },
                ProjectCmd::Remove {
                    spoke_id,
                    project_id,
                } => ClientCommand::ProjectRemove {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                },
                ProjectCmd::Model {
                    spoke_id,
                    project_id,
                    provider,
                    model,
                } => ClientCommand::ProjectModelSet {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                    provider,
                    model,
                },
            })
            .await
        }
        Cmd::Intent { cmd } => {
            one(match cmd {
                IntentCmd::Send {
                    spoke_id,
                    project_id,
                    message,
                } => ClientCommand::IntentSend {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                    message,
                    expected_revision: None,
                },
                IntentCmd::Summarize {
                    spoke_id,
                    project_id,
                } => ClientCommand::IntentGetProjection {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                },
                IntentCmd::Cancel {
                    spoke_id,
                    project_id,
                    operation_id,
                } => ClientCommand::IntentCancel {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                    operation_id: OperationId(operation_id),
                },
                IntentCmd::Answer {
                    spoke_id,
                    project_id,
                    operation_id,
                    request_id,
                    response_json,
                } => ClientCommand::IntentAnswer {
                    spoke_id: SpokeId(spoke_id),
                    project_id: ProjectId(project_id),
                    operation_id: OperationId(operation_id),
                    request_id,
                    response: serde_json::from_str(&response_json)?,
                },
            })
            .await
        }
        Cmd::Provider { cmd } => {
            one(match cmd {
                ProviderCmd::List => ClientCommand::ProviderList,
                ProviderCmd::Set {
                    provider_id,
                    label,
                    api_key,
                } => ClientCommand::ProviderSet {
                    provider_id,
                    label,
                    api_key,
                },
                ProviderCmd::Revoke {
                    provider_account_id,
                } => ClientCommand::ProviderRevoke {
                    provider_account_id,
                },
            })
            .await
        }
        Cmd::Chat {
            spoke_id,
            project_id,
        } => chat(SpokeId(spoke_id), ProjectId(project_id)).await,
    }
}
fn exec(binary: &str, args: Vec<OsString>) -> Result<()> {
    let status = ProcessCommand::new(binary).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{binary} exited with {status}"))
    }
}
async fn connect() -> Result<(
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
)> {
    let cfg = client_config::load()?;
    let token = client_config::resolve_token()?.ok_or_else(|| anyhow!("not logged in"))?;
    let (ws, _) = connect_async(cfg.hub).await?;
    let (mut w, mut r) = ws.split();
    w.send(Message::Text(
        serde_json::to_string(&ClientHello::Auth {
            protocol_version: PROTOCOL_VERSION,
            token,
        })?
        .into(),
    ))
    .await?;
    let first: ClientEvent = read_event(&mut r).await?;
    if !matches!(first.payload, ClientPayload::Authenticated) {
        return Err(anyhow!("authentication failed"));
    }
    Ok((w, r))
}
async fn one(command: ClientCommand) -> Result<()> {
    let (mut w, mut r) = connect().await?;
    let id = RequestId(Uuid::new_v4().to_string());
    w.send(Message::Text(
        serde_json::to_string(&ClientRequest {
            protocol_version: PROTOCOL_VERSION,
            id: id.clone(),
            command,
        })?
        .into(),
    ))
    .await?;
    loop {
        let e = read_event(&mut r).await?;
        if e.id.as_ref() == Some(&id) {
            println!("{}", serde_json::to_string_pretty(&e)?);
            return if let ClientPayload::Error { message, .. } = e.payload {
                Err(anyhow!(message))
            } else {
                Ok(())
            };
        }
    }
}
async fn chat(spoke_id: SpokeId, project_id: ProjectId) -> Result<()> {
    let (mut w, mut r) = connect().await?;
    send(
        &mut w,
        ClientCommand::IntentSubscribe {
            spoke_id: spoke_id.clone(),
            project_id: project_id.clone(),
            cursor: None,
        },
    )
    .await?;
    let snap = read_event(&mut r).await?;
    print_event(&snap);
    println!("Intent Chat. Enter intent; /quit exits.");
    let mut lines =
        tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(tokio::io::stdin()));
    loop {
        print!("> ");
        io::stdout().flush()?;
        tokio::select! {line=lines.next_line()=>{let Some(line)=line? else{break};if line.trim()=="/quit"{break}send(&mut w,ClientCommand::IntentSend{spoke_id:spoke_id.clone(),project_id:project_id.clone(),message:line,expected_revision:None}).await?},event=read_event(&mut r)=>print_event(&event?)}
    }
    Ok(())
}
async fn send<W: SinkExt<Message> + Unpin>(w: &mut W, command: ClientCommand) -> Result<()>
where
    W::Error: std::error::Error + Send + Sync + 'static,
{
    let req = ClientRequest {
        protocol_version: PROTOCOL_VERSION,
        id: RequestId(Uuid::new_v4().to_string()),
        command,
    };
    w.send(Message::Text(serde_json::to_string(&req)?.into()))
        .await
        .map_err(anyhow::Error::new)
}
async fn read_event<
    R: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
>(
    r: &mut R,
) -> Result<ClientEvent> {
    loop {
        let m = r
            .next()
            .await
            .ok_or_else(|| anyhow!("connection closed"))??;
        if let Message::Text(t) = m {
            return Ok(serde_json::from_str(&t)?);
        }
    }
}
fn print_event(e: &ClientEvent) {
    match &e.payload {
        ClientPayload::ProjectSnapshot { snapshot } => {
            for item in &snapshot.timeline {
                print_timeline_item(item);
            }
        }
        ClientPayload::Timeline { item } => print_timeline_item(item),
        ClientPayload::Accepted { operation } | ClientPayload::Operation { operation } => println!(
            "[{}] [operation {:?}]",
            gui::format_local_timestamp(operation.updated_at),
            operation.status
        ),
        ClientPayload::Interaction {
            method, payload, ..
        } => {
            let prompt = payload
                .get("message")
                .or_else(|| payload.get("prompt"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| payload.to_string());
            println!(
                "\n[{}] [Action required · {method}] {prompt}",
                gui::format_local_timestamp(e.created_at)
            );
        }
        ClientPayload::Error { message, .. } => eprintln!(
            "[{}] error: {message}",
            gui::format_local_timestamp(e.created_at)
        ),
        _ => println!(
            "[{}] {}",
            gui::format_local_timestamp(e.created_at),
            serde_json::to_string_pretty(e).unwrap()
        ),
    }
}

fn print_timeline_item(item: &TimelineItem) {
    println!(
        "\n[{}] {}",
        gui::format_local_timestamp(item.created_at),
        item.content
            .as_deref()
            .or(item.summary.as_deref())
            .unwrap_or("")
    );
}

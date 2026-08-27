use clap::{Args, Subcommand};

use super::{ExternalArgs, NodeCommand, ProjectCommand, SessionCommand};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    Status {
        #[arg(long)]
        hub: Option<String>,
    },
    /// Save client hub/auth settings for future CLI and GUI use.
    Login(LoginArgs),
    /// Remove saved client auth token.
    Logout,
    /// Print the active client config path/settings, redacting secrets.
    Config,
    Gui,
    Hub(ExternalArgs),
    Node(ExternalArgs),
    Nodes {
        #[command(subcommand)]
        command: NodeCommand,
        #[arg(long)]
        hub: Option<String>,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
        #[arg(long)]
        hub: Option<String>,
        #[arg(long)]
        node_id: String,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
        #[arg(long)]
        hub: Option<String>,
        #[arg(long)]
        node_id: String,
    },
}

#[derive(Args, Debug)]
pub(crate) struct LoginArgs {
    #[arg(long, default_value = "ws://127.0.0.1:8080/ws/client")]
    pub(crate) hub: String,
    #[arg(long, env = "PUMPKINPI_TOKEN")]
    pub(crate) token: String,
}

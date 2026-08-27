use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum SessionCommand {
    List {
        #[arg(long)]
        project_id: Option<String>,
    },
    Create {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        run_as_user: Option<String>,
        #[arg(long, default_value_t = false)]
        run_as_root: bool,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    Attach {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
    },
    Subscribe {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
    },
    Detach {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
    },
    Send {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        provider_account_id: Option<String>,
        message: String,
    },
    Stop {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
    },
    Restart {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        provider_account_id: Option<String>,
    },
    Delete {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        session_id: String,
    },
}

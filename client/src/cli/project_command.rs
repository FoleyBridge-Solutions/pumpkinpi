use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum ProjectCommand {
    List,
    Add {
        #[arg(long)]
        cwd: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        run_as_user: Option<String>,
        #[arg(long, default_value_t = false)]
        allow_root_sessions: bool,
        #[arg(long)]
        default_provider: Option<String>,
        #[arg(long)]
        default_model: Option<String>,
    },
    Get {
        project_id: String,
    },
    Remove {
        project_id: String,
    },
}

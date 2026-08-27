use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    Enroll {
        #[arg(long)]
        hub: String,
        #[arg(long)]
        setup_key: String,
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    Serve {
        #[arg(long)]
        hub: Option<String>,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        local_only: bool,
        #[arg(long, default_value = "127.0.0.1:4242")]
        listen: String,
    },
}

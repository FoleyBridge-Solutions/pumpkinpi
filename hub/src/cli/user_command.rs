use std::path::PathBuf;

use clap::Args;

use super::UserSubcommand;

#[derive(Args, Debug)]
pub(crate) struct UserCommand {
    #[command(subcommand)]
    pub(crate) command: UserSubcommand,
    #[arg(long)]
    pub(crate) data_dir: Option<PathBuf>,
}

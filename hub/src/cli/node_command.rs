use std::path::PathBuf;

use clap::Args;

use super::NodeSubcommand;

#[derive(Args, Debug)]
pub(crate) struct NodeCommand {
    #[command(subcommand)]
    pub(crate) command: NodeSubcommand,
    #[arg(long)]
    pub(crate) data_dir: Option<PathBuf>,
}

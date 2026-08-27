use std::path::PathBuf;

use clap::Args;

use super::ProviderSubcommand;

#[derive(Args, Debug)]
pub(crate) struct ProviderCommand {
    #[command(subcommand)]
    pub(crate) command: ProviderSubcommand,
    #[arg(long)]
    pub(crate) data_dir: Option<PathBuf>,
}

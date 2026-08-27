use clap::Parser;

use super::Command;

#[derive(Parser, Debug)]
#[command(name = "pumpkinpi-node", version, about = "PumpkinPi node daemon")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

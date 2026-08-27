use clap::Parser;

use super::Command;

#[derive(Parser, Debug)]
#[command(name = "pumpkinpi", version, about = "PumpkinPi client CLI")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

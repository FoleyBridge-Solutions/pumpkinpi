use clap::Parser;

use super::HubSubcommand;

#[derive(Parser, Debug)]
#[command(name = "pumpkinpi-hub", version, about = "PumpkinPi hub daemon")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: HubSubcommand,
}

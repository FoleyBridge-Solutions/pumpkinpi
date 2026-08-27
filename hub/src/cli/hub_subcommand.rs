use clap::Subcommand;

use super::{NodeCommand, ProviderCommand, ServeArgs, UserCommand};

#[derive(Subcommand, Debug)]
pub(crate) enum HubSubcommand {
    Serve(ServeArgs),
    Node(NodeCommand),
    User(UserCommand),
    Provider(ProviderCommand),
}

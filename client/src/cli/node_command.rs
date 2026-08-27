use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum NodeCommand {
    List,
    Get { node_id: String },
}

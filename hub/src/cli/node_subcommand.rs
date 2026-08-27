use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum NodeSubcommand {
    Create { name: String },
    List,
    Revoke { node_id: String },
    IssueSetupKey { node_id: String },
    Disable { node_id: String },
    RotateKey { node_id: String },
}

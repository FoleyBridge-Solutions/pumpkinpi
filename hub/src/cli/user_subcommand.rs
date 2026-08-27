use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum UserSubcommand {
    Create { username: String },
    List,
    GrantNode { user_id: String, node_id: String },
}

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum ProviderSubcommand {
    AddApiKey {
        user_id: String,
        provider_id: String,
        #[arg(long)]
        display_name: String,
        #[arg(long)]
        api_key: String,
    },
    List {
        user_id: String,
    },
    Revoke {
        provider_account_id: String,
    },
}

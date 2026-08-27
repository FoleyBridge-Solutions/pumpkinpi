use std::{net::SocketAddr, path::PathBuf};

use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub(crate) listen: SocketAddr,
    #[arg(long)]
    pub(crate) data_dir: Option<PathBuf>,
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub(crate) public_url: String,
    #[arg(long, env = "PUMPKINPI_ADMIN_TOKEN")]
    pub(crate) admin_token: Option<String>,
}

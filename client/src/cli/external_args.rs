use std::ffi::OsString;

use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct ExternalArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub(crate) args: Vec<OsString>,
}

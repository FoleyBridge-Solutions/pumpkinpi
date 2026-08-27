use pumpkinpi_protocol::protocol_version;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct EnrollRequest {
    #[serde(default = "protocol_version")]
    pub(crate) protocol_version: u32,
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    pub(crate) setup_key: String,
    pub(crate) hostname: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) public_key: Option<String>,
}

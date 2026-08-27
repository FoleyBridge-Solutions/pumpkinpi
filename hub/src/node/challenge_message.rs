use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct ChallengeMessage {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) protocol_version: u32,
    pub(crate) nonce: String,
    pub(crate) expires_at: u64,
}

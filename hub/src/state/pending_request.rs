use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct PendingRequest {
    pub(crate) client_id: String,
    pub(crate) external_id: Value,
}

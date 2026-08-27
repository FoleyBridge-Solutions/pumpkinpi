#[derive(Debug, Clone)]
pub(crate) struct ExtensionUiRequest {
    pub(crate) request_id: String,
    pub(crate) method: String,
    pub(crate) origin_client_id: Option<String>,
    pub(crate) created_at: u64,
}

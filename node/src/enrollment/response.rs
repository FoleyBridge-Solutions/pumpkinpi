use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct EnrollResponse {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) node_id: Option<String>,
    pub(crate) hub_url: Option<String>,
    pub(crate) error: Option<String>,
}

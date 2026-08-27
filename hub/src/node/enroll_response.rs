use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct EnrollResponse {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) node_id: String,
    pub(crate) hub_url: String,
}

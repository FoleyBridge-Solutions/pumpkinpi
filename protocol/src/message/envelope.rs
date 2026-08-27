use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::version::protocol_version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(default = "protocol_version")]
    pub protocol_version: u32,
    pub id: Option<Value>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

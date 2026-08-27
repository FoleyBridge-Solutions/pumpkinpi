use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::NodeStatus;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct NodeRecord {
    pub(crate) node_id: String,
    pub(crate) name: String,
    pub(crate) hostname: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) status: NodeStatus,
    pub(crate) setup_key_hash: Option<String>,
    pub(crate) setup_key_expires_at: Option<u64>,
    pub(crate) token_hash: Option<String>,
    pub(crate) public_key: Option<String>,
    pub(crate) capabilities: Vec<String>,
    pub(crate) projects: Vec<Value>,
    pub(crate) sessions: Vec<Value>,
    #[serde(default)]
    pub(crate) inventory_revision: Option<u64>,
    pub(crate) created_at: u64,
    pub(crate) enrolled_at: Option<u64>,
    pub(crate) last_seen_at: Option<u64>,
    pub(crate) revoked_at: Option<u64>,
}

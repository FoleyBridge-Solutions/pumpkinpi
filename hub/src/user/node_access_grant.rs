use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct NodeAccessGrant {
    pub(crate) user_id: String,
    pub(crate) node_id: String,
    pub(crate) created_at: u64,
}

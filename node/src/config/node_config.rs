use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NodeConfig {
    pub(crate) node_id: String,
    pub(crate) hub_url: String,
    #[serde(default)]
    pub(crate) node_token: Option<String>,
    #[serde(default)]
    pub(crate) trusted_roots: Vec<PathBuf>,
    #[serde(default)]
    pub(crate) root_session_user_ids: Vec<String>,
    #[serde(default)]
    pub(crate) max_concurrent_sessions: Option<usize>,
    #[serde(default)]
    pub(crate) max_sessions_per_project: Option<usize>,
}

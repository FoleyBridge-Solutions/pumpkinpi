use serde::{Deserialize, Serialize};

use super::crash::CrashInfo;
use super::status::{SessionStatus, default_session_status};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    #[serde(default)]
    pub node_id: String,
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default = "default_session_status")]
    pub status: SessionStatus,
    #[serde(default)]
    pub run_as_user: Option<String>,
    #[serde(default)]
    pub run_as_root: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub pi_session_id: Option<String>,
    #[serde(default)]
    pub pi_session_file: Option<String>,
    #[serde(default)]
    pub pi_leaf_id: Option<String>,
    #[serde(default)]
    pub pi_session_name: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub last_active_at: Option<u64>,
    #[serde(default)]
    pub crash: Option<CrashInfo>,
}

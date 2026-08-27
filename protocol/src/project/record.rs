use serde::{Deserialize, Serialize};

use super::status::{ProjectStatus, default_project_status};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub project_id: String,
    #[serde(default)]
    pub node_id: String,
    pub name: String,
    pub cwd: String,
    #[serde(default)]
    pub default_pi_args: Vec<String>,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub run_as_user: Option<String>,
    #[serde(default)]
    pub allow_root_sessions: bool,
    #[serde(default = "default_project_status")]
    pub status: ProjectStatus,
    #[serde(default = "default_true")]
    pub trusted: bool,
    #[serde(default)]
    pub created_at: u64,
    pub updated_at: u64,
}

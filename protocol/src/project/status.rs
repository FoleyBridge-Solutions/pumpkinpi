use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Missing,
    Stale,
    Removed,
}

pub(crate) fn default_project_status() -> ProjectStatus {
    ProjectStatus::Active
}

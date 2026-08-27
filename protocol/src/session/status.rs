use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Starting,
    Idle,
    Running,
    Stopped,
    Crashed,
    Missing,
    Stale,
}

pub(crate) fn default_session_status() -> SessionStatus {
    SessionStatus::Idle
}

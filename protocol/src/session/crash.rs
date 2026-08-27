use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashInfo {
    pub exit_status: Option<i32>,
    pub signal: Option<i32>,
    pub stderr_tail: Vec<String>,
    pub crashed_at: u64,
}

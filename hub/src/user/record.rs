use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct UserRecord {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) token_hash: String,
    #[serde(default)]
    pub(crate) auth_identities: Vec<String>,
    #[serde(default)]
    pub(crate) client_preferences: Map<String, Value>,
    #[serde(default)]
    pub(crate) recently_used: Map<String, Value>,
    #[serde(default)]
    pub(crate) provider_preferences: Map<String, Value>,
    #[serde(default)]
    pub(crate) default_session_settings: Map<String, Value>,
    #[serde(default)]
    pub(crate) audit_metadata: Map<String, Value>,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

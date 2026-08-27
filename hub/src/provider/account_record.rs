use serde::{Deserialize, Serialize};

use super::EncryptedSecret;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ProviderAccountRecord {
    pub(crate) provider_account_id: String,
    pub(crate) user_id: String,
    pub(crate) provider_id: String,
    pub(crate) display_name: String,
    pub(crate) auth_type: String,
    pub(crate) encrypted_secret: EncryptedSecret,
    #[serde(default)]
    pub(crate) available_models: Vec<String>,
    #[serde(default)]
    pub(crate) default_model: Option<String>,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) revoked_at: Option<u64>,
}

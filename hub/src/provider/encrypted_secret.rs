use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct EncryptedSecret {
    pub(crate) nonce: String,
    pub(crate) ciphertext: String,
}

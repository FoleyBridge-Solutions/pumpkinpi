use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NodeStatus {
    Offline,
    Online,
    Disabled,
    Revoked,
}

use std::collections::HashMap;

use pumpkinpi_protocol::{ProjectRecord, SessionRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct NodeStore {
    pub(crate) projects: HashMap<String, ProjectRecord>,
    pub(crate) sessions: HashMap<String, SessionRecord>,
}

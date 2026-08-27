use std::{collections::HashMap, sync::Arc};

use serde_json::Value;
use tokio::sync::{Mutex, mpsc};

use super::RuntimeHandle;

pub(crate) type RuntimeMap = Arc<Mutex<HashMap<String, RuntimeHandle>>>;
pub(crate) type HubTx = mpsc::UnboundedSender<Value>;

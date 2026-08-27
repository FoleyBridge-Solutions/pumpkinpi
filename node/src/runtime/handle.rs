use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use serde_json::Value;
use tokio::sync::{Mutex, mpsc};

use super::{ExtensionUiRequest, RuntimeCommand};

#[derive(Clone)]
pub(crate) struct RuntimeHandle {
    pub(crate) lifecycle_tx: mpsc::UnboundedSender<RuntimeCommand>,
    pub(crate) unblock_tx: mpsc::UnboundedSender<RuntimeCommand>,
    pub(crate) cancellation_tx: mpsc::UnboundedSender<RuntimeCommand>,
    pub(crate) normal_tx: mpsc::UnboundedSender<RuntimeCommand>,
    pub(crate) pending_extension_ui: Arc<Mutex<HashMap<String, ExtensionUiRequest>>>,
    pub(crate) recent_events: Arc<Mutex<VecDeque<Value>>>,
}

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use axum::extract::ws::Message;
use serde_json::Value;
use tokio::sync::{Mutex, mpsc};

use crate::HubStore;

use super::{PendingRequest, SessionKey};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) store: Arc<Mutex<HubStore>>,
    pub(crate) node_channels: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    pub(crate) client_channels: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    pub(crate) client_users: Arc<Mutex<HashMap<String, String>>>,
    pub(crate) in_flight: Arc<Mutex<HashMap<String, PendingRequest>>>,
    pub(crate) subscriptions: Arc<Mutex<HashMap<String, HashSet<SessionKey>>>>,
    pub(crate) recent_events: Arc<Mutex<HashMap<SessionKey, VecDeque<Value>>>>,
    pub(crate) data_dir: PathBuf,
    pub(crate) public_url: String,
    pub(crate) admin_token: Option<String>,
}

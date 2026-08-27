#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SessionKey {
    pub(crate) node_id: String,
    pub(crate) project_id: String,
    pub(crate) session_id: String,
}

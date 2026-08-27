use serde_json::Value;

pub(crate) enum RuntimeCommand {
    Pi {
        command: Value,
        origin_client_id: Option<String>,
        origin_external_id: Value,
    },
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPolicy {
    Allowed,
    DeniedSessionBinding,
    Unknown,
}

pub fn pi_command_policy(command_type: &str) -> CommandPolicy {
    match command_type {
        "new_session" | "switch_session" | "fork" | "clone" => CommandPolicy::DeniedSessionBinding,
        "prompt"
        | "steer"
        | "follow_up"
        | "abort"
        | "clear_queue"
        | "get_state"
        | "get_messages"
        | "set_model"
        | "cycle_model"
        | "get_available_models"
        | "set_thinking_level"
        | "cycle_thinking_level"
        | "get_available_thinking_levels"
        | "set_steering_mode"
        | "set_follow_up_mode"
        | "compact"
        | "set_auto_compaction"
        | "set_auto_retry"
        | "abort_retry"
        | "bash"
        | "abort_bash"
        | "get_session_stats"
        | "export_html"
        | "get_fork_messages"
        | "get_entries"
        | "get_tree"
        | "get_last_assistant_text"
        | "set_session_name"
        | "get_commands"
        | "extension_ui_response" => CommandPolicy::Allowed,
        _ => CommandPolicy::Unknown,
    }
}

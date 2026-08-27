# Commands

## Command Categories

### Hub-level Commands

```json
{"type":"hub.status"}
{"type":"node.list"}
{"type":"node.get","node_id":"node_home"}
```

### Project Commands

```json
{"type":"project.list","node_id":"node_home"}
{"type":"project.add","node_id":"node_home","cwd":"/home/me/app","name":"app"}
{"type":"project.remove","node_id":"node_home","project_id":"proj_api"}
{"type":"project.get","node_id":"node_home","project_id":"proj_api"}
```

### Session Commands

Session commands should include full routing metadata. `project_id` is optional only when `session_id` is globally unique and the hub/node can resolve it unambiguously.

```json
{"type":"session.create","node_id":"node_home","project_id":"proj_api","name":"fix-tests"}
{"type":"session.list","node_id":"node_home","project_id":"proj_api"}
{"type":"session.attach","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests"}
{"type":"session.detach","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests"}
{"type":"session.subscribe","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests"}
{"type":"session.stop","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests"}
{"type":"session.restart","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests"}
{"type":"session.delete","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests"}
{"type":"session.send","node_id":"node_home","project_id":"proj_api","session_id":"sess_tests","command":{"type":"prompt","message":"hello"}}
```

### Pi RPC Command Adapter

The session command payload should use PumpkinPi command types that the Node translates into documented Pi RPC commands. Initially these may closely match Pi RPC names, including:

- `prompt`
- `steer`
- `follow_up`
- `abort`
- `clear_queue`
- `new_session` deny unless wrapped
- `get_state`
- `get_messages`
- `set_model`
- `cycle_model`
- `get_available_models`
- `set_thinking_level`
- `cycle_thinking_level`
- `get_available_thinking_levels`
- `set_steering_mode`
- `set_follow_up_mode`
- `compact`
- `set_auto_compaction`
- `set_auto_retry`
- `abort_retry`
- `bash`
- `abort_bash`
- `get_session_stats`
- `export_html`
- `switch_session` deny unless wrapped
- `fork` deny unless wrapped
- `clone` deny unless wrapped
- `get_fork_messages`
- `get_entries`
- `get_tree`
- `get_last_assistant_text`
- `set_session_name`
- `get_commands`
- `extension_ui_response`

Important security caveat: dangerous behavior is not limited to direct RPC `bash`. Prompts can induce tool calls, extension commands can execute immediately, and `/skill:*` or prompt-template expansion can change behavior. Since Node access is admin-level access, PumpkinPi should expose these behaviors clearly in UX and audit. The Node should distinguish:

- direct RPC `bash`, which emits `bash_execution_update`
- agent tool executions, which emit `tool_execution_*`
- extension commands invoked through `prompt`
- project/user/path skills and prompt templates returned by `get_commands`

Session-switching commands must remain denied until PumpkinPi wrappers can update the Node session registry atomically.

## Per-Session Queue Priority

Commands are serialized per session, but cancellation and UI commands must not get stuck behind long-running normal work. Each session queue should have priority lanes:

1. **Lifecycle / emergency**: `session.stop`, process kill after timeout, crash cleanup. These are handled by the Node and may bypass Pi stdin.
2. **Interactive unblock**: `extension_ui_response`. If a dialog request is pending, route this immediately to Pi before normal queued commands.
3. **Cancellation**: `abort_bash`, `abort_retry`, `clear_queue`, `abort`. These should be accepted while Pi is running and written to Pi as soon as stdin is available, ahead of ordinary commands.
4. **Normal commands**: `prompt`, `steer`, `follow_up`, model/settings/state/bash/session queries.

Rules:

- `extension_ui_response` is valid only for a pending request id; stale or duplicate responses are rejected/dropped.
- `abort_bash` targets the currently running direct RPC `bash` command for that session.
- `clear_queue` should precede `abort` for “stop everything” UX because Pi may otherwise continue queued steering/follow-up messages after abort.
- `session.stop` should first attempt graceful Pi shutdown/abort, then kill the subprocess after a configurable timeout.
- Normal commands submitted while a session is `crashed`, `missing`, `stopped`, or `stale` are rejected unless the command is an allowed lifecycle/diagnostic command.


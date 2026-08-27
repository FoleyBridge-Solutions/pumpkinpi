# Events

The Node consumes Pi events, updates its local session state, and emits normalized PumpkinPi API events to the Hub. Raw Pi events are internal Node inputs, not the Hub/Client protocol.

Important Pi event inputs for the Node to understand:

- `agent_start`
- `agent_end`
- `agent_settled`
- `turn_start`
- `turn_end`
- `message_start`
- `message_update`
- `message_end`
- `bash_execution_update`
- `tool_execution_start`
- `tool_execution_update`
- `tool_execution_end`
- `queue_update`
- `compaction_start`
- `compaction_end`
- `auto_retry_start`
- `auto_retry_end`
- `summarization_retry_scheduled`
- `summarization_retry_attempt_start`
- `summarization_retry_finished`
- `extension_error`
- `extension_ui_request`

Node lifecycle state should treat `agent_settled` as the reliable transition to idle. `agent_end` is only a low-level run completion and may be followed by automatic retry, compaction retry, overflow compaction, or queued continuations.

## Response Correlation

Multiple clients may send commands to the same session, so request IDs can collide.

Pi events generally do not include request IDs. Normal agent events must be treated as session-broadcast events. Direct RPC `bash` is an exception: `bash_execution_update.id` matches the originating bash command ID when provided.

Recommended strategy for command responses and bash update correlation:

```text
external id: req-7
internal pi id: client_abc:req-7
```

Node keeps a map:

```text
(session_id, internal_pi_id) -> (client_id, external_id)
```

When Pi emits a response, Node rewrites the ID back before routing it to the originating client.

Broadcast session events go to all subscribed clients.

The node should not promise per-client causality for ordinary agent events unless Pi exposes sufficient metadata. It may annotate events with best-effort `origin_client_id` internally, but clients must rely on `node_id`, `project_id`, and `session_id` for demultiplexing.


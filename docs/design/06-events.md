# Events

The Spoke consumes Pi events, updates its local session state, and emits normalized PumpkinPi API events to the Hub. Raw Pi events are internal Spoke inputs, not the Hub/Client protocol.

Important Pi event inputs for the Spoke to understand:

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

Spoke lifecycle state should treat `agent_settled` as the reliable transition to idle. `agent_end` is only a low-level run completion and may be followed by automatic retry, compaction retry, overflow compaction, or queued continuations.

## Normalized Event Families

Public PumpkinPi events should be stable product events. Suggested families:

```text
spoke.online / spoke.offline / spoke.updated
project.initializing / project.ready / project.updated / project.missing / project.removed
intent.message_added / intent.question / intent.decision_recorded
intent.update_started / intent.updated / intent.conflicted / intent.unavailable
work.started / work.progress / work.blocked / work.completed / work.failed / work.cancelled
evidence.added / outcome.reported
timeline.item_added / timeline.item_updated / timeline.gap
extension_ui.requested / extension_ui.answered / extension_ui.timed_out
diagnostics.notice / diagnostics.warning / diagnostics.error
```

Internal/diagnostic event families additionally include Session lifecycle and command lifecycle events.

Every primary event that targets work includes `spoke_id` and `project_id`; internal events additionally include `session_id`/`run_id`. Intent updates and outcomes include the relevant Source of Intent revision. Timeline events carry a monotonically increasing Intent Chat- or Session-local cursor so clients can replay and detect gaps.

Raw Pi event names may inform implementation, but clients should not need to understand Pi internals to render normal UI.

## Response Correlation

Multiple clients may send commands to the same session, so request IDs can collide.

Pi events generally do not include request IDs. Normal agent events must be treated as session-broadcast events. Direct RPC `bash` is an exception: `bash_execution_update.id` matches the originating bash command ID when provided.

Recommended strategy for command responses and bash update correlation:

```text
external id: req-7
internal pi id: client_abc:req-7
```

Spoke keeps a map:

```text
(session_id, internal_pi_id) -> (client_id, external_id)
```

When Pi emits a response, Spoke rewrites the ID back before routing it to the originating client.

Internal Session events go to orchestration and explicit diagnostic subscribers. PumpkinPi promotes relevant questions, progress, outcomes, evidence, and failures into Project/Intent Chat events for normal Clients.

The spoke should not promise per-client causality for ordinary agent events unless Pi exposes sufficient metadata. It may annotate events with best-effort `origin_client_id` internally, but clients must rely on `spoke_id`, `project_id`, and `session_id` for demultiplexing.


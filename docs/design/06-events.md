# Events

Slice converts native provider/model/tool activity into stable PumpkinPie Session and Project events. Raw provider wire events never become public API.

## Internal Session Events

```text
session.started/idle/stopped/stale/crashed
run.started/completed/failed/cancelled
model.started/text_delta/tool_call_delta/completed
provider.retry/rate_limited/usage/error
tool.requested/started/progress/completed/failed/cancelled
interaction.requested/resolved/timed_out
context.compaction_started/completed/failed
```

Every event has session/run sequence, correlation where applicable, authoritative time, typed/redacted payload, and visibility. Hidden reasoning content is not retained as normal events.

## Project Timeline Events

```text
intent.message_added/question/decision/update_started/updated/conflicted
operation.accepted/progress/blocked/completed/failed/cancelled
realization.iteration_started/checkpointed/validation/reviewed
review.findings/approved
divergence.opened/verified/reopened/superseded
project.offline/stale/recovered/removed
```

Only useful intent, decisions, risk, progress, outcomes, evidence, questions, and failures are primary. Tool chatter and provider retries remain detail/diagnostics unless they materially affect work.

## Correlation

Hub assigns collision-safe routed request IDs namespaced independently of Slice GUI-chosen IDs. Slice operation, Session, Run, ToolCall, Interaction, and event IDs provide end-to-end correlation. Direct provider tool-call IDs are private runtime metadata and never sole public identity.

## Streaming and Snapshots

Assistant deltas are ephemeral/detail until assembled into a durable message/result. Tool output streams to bounded views while complete artifacts persist separately. Reconnect receives current snapshot then cursor replay; deduplication uses stable IDs/sequence, not text or receipt time.

## Unknown and Diagnostic Events

Unknown provider fields/events may be retained redacted and bounded for diagnostics but cannot drive control transitions. Unknown native event types at a versioned boundary are rejected or explicitly capability-gated.

## Time and Ordering

Display uses authoritative event time with explicit local offset. Causal ordering uses stream sequence/cursor and IDs. Wall-clock timestamps never replace monotonic inventory/event revision.

# Commands

Commands are PumpkinPie/Slice domain types, never raw provider or external-agent commands.

## Product Commands

```text
hub.status
slice.list/get/disable/revoke/rotate_key
project.initialize/list/get/status/remove
intent.send/subscribe/get_projection/cancel
interaction.answer
provider.list/set/revoke
operation.cancel
```

Example:

```json
{"type":"intent.send","slice_id":"slice_home","project_id":"proj_app","message":"Make reconnect durable","expected_revision":4}
```

`intent.send` may clarify/correct/adopt/prioritize/pause/resume intent or request a projection. The serialized Intent Agent proposes typed acts/actions; Slice validates and commits.

## Local Slice Commands

Standalone/local IPC additionally supports:

```text
session.start/resume/list/archive/export
interactive.prompt/cancel
realization.start/pause/resume/cancel
project.trust/policy
runtime.doctor
```

These use the same operation/event machinery and cannot bypass Project policy.

## Native Runtime Commands

Internal typed commands include start/resume turn, cancel, submit interaction response, request compaction, and inspect redacted state. Model tool calls are not Session commands; they pass through tool schema/policy and become durable ToolCall records.

No command switches an opaque external session binding. Session resume/fork/archive are Slice-owned transactions over native SQLite events/context.

## Queueing

Each Session serializes model turns. Priority order is:

1. cancellation/abort and process kill deadline;
2. interaction answer;
3. owner steering explicitly allowed by mode;
4. current-turn continuation/tool result;
5. ordinary prompt;
6. background compaction/maintenance.

Project and Slice concurrency/resource limits are enforced before Run start. Queue admission and rejection are durable and visible when they affect user work.

## Validation

Before execution Slice validates protocol version, route/Project ownership, IDs, expected Source revision, operation state, trust/policy, purpose/tool policy, provider capability, effective identity, queue limit, deadline, and schema/size limits. Unknown commands and fields at authority boundaries are rejected.

## Cancellation

Cancellation targets an operation and propagates to objective, Runs, provider streams, queued/active tools, interactions, and timeline. Graceful child termination is bounded, then process groups are killed. Late provider/tool output is stale diagnostic input only.

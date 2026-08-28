# Protocol

PumpkinPie has three typed transport roles and one internal native event boundary. There is no external-agent RPC layer.

## Slice GUI–Hub Control Channel

Authenticated `slice gui` instances connect to the Hub with owner-control credentials over versioned HTTP/WebSocket APIs. One connection multiplexes commands/subscriptions across enrolled serve endpoints and Projects.

```json
{"protocol_version":4,"id":"req-1","type":"intent.send","slice_id":"slice_home","project_id":"proj_app","message":"Improve reconnect recovery","expected_revision":4}
```

Responses preserve the GUI request ID. Events carry authoritative creation time, stable object IDs, Project/Slice route, cursor/sequence where applicable, and typed payload.

## Hub–Slice Serve Endpoint Channel

An enrolled `slice serve` maintains an authenticated outbound WebSocket to the Hub using endpoint identity distinct from GUI owner credentials. The connection carries:

- challenge/authentication/key lifecycle;
- heartbeat and capability/version negotiation;
- monotonic revisioned complete/partial inventory;
- routed typed commands with internal collision-safe correlation IDs;
- Project snapshots and normalized events;
- scoped provider capabilities only for Runs that require them;
- cancellation and interaction answers.

The Hub never forwards a GUI-chosen request ID as the sole internal correlation key. It creates a unique routed request identity and maps it back to GUI connection/external ID.

## Local Slice IPC

When `slice serve` owns active local execution, standalone Slice TUI/CLI may explicitly attach over authenticated local IPC using the same Slice command/event domain. Unix-domain sockets are preferred on Unix. Peer identity, socket permissions, protocol version, and runtime lease ownership are validated.

Local IPC does not go through the Hub and remains available when unenrolled/offline.

## Native Runtime Boundary

The Slice orchestrator calls an in-process Rust runtime with typed requests/results/events. Provider HTTP payloads are private to provider modules. Raw provider events do not enter PumpkinPie wire protocols.

```text
orchestrator -> NativeTurnRequest -> runtime
runtime -> SessionEvent / NativeTurnResult -> orchestrator
```

Unknown event/tool/output types are rejected by default. Redacted raw provider fragments may be retained only as bounded diagnostics.

## Versioning

Protocol version 4 is the destructive PumpkinPie/Slice rename and native-runtime contract. New APIs use `slice_id` and `Slice*` types; `spoke_id`, legacy executable names, and dual aliases are not retained in the normal API.

Handshake negotiates exact required version plus optional capabilities. A peer with incompatible authority semantics is rejected explicitly, not interpreted best-effort.

## Routing Envelope

Every routed command includes:

```text
protocol_version
routed_request_id
external request mapping retained only at Hub
slice_id
project_id where applicable
operation_id where applicable
source_revision where applicable
command
created_at/deadline
scoped provider capability optional
```

Slice validates route ownership, current revision, authorization, policy, IDs, and command schema before acknowledging acceptance.

## Acknowledgement and Completion

Transport acceptance, durable operation acceptance, and terminal outcome are distinct:

1. Hub acknowledges receipt/routing or correlated failure.
2. Slice persists user message and operation, then returns `accepted`.
3. Timelines/events communicate interpretation, progress, questions, outcomes, review, failure, or cancellation.

Slice GUI instances keep optimistic content visible until the matching durable timeline item is present or correlated failure marks it failed.

## Subscriptions and Replay

Subscriptions are Project/Intent based and resume from durable cursor. Snapshots include project, source metadata, chat, timeline, operations, reviews, divergences, requirement index metadata, interactions, telemetry, freshness, and replay-gap declaration.

Complete inventory may terminally reconcile absence; partial inventory never deletes omitted cache entries. Inventory revisions are strictly monotonic counters, not timestamps. Offline cache is projected explicitly stale.

## Provider Capabilities

Provider secrets are not ordinary command fields. The Hub delivers an encrypted/scoped capability only for a native Run launch requiring that account. Slice keeps it in memory/provider storage policy, excludes it from events/logs/tools, and reports only account reference/provider/model/availability.

## Interaction Correlation

Interaction requests carry interaction/session/run/tool/operation IDs, method, schema, deadline, and blocking state. Answers route to the exact pending interaction. Slice accepts the first valid answer and emits a terminal resolution; duplicates/stale answers receive correlated rejection.

## Errors

Errors are typed by layer:

- transport/auth/version/routing;
- stale revision or ownership;
- policy/trust/identity;
- provider/runtime/tool;
- persistence/corruption/conflict;
- unavailable/offline/replay gap.

Normal GUI errors are concise and correlated. Detailed redacted diagnostics use secondary gated APIs.

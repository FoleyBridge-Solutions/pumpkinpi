# Native Sessions and Runs

Sessions and Runs are Slice execution machinery. Users normally interact with Slice's coding conversation or a Project Intent Chat, not runtime process management.

## Roles

Every Session declares one purpose:

- `interactive`: standalone bounded coding conversation;
- `intent`: interpret owner conversation and propose Source changes;
- `inspection`: read-only situated observations;
- `implementation`: mutate an assigned checkout/worktree toward an objective;
- `validation`: supervised deterministic checks and assessment;
- `review`: independent whole-Project review with reusable valid context/evidence;
- `approval_review`: cold independent final approval candidate;
- `recovery`: diagnose or repair failed machinery.

Every Run binds its actual Source revision, Project reality, operation, objective where applicable, provider/model/account reference, tool policy, and effective identity.

## Concurrency

Commands serialize within a Session. Independent Sessions may stream concurrently subject to per-Project, per-Slice, provider, tool-resource, and policy limits. One Project intent-maintenance lane serializes canonical Source commits; realization promotion is serialized transactionally.

A Slice core owns active runtime leases. TUI, local CLI, and Hub-routed `slice gui` requests submit typed commands to that core rather than opening competing model loops. GUI or terminal disconnect does not stop accepted work by default.

## Persistence

A native Session is an append-only SQLite event stream plus derived context checkpoints. It has no external-agent process or session file. Persist accepted commands, model-visible messages, tool calls/results, interaction boundaries, usage, cancellation, structured outcomes, and compaction metadata. Provider socket state is ephemeral and reconstructed.

Hidden chain-of-thought is neither required nor stored. Derived summaries are bound to exact event ranges and are never authority or evidence.

## Interactive Sessions

Standalone `slice` interactive mode may operate in the selected checkout after explicit trust and mutation-policy display. It provides visible tools/diffs/validation and can resume context. A completed interactive request does not imply Source-of-Intent or whole-Project satisfaction.

## Realization Sessions

Implementation and validation use a per-operation isolated Git worktree. Each successful implementation increment creates a checkpoint. Independent review examines the complete checkpoint against complete current intent. Findings drive another objective; cold approval promotes through a crash-idempotent fast-forward transaction.

Implementer and reviewer context never mix. Warm reviewer state can retain validated complete-review knowledge. Approval review starts with a newly isolated context and fresh evidence required by policy.

## Execution Identity

Slice may run with administrative privilege to manage Projects owned by different local users, but each tool execution has explicit uid/gid/root state. Root is denied unless Project setting, operation need, and local policy all permit it. Identity, writable mounts, network policy, command environment fingerprint, and provider isolation are recorded.

Provider requests execute in Slice. Project tools do not inherit provider or Hub credentials.

## Queueing and Cancellation

Queues prioritize interaction responses and cancellation over ordinary prompts and background work. Cancellation propagates through operation, objective, native provider stream, pending model turn, queued/active tools, interaction, and eventual timeline state.

Graceful command termination has a bounded deadline followed by process-group kill. Provider cancellation closes the stream and records whether billing/remote completion may be unknown. Late results remain stale diagnostics and cannot mutate authority.

## Context Staleness

A context checkpoint is valid only for its bound Source revision, role, Project hashes/observations, and policy. Intent or material reality changes invalidate incompatible observations and approval. Safe reusable content-addressed evidence remains explicit; stale memory is never silently trusted.

## Failure and Recovery

Typed failures distinguish provider rejection/rate limit/transport, invalid model output, tool policy denial, command failure, sandbox failure, runtime bug, persistence failure, interaction timeout, stale revision, and cancellation.

A crash record retains last durable event, role, revision, objective, provider/model, active tool, redacted diagnostic details, exit/signal where a child process died, and recovery policy.

On restart Slice:

1. reconciles Sessions/Runs with durable events;
2. validates worktree, checkpoint, Source, policy, and context bindings;
3. rolls uncommitted implementation state back only according to the recorded phase;
4. resumes from the exact safe phase rather than always repeating implementation;
5. marks uncertain/missing/corrupt state blocked with retained diagnostics;
6. publishes concise Project-level impact and recovery actions.

## Lifecycle

```text
starting -> idle -> running -> idle
                    -> blocked -> running
                    -> stopped
                    -> crashed -> recovering | stopped
                    -> stale
```

A Run is bounded and terminal; a Session may host many Runs. A Project realization persists across Sessions/Runs and reaches satisfaction only through the independent approval contract.

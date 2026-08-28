# Intent-First Implementation Migration

## Objective

Replace the current session-manager product with the intent-first product described by this design:

```text
Personal Hub → Projects → Intent Chat → Source of Intent
```

Pi Sessions remain Spoke-side execution machinery. They disappear from normal Client and CLI workflows.

This is a prerelease migration. We will not preserve the current protocol, persisted stores, CLI shape, GUI state, or Node terminology. When an area is remodeled, delete the old path rather than maintain compatibility layers.

## Current Baseline

The implementation is a useful execution prototype, but its product boundary is the old one:

- The protocol exposes generic JSON envelopes with `node_id`, `project_id`, and `session_id`.
- The Hub authenticates multiple users, stores per-user Node grants, routes Session commands, and caches Session event logs.
- The Node owns Projects, Sessions, Pi runtimes, command validation, execution identity, and local JSON stores.
- The GUI makes users select/create Sessions and renders mostly raw JSON/event output.
- The CLI exposes Node, Project-add, and Session management as primary workflows.
- There is no Source of Intent, Intent Chat, project-level timeline, operation lifecycle, intent revision binding, or Project initialization lifecycle.

The existing code has strong pieces worth retaining: enrollment challenge/response, outbound connection topology, trusted-root and execution-identity checks, Pi process supervision, command priority, crash capture, extension UI correlation, provider-secret encryption/redaction, and basic inventory reconciliation.

## Migration Rules

1. **No compatibility mode.** Increment the protocol version and reject old peers.
2. **No persisted-data migration.** Change data-directory names/schema versions and require deletion/re-enrollment of prerelease state.
3. **No dual vocabulary.** Rename Node to Spoke across binaries, modules, commands, fields, storage, logs, and UI in one pass.
4. **No dual APIs.** Remove normal Client access to `session.*`; do not leave it beside `intent.*` except under an explicitly diagnostic/internal boundary.
5. **No speculative runtime abstraction.** Keep Pi as the one internal Session implementation.
6. **Typed core, dynamic edge.** Domain records, commands, events, and Client state are typed. `serde_json::Value` is allowed only at Pi RPC and redacted diagnostics boundaries.
7. **Build vertical slices.** Each milestone must produce a durable, testable user outcome rather than a collection of disconnected models.
8. **Authority stays explicit.** The Spoke is authoritative for Source of Intent, Intent Chat, evidence, and local execution reality. Hub copies are marked caches.

## Decisions Required Before Feature Work

The design establishes the product model but intentionally leaves several implementation choices open. Resolve these as short ADRs before building orchestration:

### 1. Source of Intent v1 representation

Recommended v1: a versioned structured Markdown document plus typed metadata (`revision`, hash, status, timestamps). Markdown is an implementation choice, not a public editing contract. Store it under the Spoke data directory, not in the Project repository by default.

Define:

- required sections or schema
- maximum size and compaction behavior
- atomic compare-and-swap update algorithm
- corruption detection and backup policy
- what an internal Session receives and how it proposes an update

### 2. Intent Agent control contract

This is the largest blocker. The contract must implement the behavioral semantics in [`16-intent-orchestration.md`](16-intent-orchestration.md), including the separation of conversation, canonical intent, observed reality, bounded work, evidence, and satisfaction. Define how Pi can ask PumpkinPi to:

- propose/commit a Source of Intent revision
- ask a user question
- start inspection, implementation, validation, or independent whole-Project review work
- report progress, evidence, outcome, divergence, reviewer findings, and reviewer approval
- feed every review finding into another bounded implementation iteration
- mark a child Run stale, cancelled, blocked, or complete
- mark Project realization satisfied only after current complete review returns no findings

Do not infer these transitions from arbitrary assistant prose. Use a typed Spoke-controlled contract. An `execute` boolean plus a free-form `work_request` is explicitly insufficient. Prototype context requests, intent proposals, bounded objectives, evidence, divergence, satisfaction assessment, cancellation, and stale-result handling before restructuring the whole system.

### 3. Orchestration concurrency

Choose v1 rules for:

- one serialized intent-maintenance lane per Project
- maximum child Runs per Project/Spoke
- whether implementation and validation may overlap
- what constitutes a material intent change
- stale-run cancellation versus completion-with-warning
- cancellation propagation from operation to Runs and Pi commands

### 4. Hub retention

Specify exactly which Intent Chat items, projections, evidence summaries, and diagnostics the Hub caches, for how long, and whether content is encrypted separately from the Hub store. Until specified, treat the Spoke copy as authoritative and Hub content as a replaceable cache.

### 5. Personal-Hub identity

Replace the current multi-user plus per-Node-grant model with one owner and revocable Client credentials. Do not preserve access-grant machinery as a dormant pseudo-multiuser feature.

### 6. One-Hub Spoke enrollment

The current uncommitted implementation permits one daemon configuration to connect to multiple Hubs. The design describes one personal Hub with many Spokes. Remove multi-Hub support unless the design is deliberately amended before migration.

## Target Crate Boundaries

Keep the four-crate workspace, but change responsibilities:

### `protocol`

Own all stable wire/domain types:

- typed IDs and route types
- Spoke, Project, Source of Intent, Intent Chat, Run, Operation, and Timeline records
- initialization, intent, work, and connection statuses
- Client↔Hub and Hub↔Spoke command/event enums
- cursors, snapshots, replay gaps, errors, and capability negotiation

It must not contain Pi RPC types except private/internal command adapter types if genuinely shared with the Spoke.

### `spoke` (rename current `node`)

Split the current monolithic `main.rs` into:

- enrollment/connection
- Project registry and initialization
- Source of Intent repository
- Intent Chat/timeline repository
- operation repository
- orchestrator/Intent Agent
- internal Session/Run registry
- Pi RPC adapter/runtime
- evidence and diagnostics
- security policy/execution identity
- inventory and reconciliation

### `hub`

Split routing from state/cache concerns:

- owner and Client authentication
- Spoke enrollment/authentication/presence
- Project/Intent cache and inventory reconciliation
- Project subscriptions and cursor replay
- operation request routing/correlation
- provider account store
- audit/diagnostics

### `client`

Implement the layers in `15-client-architecture.md`:

- protocol actor
- typed application store
- Intent Chat UI
- secondary inspector/diagnostics
- CLI commands over the same typed request model

The GUI must not directly manipulate wire JSON.

## Destructive Keep/Gut Map

### Keep and adapt

- Ed25519 Spoke enrollment and challenge authentication
- outbound persistent Spoke connection
- trusted-root canonicalization and local policy
- explicit subprocess execution identity and root checks
- Pi JSONL framing/process supervision
- per-Session priority command dispatch
- crash metadata and stderr tail capture
- extension UI pending-request validation
- provider credential encryption and redaction
- inventory complete/partial semantics

### Gut and replace

- `node_*`, `Node*`, `node_id`, and `pumpkinpi-node` naming
- multi-user records and Node access grants
- multi-Hub Spoke configuration unless separately approved
- generic flattened public `Envelope`
- Hub Session subscriptions as the primary subscription model
- Client-visible Session create/select/attach workflow
- raw Session transcript as the primary timeline
- `project.add` synchronous directory registration
- GUI `Vec<Value>` state and string transcript
- normal CLI `session` workflow
- current JSON store schemas and event-log locations

### Keep only behind diagnostics/internal APIs

- Session list/get/restart/delete
- raw normalized internal Run events
- Pi state and entry queries
- direct bash and low-level queue controls
- raw redacted protocol envelopes

## Delivery Plan

### Milestone 0 — Contract spike and reset policy

Deliverables:

- ADRs for the six decisions above
- a small executable test proving the Intent Agent control contract with Pi
- protocol v3 naming and versioning policy
- documented prerelease reset command/procedure
- acceptance scenarios used by all later milestones

Exit criteria:

- A Pi-backed Intent Agent can produce one typed question or Source of Intent update proposal without parsing arbitrary prose.
- Source of Intent compare-and-swap and hashing behavior is specified and tested.
- No unresolved authority question blocks persistence work.

### Milestone 1 — New typed domain and Spoke rename

Deliverables:

- rename crate/binary/module/config/API vocabulary from Node to Spoke
- replace string IDs with typed ID newtypes
- add typed domain records and status enums
- replace the public generic envelope with typed command/event payloads plus a common routing header
- remove users/access grants in favor of owner Client credentials
- remove multi-Hub configuration unless approved
- bump protocol version and move to fresh data directories

Exit criteria:

- Hub, Spoke, and a minimal CLI authenticate with protocol v3.
- `spoke.list` works with typed messages.
- Old stores and protocol peers fail clearly; there is no fallback path.
- `rg 'node_id|NodeRecord|node\.'` finds only intentional migration notes, if any.

### Milestone 2 — Durable Project, Source of Intent, timeline, and operations

Deliverables:

- Spoke repositories for Projects, Sources of Intent, Intent Chats, Operations, Timeline items, and Evidence metadata
- atomic writes, schema versions, monotonically increasing per-Project timeline cursors, hashes, and revision checks
- Project-level snapshots and cursor replay
- `project.list/get/remove`, `intent.subscribe`, and projection metadata commands
- Hub cache/reconciliation for Project and intent metadata
- explicit stale/offline/gap states

Use simple files/JSONL initially if they meet atomicity and replay requirements; do not add a database merely to avoid defining invariants.

Exit criteria:

- Restarting Hub, Spoke, or Client does not lose acknowledged user timeline items.
- Two competing Source of Intent updates cannot silently overwrite each other.
- A late subscriber reconstructs the same Project timeline or receives an explicit gap.
- Hub cache is visibly stale when the Spoke is offline.

### Milestone 3 — Project initialization vertical slice

Deliverables:

- replace `project.add` with `project.initialize`
- trusted-path validation and typed context inspection in Rust
- initialization states: uninitialized, inspecting, clarifying, ready, failed
- one stable Intent Chat and Source of Intent identity per Project
- initial Intent Agent interaction, focused clarification, Source of Intent assembly, and human-readable summary
- initialization recovery after disconnect/restart

Exit criteria:

- Creating a Project from a directory never lands in blank generic chat.
- The flow inspects context, asks or records clarification, creates revision 1, and reaches `ready`.
- Failed initialization is resumable or removable and leaves diagnostic evidence.

### Milestone 4 — `intent.send` and internal orchestration

Deliverables:

- operation lifecycle from immediate acknowledgement through completion/failure/question
- one serialized intent lane per Project
- internal Session records with purpose, parent operation, and Source of Intent revision
- hidden creation/resume of intent, inspection, implementation, validation, review, and recovery Sessions
- per-operation isolated Git worktrees, checkpoint commits, rollback, and automatic fast-forward promotion after reviewer approval
- durable realization phase/workspace recovery and automatic resume after Spoke restart
- intent revision staleness checks and cancellation propagation
- iterative realization in which every independent whole-Project reviewer finding drives another bounded implementation/validation increment
- reviewer approval only when no fault and no unreviewed required scope remain
- promotion of questions, progress, incremental outcomes, evidence, reviewer findings/approval, divergence, and consequential prompts into Intent Chat
- raw Run activity retained only as detail/diagnostics

Exit criteria:

- A user sends one Project-level message without naming a Session.
- PumpkinPi activates sufficient intent, performs situated work, and reports incremental outcomes against a specific revision with evidence.
- Realization continues through implementation/validation/review iterations until independent whole-Project review finds no fault; resource limits may pause but never produce success.
- Disconnecting the Client does not stop the operation.
- Changing intent during a Run cannot result in a stale success being presented as satisfying current intent.
- Cancelling visible work produces a terminal visible timeline item.

### Milestone 5 — Hub multiplexing, replay, and recovery

Deliverables:

- Project-level subscriptions across all Spokes
- durable cursors and replay from last-seen cursor
- operation request correlation independent of child Sessions
- Project/Intent inventory reconciliation after Spoke reconnect
- promoted crash/offline/missing/conflict events
- secondary internal Run diagnostic subscriptions

Exit criteria:

- One Client receives interleaved updates from multiple Projects/Spokes and routes all items correctly.
- Reconnect restores prior subscriptions and catches up without duplicate timeline items.
- A Spoke or Pi crash produces a concise Project-level explanation plus diagnostics.
- Complete inventory may stale absent objects; partial inventory never does.

### Milestone 6 — Replace the Client and CLI

Deliverables:

- delete the current GUI Session manager rather than evolve it in place
- typed `ConnectionState`, `HubState`, Project summaries, Intent Chats, pending Operations, and Run details
- protocol actor with IDs, timeouts, reconnect, resubscription, cursors, and diagnostic retention
- Projects/recent-work left pane, Intent Chat center pane, contextual inspector right pane
- immediate local user message plus operation state
- explicit loading, empty, offline, stale, blocked, conflict, and recovery states
- normal CLI: `spoke`, `project init/list/status`, `chat`, `intent send/summarize`
- move Session commands under `diagnostics` or a separately gated development command

Exit criteria:

- Normal GUI use exposes no Session creation/selection or raw JSON.
- Every consequential action shows Spoke, cwd, execution user, provider/model, and risk context.
- Background Project work updates without stealing focus.
- CLI and GUI use the same product-level protocol, not separate behavior.

### Milestone 7 — Hardening and product-quality gate

Deliverables:

- crash/restart/missing-session recovery
- Source of Intent conflict/corruption recovery and export
- extension UI first-valid-response and timeout behavior through Intent Chat
- provider/model Project defaults and secure delivery to internal Runs
- redacted audit and diagnostic retention tests
- concurrency limits, queue priority, graceful stop/kill timeout
- fault-injection tests for Hub restart, Spoke restart, Client reconnect, dropped events, stale intent, and Pi crash

Exit criteria:

- All quality-bar items in `14-product-experience.md` have automated or scripted acceptance coverage.
- Ordinary product workflows do not emit or require Pi-specific concepts.
- Recovery always identifies affected Project, intent revision, execution context, evidence, and next action.

## Acceptance Scenarios

Use these as end-to-end tests from Milestone 0 onward:

1. **Initialize an existing repository** — inspect it, clarify intent, create revision 1, show a summary, and become ready.
2. **Adopt broad existing design** — when an initializing Project is told to use its extensive design documents, inspect them, commit comprehensive intent, activate it when sufficiently established, and begin iterative realization; never store canonical intent as a Run outcome.
3. **Simple implementation** — send intent, acknowledge immediately, run internal work, validate, and report evidence.
4. **Bounded completion** — completing one increment reports its evidence and residual divergence, then triggers independent whole-Project review; it does not imply broad Project intent is satisfied.
5. **Reviewer loop** — every reviewer finding drives another bounded implementation iteration, however many iterations are required; only zero findings with complete required scope can approve satisfaction.
6. **Unsupported claim** — fluent Run prose without valid evidence cannot produce a satisfied assessment.
7. **Intent changes mid-run** — supersede or mark old work stale; never claim it satisfies the new revision.
8. **Client disappears** — work continues; another Client or a reconnecting Client replays the same timeline.
9. **Spoke goes offline** — cached Project/Intent state remains visible and stale; live actions are blocked clearly.
10. **Pi crashes** — Project timeline receives a concise failure; diagnostics retain exit information and recovery actions.
11. **Blocking question** — first valid observing Client answer resumes the correct internal Run; duplicates are rejected.
12. **Concurrent Projects** — independent work on two Spokes remains correctly routed and does not serialize globally.
13. **Source conflict/corruption** — work freezes, both conflicting states are preserved, and recovery is explicit.
14. **Consequential execution** — UI identifies machine, path, effective user/root state, provider/model, and risk before action.

## Test Strategy

- **Protocol contract tests:** serialization fixtures, version rejection, route validation, unknown command rejection.
- **Repository tests:** atomic revisions, crash-safe writes, cursor monotonicity, deduplication, replay gaps, corruption handling.
- **Orchestrator tests:** operation state machine, Run purpose/revision binding, stale intent, cancellation, promotion rules.
- **Pi adapter tests:** JSONL framing, event normalization, queue priority, extension UI, metadata refresh, process death.
- **Hub tests:** authentication, routing, cache authority, complete/partial inventory, multiplexed subscriptions.
- **Client store tests:** typed reducers, optimistic user item, correlation, reconnect, replay deduplication, stale/offline states.
- **End-to-end tests:** run the acceptance scenarios with a deterministic fake Pi RPC process before testing against real Pi.

A deterministic fake Pi executable is essential. It should emit scripted deltas, tool events, questions, retries, malformed lines, delayed responses, and crashes.

## Recommended Work Order Within Each Milestone

1. Write typed state-machine and persistence tests.
2. Implement Spoke authority and invariants.
3. Add Hub routing/cache behavior.
4. Add CLI probes for debugging.
5. Add/replace GUI behavior only after the protocol path is stable.
6. Run fault cases before declaring the vertical slice complete.
7. Delete superseded code immediately.

## Definition of Done for the Migration

The migration is complete when a user can initialize and operate Projects entirely through Intent Chat; active durable Source of Intent revisions drive however many hidden situated implementation, validation, and independent review Runs are required for a reviewer to find no fault; findings drive further iterations; outcomes, evidence, questions, approvals, and failures replay across disconnects; execution context remains legible; and Session/Pi mechanics exist only in orchestration and diagnostics.

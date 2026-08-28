# PumpkinPie Native Rust and Slice Migration

## Goal

Destructively migrate the prerelease PumpkinPi/Spoke/external-agent implementation into the product defined by this design:

- product and executable branding is **PumpkinPie**;
- the situated endpoint is **Slice**;
- Slice is a standalone native Rust TUI coding agent and enrollable service;
- all first-party production/test logic is Rust;
- Slice owns provider streaming, model/tool loop, native tools, evidence, context, interactions, persistence, and recovery;
- the legacy coding-agent runtime, Node.js dependency, RPC adapter, and legacy names are deleted;
- active Source of Intent still drives implementation/validation/independent whole-Project review until cold approval finds no fault.

This is a deliberate replacement, not a permanent runtime plugin system or cosmetic rename.

## Non-Negotiable Decisions

1. **Rust only.** No JavaScript, TypeScript, Python application/test sidecar, Node process, provider CLI, or generated fake-agent executable.
2. **One native runtime.** Temporary legacy/native switching may exist only on the migration branch and is deleted at cutover.
3. **One Slice authority.** TUI, CLI, service, and enrolled operation share SQLite, actors, commands, events, policy, and runtime.
4. **Local-first.** `slice .` works with no Hub; enrollment exposes the same state remotely.
5. **Provider calls are native.** Rust HTTPS/streaming implementations; provider credentials never enter Project tool processes.
6. **Typed authority.** Models propose typed values; Slice validates every transition/tool call/evidence binding.
7. **Complete review remains.** Migration/performance cannot narrow intent or weaken independent review/cold approval.
8. **Destructive naming cutover.** Protocol v4 uses Slice/PumpkinPie names only. No permanent `spoke_id`, binary aliases, environment aliases, or dual API.
9. **SQLite authority.** Replace monolithic JSON and external session files; no required Redis.
10. **Delete superseded code promptly.** Milestones include negative deletion gates, not only new scaffolding.

## Target Workspace

```text
runtime/   pumpkinpie-runtime
protocol/  pumpkinpie-protocol
slice/     slice binary: standalone TUI, GUI Client, CLI, serve endpoint, enrollment
hub/       pumpkinpie-hub
```

Target binaries:

```text
slice
pumpkinpie-hub
```

## Keep / Adapt / Delete

### Keep and adapt

- exact Source-of-Intent bundle/hash/coverage semantics;
- intent/realization state machines and divergence ledger;
- worktree checkpoint/rollback/promotion principles;
- Ed25519 enrollment challenge topology;
- Hub routing plus existing Rust GUI foundations, migrated into `slice gui`;
- Rust protocol/domain types after v4 rename;
- Bubblewrap/system-tool supervision concepts;
- provider-secret encryption/redaction principles;
- deterministic validation/evidence and convergence telemetry.

### Replace

- monolithic Slice main module with repositories/actors/runtime modules;
- JSON stores with SQLite plus content-addressed artifacts;
- generic JSON provider/tool edges with typed normalized Rust contracts;
- read-output evidence with complete chunked observations;
- legacy interaction events with native Interaction records/timeouts;
- process/session recovery based on external files with native event checkpoints;
- Python/script fake agents with Rust fake provider/model/tool fixtures.

### Delete by completion

- external coding-agent process launcher and RPC/JSONL parser;
- agent credential/session directory copying;
- external session IDs/files/metadata fields;
- legacy runtime reference documentation;
- Node/runtime/package discovery and installation assumptions;
- `pumpkinpi-*` crate/binary/environment/state names;
- `Spoke*`, `spoke_id`, and `spoke_*` wire/domain names;
- fake executable scripts in tests;
- temporary runtime selector and state converters after cutover.

## Milestone 0 — Freeze Contracts and Measure Baseline

Deliverables:

- commit all revised authoritative design documents;
- ADR for Rust-only boundary, Slice local authority, SQLite, provider support, and system-tool allowance;
- inventory every legacy runtime/name/state/test dependency;
- capture baseline iteration/provider/tool/build timing and existing acceptance behavior;
- freeze new legacy-runtime features except critical migration blockers;
- record the Polaris 0.6 assessment and dependency decision: concepts adopted, subsystems rejected, Apache attribution obligations, and criteria for any selective code adaptation;
- define the five runtime verification phases and the rule that every check lives at its earliest sound phase.

Exit criteria:

- exact role semantics are fixed: `slice` TUI, `slice serve` endpoint, `slice gui` graphical Client;
- complete provider/account support matrix and migration policy;
- executable list, protocol v4 naming, state paths, and deletion gates agreed;
- baseline fixtures can be replayed deterministically.

## Milestone 1 — Protocol v4 and Brand Cutover

Deliverables:

- rename product strings/crates/modules from PumpkinPi to PumpkinPie;
- rename Spoke domain/wire/CLI to Slice (`SliceId`, `slice_id`, `slice_*`);
- protocol v4 fixtures and exact version rejection;
- binaries/package/service templates renamed;
- environment variables and default state/config paths renamed;
- one-time prerelease state migration preserving IDs/history or explicit reset/export path;
- docs/help/UI/assets updated.

Exit criteria:

- normal protocol/source contains no old field/type/executable names;
- Hub, `slice serve`, and `slice gui` interoperate only on v4;
- migrated state preserves Project/Source/timeline/operation authority;
- no permanent aliases or dual writers.

## Milestone 2 — Native Runtime Contracts and Rust Fakes

Deliverables:

- add `pumpkinpie-runtime` crate;
- typed NativeTurn, indexed provider content-block stream, tool call/result, scoped context, usage, interaction, crash, and output-schema contracts;
- versioned role `ExecutionPlan`, stage signature/budget, capability contract, and deterministic runtime-manifest contracts;
- plan construction/composition/Run-start validators implementing the five verification phases without false-positive rejection or under-claimed interfaces;
- immutable-input/typed-output stage boundaries and explicit deterministic reducers instead of a generic mutable runtime god-struct;
- deterministic Rust fake provider server and scripted stream;
- in-process fake tool executor and clock/ID controls;
- malformed stream/tool/output, delay, retry, rate limit, cancellation, and crash fixtures;
- fixed lifecycle events, exactly-once middleware continuation, per-Run inspection controls, and redaction-before-formatting framework;
- scoped child contexts with explicit share-read-only/copy/fresh/exclude crossings and no implicit output merge.

Exit criteria:

- a deterministic Rust test executes a multi-tool turn and validates a typed terminal result;
- manifest drift, invalid plan composition, context leakage, duplicate reducer output, and middleware double/zero-continuation tests fail closed;
- no Python/shell fake-agent application logic remains for migrated tests;
- runtime has no orchestration authority methods.

## Milestone 3 — Native Provider Core

Deliverables:

- direct Rust clients for Anthropic Messages and OpenAI Responses first;
- Google Gemini and OpenAI-compatible/OpenRouter next;
- SSE/stream parsing into checked indexed start/delta/stop events, tool-call assembly only after block completion, structured output, cancellation, retry/backoff, usage, context limits, capability registry;
- provider response/call identifiers and required opaque signatures preserved for protocol round-trip without becoming authority;
- content-hashed prompt-cache plans with adapter reporting of honored/ignored breakpoints;
- canonical local JSON Schema plus explicit provider projection/weakened-constraint report;
- versioned advisory pricing snapshots and full-price/cache-read/cache-write/reasoning usage accounting;
- API-key and OAuth/device/browser flow implemented in Rust;
- explicit account/model selection and redacted diagnostics;
- provider mock contract suites and recorded non-secret fixtures.

Exit criteria:

- supported providers complete text/tool/structured turns without external SDK executables;
- cancellation interrupts stream and produces coherent Run state;
- partial, duplicate, reordered, over-limit, post-terminal, and unterminated stream events never execute tools or change authority;
- every final tool call and structured output passes canonical local schema validation even when provider strict mode accepted it;
- secrets cannot appear in tool environments, logs, events, artifacts, or exports.

## Milestone 4 — Native Tools, Sandbox, and Evidence

Deliverables:

- frozen typed tool catalog separating registration, exposure, authorization, confirmation, and execution;
- monotonic policy composition with persisted exact-call owner grants as the only widening path and mandatory dispatcher revalidation;
- containment-safe read/list/search/edit/write/bash/Git helpers;
- complete chunked file observation with aggregate hash and symlink metadata;
- atomic expected-version mutation and before/after diff evidence;
- Bubblewrap/process-group supervisor with uid/gid/root/mount/network policy;
- complete command artifacts, exit/signal/cancellation/timing/environment/toolchain/checkpoint/cache identity;
- authoritative-document protection;
- bounded content-addressed build/tool cache and cleanup;
- adversarial path/symlink/mount/temporary mutation tests.

Exit criteria:

- files larger than display/tool limits can satisfy exact complete-observation obligations;
- read-only roles cannot cause lasting/temporary Project or shared-host mutation;
- provider/Hub credentials are absent from child processes;
- cancellation kills process groups and records exact result;
- tool/model prose cannot fabricate evidence;
- hidden or denied tools cannot be reached through model dispatch, and generic tool context cannot carry credentials or authority.

## Milestone 5 — SQLite Native Authority

Deliverables:

- versioned Slice SQLite schema in WAL mode;
- repositories for identity, Projects, Source history, chats/timelines, operations/objectives, Sessions/Runs/events, tools/artifacts/evidence, interactions, divergences/reviews, workspaces, audit;
- content-addressed artifact store;
- transactional source commit, acknowledgement, tool lifecycle (`intent -> dispatched -> observed/unknown -> committed`), phase transition, and promotion preparation;
- versioned context-resource storage keys/hashes and all-or-nothing durable checkpoint publication;
- corruption/backup/export/repair and disk-full/fsync tests;
- migrate existing prerelease JSON state;
- local runtime lease and authenticated local IPC.

Exit criteria:

- restart from every durable phase reconstructs exact safe state and reconciles `outcome_unknown` effects before retry;
- required checkpoint failure cannot be reduced to a warning while the Run reports recoverability;
- no full-store rewrite per event/tool;
- TUI/service cannot run competing execution owners;
- no external-agent session file is required.

## Milestone 6 — Native Read-Only Roles

Move roles in this order:

1. validation execution/assessment;
2. initialization/inspection;
3. warm whole-Project reviewer;
4. cold approval reviewer;
5. Intent Agent.

Deliverables:

- native role-specific context checkpoints/tool policies;
- requirement/evidence completeness integration;
- content-addressed evidence reuse for warm review;
- fresh policy-bound evidence for cold approval;
- intent typed proposal and atomic Slice commit;
- native interactions and timeout while provider stream/event consumption remains safe.

Exit criteria:

- all read-only roles run without legacy runtime installed;
- complete review handles arbitrarily large repository files through chunked evidence;
- implementer context cannot enter reviewer Sessions;
- cold approval is newly isolated and cannot rely on stale warm observations;
- intent cannot mutate Project or commit arbitrary prose.

## Milestone 7 — Slice Standalone TUI

Deliverables:

- `slice` crate/executable with Ratatui/Crossterm;
- Project/session selection, conversation, streaming, tool state, diffs, command output, interactions, context/safety panel, usage/retry/compaction;
- `slice .`, `slice run`, session management, auth, doctor;
- attach to service through local IPC or own in-process core when safe;
- terminal resize/paste/color/accessibility/crash restoration;
- local direct-checkout trust/mutation policy.

Exit criteria:

- on a machine with no Hub/Node/legacy agent, `slice .` completes and resumes a native provider/tool coding turn;
- starting service/TUI in either order preserves one Session history and no duplicate execution;
- provider secrets use safe input/storage;
- interactive completion never masquerades as whole-Project satisfaction.

## Milestone 8 — Native Implementation and Realization

Deliverables:

- native implementation Session using isolated worktree policy;
- legacy model-selected objective behavior replaced by Slice-controlled objective packages throughout code/docs;
- checkpoint, deterministic validation, warm complete review, cold approval, promotion loop;
- phase-accurate restart/cancellation/stale-intent recovery;
- local `slice realize` and remote intent use identical orchestration;
- extension/interaction replacement complete.

Exit criteria:

- active broad intent converges using only native Rust runtime;
- every finding drives durable divergence and another bounded objective;
- no resource limit produces success;
- promotion is current-revision/current-reality/cold-approval bound and crash-idempotent.

## Milestone 9 — `slice gui`, Hub, and Serve Integration

Deliverables:

- migrate existing Rust GUI code/assets/state into the `slice` crate behind `slice gui` and delete the separate Client crate/binary;

- revisioned complete/partial Slice inventory for all required metadata;
- collision-safe routed IDs and lossless snapshot cache;
- offline/stale/replay-gap behavior;
- enrolled Slice appears with existing local Projects/Sessions;
- scoped provider capability only at native Run launch;
- local TUI and remote `slice gui` instances observe/cancel/answer the same operations;
- independently revocable GUI owner-control credentials, distinct endpoint keys, and redacted audit.

Exit criteria:

- Hub enrollment is additive to local Slice use;
- Hub disconnect does not break local work or accepted background realization;
- reconnect replays without duplication/loss;
- no GUI/Hub path bypasses serve policy or creates a second authority.

## Milestone 10 — Delete Legacy Runtime and Non-Rust Sources

Deliverables:

- native runtime is unconditional;
- remove temporary runtime enum/feature/config/migration bridge;
- delete launcher/RPC/event parser/session dirs/credential copies and legacy reference file;
- delete external-agent fields and fake executables;
- delete first-party JS/TS/Python/package manifests if any;
- remove Node/legacy-agent packaging/service/install detection;
- clean state/cache directories through one-time migration;
- update threat model, support docs, release packaging, SBOM.

Exit criteria:

```text
No production process discovers or launches Node or an external coding agent.
No first-party application/test source is JS, TS, or Python.
No normal code/wire/docs use legacy product or endpoint names.
A clean release machine has only Rust binaries plus declared system/Project tools.
```

Historical migration notes may name removed systems only in this document and changelog.

## Acceptance Matrix

1. Standalone Slice coding/edit/validation with no Hub or Node.
2. Session resume after terminal exit and Slice restart.
3. Service/TUI local IPC single authority.
4. Enrollment exposes existing local Projects without duplication.
5. Initialization inspects complete situated context and commits Source safely.
6. Broad design adoption preserves exact bundle and activates realization.
7. Native implementation/validation/reviewer loop across repeated findings.
8. Large-file complete review without truncation deadlock.
9. Unsupported model claim cannot become evidence/satisfaction.
10. Intent changes cancel/stale incompatible native Runs.
11. `slice gui` closes while work continues and replays in another GUI.
12. Hub/Slice restart, dropped events, partial inventory, stale cache, replay gap.
13. Provider retry/rate limit/malformed stream/cancellation/OAuth refresh.
14. Tool escape/symlink/root/network/credential isolation attempts.
15. Interaction first-valid response, timeout, duplicate, stale, disconnect.
16. Source/database/artifact/worktree corruption and repair/export.
17. Promotion crash window is idempotently reconciled.
18. Cold approval only for exact current revision/reality/evidence.
19. Provider account revocation/rotation and no secret leakage.
20. Installation/runtime test on host without Node or removed agent.

## Test Strategy

All test harness application logic is Rust:

- protocol serialization/version/unknown-field fixtures;
- compile-fail coverage for generated tool/stage contracts and invalid typed transitions;
- Axum fake provider servers and scripted normalized streams with arbitrary chunk boundaries and malformed event order;
- deterministic in-process runtime/tool/clock/ID fakes;
- temporary real Git repositories/worktrees;
- Bubblewrap adversarial integration tests where available;
- SQLite migration/corruption/fsync/fault injection;
- Ratatui state/snapshot/key handling tests;
- Hub/serve-endpoint/GUI end-to-end tests with dropped/reordered transport;
- property tests for execution-plan/signature composition, stream chunking equivalence, deterministic parallel reduction, schema projection, and event replay;
- provider/tool conformance suites shared by every adapter/implementation;
- provider live smoke tests optional and never the correctness oracle.

## Definition of Done

PumpkinPie is complete for this migration when `slice .` is a high-quality standalone native Rust coding agent, `slice serve` is its enrollable situated endpoint role, and `slice gui` is the only graphical PumpkinPie Client; the same installation can use any combination without conflating credentials or authority; every intent/inspection/implementation/validation/review/recovery role uses direct Rust provider streaming and native tools; SQLite/event/artifact state recovers durably; whole-Project review and cold approval retain all original satisfaction semantics; and the legacy runtime, Node dependency, old product/endpoint names, non-Rust test sidecars, and compatibility layer are deleted.

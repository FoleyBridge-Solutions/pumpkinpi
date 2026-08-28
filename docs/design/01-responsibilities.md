# Responsibilities

The owner maintains intent and makes consequential decisions. PumpkinPie owns durable representation, orchestration, evidence, and user-visible outcomes. Slice performs situated native execution where Project reality lives.

## Hub

The personal Hub owns:

- owner and independently revocable `slice gui` authentication;
- Slice enrollment, challenge, key rotation, disable, revoke, and connection state;
- encrypted provider-account custody, selection metadata, and scoped delivery;
- multiplexed GUI-control/serve-endpoint routing and request correlation;
- revisioned complete/partial Slice inventory cache;
- Project/Intent subscription routing, cursors, replay, and stale projections;
- redacted administrative audit;
- connection-level rate and abuse controls.

The Hub does not own Project files, local execution policy, live native Sessions, tool processes, evidence capture, canonical Source commits, or satisfaction decisions.

## Slice

Slice is authoritative for:

- standalone TUI/CLI/service operation;
- local Project initialization, canonical paths, trust, and policy;
- Source of Intent revisions and exact authoritative bundles;
- Intent Chat, timelines, operations, divergences, evidence, and review records;
- native Rust Session/provider/model/tool runtime;
- provider streaming, context checkpoints, compaction, retries, cancellation, and usage;
- native tools, sandbox policy, process identity, writable surfaces, and command lifecycle;
- isolated realization worktrees, checkpoints, rollback, validation, review, and promotion;
- interaction requests/timeouts and local/remote answer correlation;
- SQLite persistence, content-addressed artifacts, recovery, retention, and inventory publication;
- reconciliation after restart or Hub reconnect.

Slice validates every model-produced proposal before authority changes or tools run.

## Native Runtime

The internal runtime owns mechanics, not product authority:

- normalize provider capabilities and streams;
- assemble tool calls and typed model output;
- enforce per-turn output schema and tool policy;
- execute approved calls through Slice supervisors;
- persist typed events and independently captured results;
- return structured proposals/evidence IDs to orchestration;
- expose cancellation, usage, retry, compaction, and crash state.

It cannot commit Source of Intent, select authorization, promote worktrees, or mark satisfaction independently.

## Intent Orchestrator

The Slice orchestrator owns:

- serialized intent-maintenance lanes;
- controlled context requests and inspection resumption;
- Source proposal compare-and-swap, limits, exact coverage, and history;
- activation/pause/resume/cancel and stale-work decisions;
- requirement graph and durable divergence reconciliation;
- convergence-oriented bounded objective packages;
- implementation/validation/review phase transitions;
- complete-review obligations, evidence validity, cold approval, and promotion;
- recovery from every durable phase;
- precise primary timeline projection.

## Slice TUI and Local CLI (`slice`)

The standalone coding-agent mode owns:

- local Project/session discovery and selection;
- conversation, streaming, tools, diffs, validation, and interactions;
- situated identity/risk display before consequential work;
- attach/detach to one Slice core through local IPC when service mode owns execution;
- direct-checkout interactive policy versus isolated realization policy;
- terminal-safe provider login and credential references;
- diagnostics without exposing secrets or provider raw payloads.

It does not maintain a second local authority or bypass Slice policy.

## Slice GUI (`slice gui`)

The graphical Client mode owns:

- authenticated owner-control Hub connection and reconnect, distinct from serve endpoint identity;
- Project/Slice discovery and selection;
- Intent Chat input, immediate correlated optimistic state, and replay;
- typed reducers for projects, sources, operations, interactions, reviews, divergence, and diagnostics;
- offline/stale/conflict/recovery presentation;
- provider/model/account administration through Hub APIs;
- explicit situated safety context and cancellation/answer controls.

GUI state caches projections/preferences but never becomes execution or canonical authority. Co-location with `slice serve` does not grant a direct authority bypass.

## Owner

The owner:

- explains, corrects, and prioritizes intent;
- adopts authoritative Project documents explicitly;
- answers consequential questions;
- chooses trust, provider accounts, execution identity, and exceptional capabilities;
- may pause, cancel, resume, repair, export, or remove;
- need not manage internal Sessions or approve every autonomous increment.

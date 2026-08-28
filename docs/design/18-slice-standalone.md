# Slice Standalone Coding Agent

## Product Definition

**Slice** is one native Rust tool with three primary roles selected by command:

1. `slice [PATH]` — standalone TUI coding agent;
2. `slice serve` — situated execution endpoint formerly described as the network worker role;
3. `slice gui` — PumpkinPie graphical Hub client.

A person installs one tool and chooses any combination. The modes share native runtime, protocol, domain models, provider support, persistence components, and security policy, but they do not pretend to be one authority when acting in different roles.

```text
slice [PATH]             standalone interactive TUI coding agent
slice gui                graphical PumpkinPie Hub client
slice run [PATH] PROMPT  headless bounded coding turn
slice realize [PATH]     local intent realization and whole-Project review
slice serve              long-running local/enrolled execution endpoint
slice enroll             enroll the serve identity with a PumpkinPie Hub
```

## Local-First Authority

A Slice owns its Projects, Sources of Intent, Sessions, Runs, tools, evidence, worktrees, provider references, interactions, and recovery state in local SQLite storage. The Hub routes and caches; it does not become execution authority.

A Project created locally can later be exposed through enrollment without copying it into a second state model:

```text
use locally -> enroll Slice -> register existing Project -> direct from PumpkinPie
```

Disconnecting or removing the Hub does not make local Slice Sessions unusable. Remote operations already accepted under policy continue durably unless cancelled or made stale by intent/policy changes.

## Shared Tool, Explicit Role Boundaries

The `slice` crate/binary contains shared Rust libraries plus three applications:

- TUI agent: local conversation/runtime in process or attached to local service by explicit choice;
- serve endpoint: local Project authority, scheduler, Hub endpoint channel, and local IPC;
- GUI client: Hub owner-control channel and typed graphical state, without local execution authority merely because it is on the same machine.

TUI and serve reuse the same runtime/tools/repositories rather than implementing two agents. GUI reuses protocol/domain/UI-store modules but never opens Project authority databases or provider/tool loops for remote Projects.

When `slice serve` owns a local Project/runtime lease, standalone TUI may explicitly attach rather than compete. SQLite alone is not a distributed execution lock. Running `slice gui` alongside either mode is safe because GUI state is non-authoritative and separately stored.

## Interaction Modes

### Interactive coding mode

`slice [PATH]` behaves like a high-quality terminal coding agent:

- conversation is the primary surface;
- model/tool streaming is visible;
- diffs, commands, validation, and context usage are inspectable;
- confirmation policy is local and explicit;
- Sessions can be named, resumed, forked through a Slice-owned operation, or archived;
- default mutation is the selected checkout after trust confirmation;
- every mutation has before/after evidence and undo/checkpoint information where Git permits.

Interactive completion means the bounded user request ended. It does not claim whole-Project Source-of-Intent satisfaction unless the user entered realization mode and independent review approved it.

### Intent realization mode

`slice realize [PATH]` uses PumpkinPie's full autonomous contract:

- active complete Source of Intent;
- isolated per-operation Git worktree;
- convergence-oriented bounded objectives;
- deterministic validation;
- independent whole-Project review after every increment;
- cold final approval;
- transactional promotion;
- durable pause/cancel/recovery.

Local and remotely requested realization are semantically identical.

### Headless mode

`slice run` executes one bounded typed turn and emits human text by default or stable JSON with `--json`. It supports scripting without exposing provider-specific payloads. Consequential policy cannot be bypassed merely because no TUI is attached.

### Service mode

`slice serve` owns scheduling, Hub connectivity, local IPC, runtime leases, recovery, and background realization. The TUI may attach/detach without changing work lifetime.

## TUI Information Architecture

The native Ratatui/Crossterm interface has three responsive regions:

```text
Projects/Sessions | conversation, tools, diffs, composer | context/safety
```

It must expose:

- current Project, cwd, repository root, branch/worktree;
- Slice name/machine, effective user, root possibility, trust;
- provider/model/account availability without secret material;
- Session purpose and Source revision;
- streaming assistant output;
- queued/running tool calls and cancellation;
- file diffs and command output with truncation/full-artifact cues;
- context usage, compaction, retry/rate-limit state;
- interaction requests and timeout;
- realization iteration, divergence transitions, review outcome;
- offline/Hub connectivity as secondary context, never as a prerequisite for local use.

Keyboard behavior, accessibility, color, terminal resizing, paste handling, and recovery after terminal loss are product-quality requirements. The TUI must not rely on a browser, webview, Node.js, or external terminal UI runtime.

## CLI Shape

```text
slice [PATH]
slice run [PATH] [--model MODEL] [--json] PROMPT
slice realize [PATH]
slice gui [--hub URL]
slice serve [--hub URL]
slice enroll --hub URL --setup-key KEY
slice auth login PROVIDER
slice auth set-key PROVIDER
slice auth list
slice project init/list/status/remove
slice session list/resume/archive/export
slice evidence list/show/export
slice doctor
slice reset --yes
```

Secrets are read from a terminal-safe prompt, stdin descriptor, environment reference, or platform credential service; API keys are not positional argv values.

Hub administration remains under `pumpkinpie-hub` and may be surfaced in `slice gui`; Slice TUI/serve commands govern local situated authority.

## Project Discovery and Trust

Opening a path canonicalizes it, identifies repository/worktree boundaries, checks symlinks and configured trusted roots, discovers local policy and authoritative design manifests, and shows the effective execution boundary before tools run.

Trust is recorded per canonical Project identity and policy version. A changed repository location, owner, security policy, or dangerous capability can invalidate prior trust. Enrollment does not implicitly trust every local path.

## Provider Accounts

Standalone Slice can hold local encrypted provider accounts or platform credential references. An enrolled Slice may receive an ephemeral account capability from the Hub for a specific Run. Account provenance and model selection are visible; secret values are not.

Hub-delivered credentials do not overwrite local accounts silently. Project/session policy selects an explicit account reference and defines fallback behavior. Provider credentials stay in the native Slice provider client and never enter Project tool sandboxes.

## Session Continuity

A local interactive Session and a remotely observed Session are one identity. Events use durable cursors, and attached TUI/GUI instances replay from their own cursors. Input ownership is explicit when multiple observers are present; first-valid-answer semantics apply only to a specific pending interaction, not arbitrary simultaneous prompts.

Terminal disconnect, Hub disconnect, laptop sleep, provider retry, Slice restart, and GUI switching preserve coherent state and explain whether work is running, queued, paused, blocked, stale, crashed, or complete.

## Enrollment Identity

A configured `slice serve` role has a stable `slice_id`, signing key, display name, machine metadata, and revocation state. Standalone TUI or GUI-only use does not need an enrolled endpoint identity. It may enroll with one personal Hub in the initial product. Re-enrollment, key rotation, disable, revoke, and removal are explicit operations that preserve local state.

## Packaging

Slice is delivered as one Rust binary plus ordinary native/system dependencies. It does not install npm packages, a JavaScript runtime, Python environment, provider CLI, or external coding agent. Release artifacts include shell completions and optional service definitions generated from Rust-owned command metadata/templates.

## Acceptance Scenarios

1. On a machine without Node.js or a Hub, `slice .` authenticates to a supported provider and completes an edit/validation turn.
2. The user exits and resumes the exact Session with history, evidence, and context checkpoint intact.
3. `slice serve` starts while no work is lost; a TUI attaches through local IPC.
4. `slice serve` enrolls, and `slice gui` shows its existing Projects and Sessions through PumpkinPie.
5. A remote intent starts realization; closing every GUI does not stop it.
6. A local TUI observes and can cancel the same remote operation under policy.
7. Interactive direct-checkout work never masquerades as independently approved whole-Project satisfaction.
8. Realization uses isolated worktrees and promotes only after cold approval.
9. Provider credentials are absent from command environments and captured evidence.
10. Removing Hub enrollment leaves local Slice use intact.

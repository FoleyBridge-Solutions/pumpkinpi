# Product Experience

PumpkinPie is the unified place where one person works with Projects through **Intent Chat**. Each Project has a **Source of Intent** that PumpkinPie implements on the Slice where the real context lives, iterating situated implementation, validation, and independent whole-Project review until the reviewer finds no fault.

The product should make three things continuously clear:

1. **What intent is being pursued**: goals, decisions, open questions, and current requested outcome as explained through Intent Chat.
2. **Where work is happening**: slice, project path, execution user, and trust boundary.
3. **What happened / what can happen next**: progress, evidence, blockers, risk, recovery choices, and requested decisions.

## Primary Object Model

The UI should be organized around Projects and Intent Chat, not protocol resources or internal sessions.

User-facing model:

```text
Personal Hub
  └─ Project
      └─ Intent Chat
          ├─ user intent
          ├─ questions / decisions
          ├─ outcome summaries
          ├─ evidence
          ├─ blockers
          └─ human-readable projections of Source of Intent
```

Internal/situated model:

```text
Personal Hub
  └─ Slice
      └─ Project
          ├─ Source of Intent
          ├─ Intent Chat timeline
          ├─ Internal Sessions / Runs
          │   ├─ implementation
          │   ├─ validation
          │   └─ inspection
          └─ Evidence / diagnostics
```

Definitions for product surfaces:

- **Personal Hub**: the unified view of the owner's Projects, Slices, preferences, and recent activity.
- **Slice**: one connected computer contributing real execution context. Slice surfaces expose online/offline/revoked state, version, hostname, and last seen time.
- **Project**: a trusted working directory on a Slice, defined by a Source of Intent and primarily accessed through Intent Chat.
- **Source of Intent**: the canonical LLM-facing representation of project intent. It does not need to be directly human-readable; user-readable summaries/diffs/questions are generated into Intent Chat.
- **Intent Chat**: the primary Project interface. It evolves the Source of Intent and reports progress, decisions, evidence, blockers, and outcomes.
- **Internal Session / Run**: execution machinery used to implement, inspect, validate, and recover. It may be visible as status/evidence/diagnostics but should not become the user's normal workspace.
- **Timeline**: durable user-facing history for Intent Chat plus structured evidence/history from internal runs. Raw protocol JSON is diagnostic detail.

## Core Workflows

### Standalone Slice

1. Run `slice .` in an existing Project on a machine with no Hub requirement.
2. See canonical path/repository, branch, effective identity, trust, provider/model, and mutation policy.
3. Authenticate through a terminal-safe native provider flow.
4. Converse, inspect streaming tools/diffs, answer interactions, validate, and resume the Session later.
5. Optionally start full `slice realize` intent reconciliation in an isolated worktree.
6. Optionally enroll the same Slice; PumpkinPie then shows the same Projects, Sessions, operations, and evidence.

The local TUI and one or more `slice gui` instances are views into one serve authority. Enrollment is additive, not a migration to a different agent.

### First Run / Onboarding

A first-run client should guide the user through:

1. Choose or enter a Hub URL.
2. Authenticate to the Hub.
3. Show enrolled Slices or explain that none are enrolled.
4. Provide a Slice enrollment path.
5. Select a Slice and choose a directory.
6. Inspect local context where allowed.
7. Start the Project's Intent Chat.
8. Ask clarifying questions and assemble the initial Source of Intent.
9. Present a human-readable summary of assumptions, goals, risks, and validation strategy.
10. Let the user correct or confirm through Intent Chat.
11. Begin intent-driven work only after the Project has enough intent to act safely.

The user should never land on an empty pane with only protocol controls or a generic blank chat.

### Daily Use

Common path:

1. Open `slice gui`.
2. See recent Projects and active Intent Chats across all Slices.
3. Resume a Project.
4. Read what PumpkinPie believes the current intent/status is.
5. Say what should change or ask for work.
6. Watch concise progress updates and inspect evidence when desired.
7. Answer questions or approve consequential choices.
8. Let PumpkinPie iterate implementation, validation, and independent whole-Project review internally.
9. Review findings, evidence, and eventual reviewer approval against intent.

### Recovery

Recovery is a first-class workflow, but it should still return through Intent Chat where possible.

Examples:

- Slice offline: show last known Project state and Intent Chat, mark live execution unavailable, offer retry diagnostics.
- Internal run crashed: summarize exit reason, affected intent, stderr tail/evidence, restart/delete/export diagnostics actions.
- Project missing: show previous cwd, explain that identity is still `project_id`, offer remove or rescan.
- Source of Intent unavailable/corrupt: freeze ordinary work, show last known projections/evidence, offer repair/export diagnostics.
- Slice GUI reconnect: resubscribe to previous Project/Intent Chat state and replay durable timeline entries after the last known cursor.

## Information Architecture

Recommended desktop layout:

```text
┌────────────────────────────────────────────────────────────────┐
│ Global bar: Hub, connection, diagnostics, search                │
├───────────────┬───────────────────────────────┬────────────────┤
│ Projects /    │ Intent Chat                   │ Inspector      │
│ recent work   │                               │ context/evidence│
└───────────────┴───────────────────────────────┴────────────────┘
```

- Left pane: recent Projects, grouped or filterable by Slice, with status badges.
- Center pane: selected Project's Intent Chat.
- Right pane: contextual inspector: slice/cwd/effective user/model/provider, current status, evidence, changed files, running internal work, diagnostics, raw event toggle.

The UI should support many active Projects at once. Background work should update the relevant Project's unread/activity state without stealing focus.

## State Surfaces

Every major object should have explicit loading, empty, error, stale, and offline states.

Required examples:

- Global connection: disconnected, authenticating, connected, reconnecting, degraded.
- Slice: online, offline, disabled, revoked, version-mismatch.
- Project: initializing, active, missing, stale, removed, untrusted.
- Source of Intent: absent, assembling, active, updating, conflicted, corrupt/unavailable.
- Intent Chat: ready, waiting-for-user, updating-intent, working, blocked, summarizing, stale.
- Internal Session/Run: starting, idle, running, blocked-on-ui, stopping, stopped, crashed, missing, stale.
- Command/Operation: queued, accepted, running, blocked, completed, failed, cancelled, rejected.

A user message should not disappear after Send. It becomes an Intent Chat item with acknowledgement, status, and eventual outcome or question.

## Product Quality Bar

PumpkinPie stops feeling like a prototype when Slice is also an excellent standalone native terminal coding agent and:

- The main UI is useful without exposing raw JSON or internal session management.
- Each Project has one obvious primary surface: Intent Chat.
- Project initialization produces a usable Source of Intent instead of a blank chat.
- Every user action has visible acknowledgement and eventual completion/failure.
- Reconnect and late-subscribe behavior is boring and reliable.
- Intent Chat history and Source of Intent state are durable enough for users to trust closing the client.
- Dangerous execution context is visible before tools run or files change.
- Incremental outcomes are reported against intent with evidence, without implying whole-Project completion.
- PumpkinPie continues implementing reviewer findings until independent whole-Project review finds no fault; limits pause rather than falsely complete work.
- Divergences between intent and reality become questions, proposed intent updates, or explicit blockers.
- Errors explain what happened, where it happened, what intent was affected, and what the user can do next.
- Protocol and raw run details are available in diagnostics, not required for normal operation.

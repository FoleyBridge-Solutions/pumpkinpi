# PumpkinPie Design

> **PumpkinPie brings the projects and LLM capabilities of all your computers into one personal Hub, while keeping execution grounded on the Slices where the real context lives.**

**Unified at the Hub, situated on the Slice, directed by intent.**

PumpkinPie's primary user-facing object is a Project's **Intent Chat**. The Intent Chat maintains the Project's **Source of Intent**: the living project definition that PumpkinPie implements through repeated situated implementation, validation, and independent whole-Project review until the reviewer finds no fault.

**Slice** is one native Rust tool: `slice` is the standalone TUI coding agent, `slice serve` is the situated execution endpoint, and `slice gui` is the PumpkinPie graphical Client. The Hub is optional for standalone/local use. Sessions, provider streams, tools, queues, commands, and events are execution machinery, not the main user-facing abstraction.

## Reading Order

Start with the product intent, then move into system mechanics:

1. [Overview](00-overview.md) — Product thesis, personal Hub, Slices, Projects, Intent Chat, and constraints
2. [User Mental Model](13-user-mental-model.md) — How intent-driven, unified-but-situated work should make sense to the user
3. [Product Experience](14-product-experience.md) — Workflows, information architecture, states, and quality bar
4. [Source of Intent](03-source-of-intent.md) — The living project definition and Intent Chat loop
5. [Intent Interpretation and Realization](16-intent-orchestration.md) — The distinction between conversation, canonical intent, observed reality, bounded Runs, evidence, and satisfaction
6. [Responsibilities](01-responsibilities.md) — Hub and Slice TUI/serve/GUI role ownership
7. [Data Model](02-data-model.md) — Hub, Slice roles, Project, Source of Intent, Intent Chat, Session, timeline, command, and GUI state
8. [Native Rust Runtime](17-native-runtime.md) — Provider streaming, agent loop, native tools, evidence, sandboxing, and the no-Node boundary
9. [Slice Standalone Experience](18-slice-standalone.md) — TUI, CLI, local authority, connected mode, and interactive versus autonomous work
10. [Sessions](03-sessions.md) — Internal execution sessions, concurrency, identity, lifecycle, and recovery
11. [Protocol](04-protocol.md) — Slice GUI control, Slice serve endpoint, local IPC, and routing
12. [Commands](05-commands.md) — Intent/project commands and per-Session queue behavior
13. [Events](06-events.md) — Normalized events and response correlation
14. [Interaction UI](07-interaction-ui.md) — Native blocking and fire-and-forget interaction requests
15. [Slice Enrollment and Auth](08-slice-enrollment-auth.md) — Enrollment, authentication, and revocation
16. [Persistence](09-persistence.md) — Hub/Slice SQLite storage and reconciliation
17. [Provider Authentication](10-provider-authentication.md) — Provider credentials and model selection
18. [Security Policy](11-security-policy.md) — Personal-Hub trust and local enforcement
19. [CLI](12-cli.md) — PumpkinPie and Slice command-line shape
20. [Slice GUI Architecture](15-slice-ui-architecture.md) — `slice gui` typed state, protocol actor, timeline, reconnect, and diagnostics

## Implementation

- [Native Rust / Slice Migration](IMPLEMENTATION-MIGRATION.md) — Destructive migration from the legacy runtime and names to PumpkinPie and Slice, with no permanent compatibility layer

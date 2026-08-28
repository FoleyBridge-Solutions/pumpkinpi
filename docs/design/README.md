# PumpkinPi Design

> **PumpkinPi brings the projects and LLM capabilities of all your computers into one personal Hub, while keeping execution grounded on the Spokes where the real context lives.**

**Unified at the Hub, situated on the Spoke, directed by intent.**

PumpkinPi's primary user-facing object is a Project's **Intent Chat**. The Intent Chat maintains the Project's **Source of Intent**: the living project definition that PumpkinPi implements through repeated situated implementation, validation, and independent whole-Project review until the reviewer finds no fault.

Pi is the current internal Session implementation. Sessions, queues, commands, and events are execution machinery, not the main user-facing abstraction.

## Reading Order

Start with the product intent, then move into system mechanics:

1. [Overview](00-overview.md) — Product thesis, personal Hub, Spokes, Projects, Intent Chat, and constraints
2. [User Mental Model](13-user-mental-model.md) — How intent-driven, unified-but-situated work should make sense to the user
3. [Product Experience](14-product-experience.md) — Workflows, information architecture, states, and quality bar
4. [Source of Intent](03-source-of-intent.md) — The living project definition and Intent Chat loop
5. [Intent Interpretation and Realization](16-intent-orchestration.md) — The distinction between conversation, canonical intent, observed reality, bounded Runs, evidence, and satisfaction
6. [Responsibilities](01-responsibilities.md) — Hub, Spoke, Client, and internal execution ownership
7. [Data Model](02-data-model.md) — Hub, Spoke, Project, Source of Intent, Intent Chat, Session, timeline, command, and Client data
8. [Sessions](03-sessions.md) — Internal execution sessions, concurrency, identity, lifecycle, and recovery
9. [Protocol](04-protocol.md) — Transport layers, routing, and Client multiplexing
10. [Commands](05-commands.md) — Intent/project commands and per-Session queue behavior
11. [Events](06-events.md) — Normalized events and response correlation
12. [Extension UI](07-extension-ui.md) — Blocking and fire-and-forget interaction requests
13. [Spoke Enrollment and Auth](08-spoke-enrollment-auth.md) — Enrollment, authentication, and revocation
14. [Persistence](09-persistence.md) — Hub/Spoke storage and reconciliation
15. [Provider Authentication](10-provider-authentication.md) — Provider credentials and model selection
16. [Security Policy](11-security-policy.md) — Personal-Hub trust and local enforcement
17. [CLI](12-cli.md) — Command-line shape
18. [Client Architecture](15-client-architecture.md) — Typed Client state, protocol actor, timeline, reconnect, and diagnostics

## Implementation

- [Intent-First Implementation Migration](IMPLEMENTATION-MIGRATION.md) — Destructive keep/gut map, blocking decisions, milestones, acceptance scenarios, and completion criteria

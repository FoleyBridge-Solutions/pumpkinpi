# PumpkinPi Design

PumpkinPi is a Rust-native multi-node, multi-project, multi-session daemon system for controlling the Pi LLM harness remotely and safely.

Pi remains the agent runtime. PumpkinPi manages hosts, projects, sessions, routing, auth, lifecycle, and transport.


## Design Specification Files

- [Overview](00-overview.md) — Terms, Topology, Core Principle, Design Constraints
- [Responsibilities](01-responsibilities.md) — Responsibilities
- [Data Model](02-data-model.md) — Data Model
- [Sessions](03-sessions.md) — Multiple Projects and Sessions
- [Protocol](04-protocol.md) — Protocol Layers, Client Multiplexing
- [Commands](05-commands.md) — Command Categories
- [Events](06-events.md) — Events, Response Correlation
- [Extension Ui](07-extension-ui.md) — Extension UI Handling
- [Node Enrollment Auth](08-node-enrollment-auth.md) — Node Enrollment and Auth, Revocation
- [Persistence](09-persistence.md) — Persistence
- [Provider Authentication](10-provider-authentication.md) — Provider Authentication
- [Security Policy](11-security-policy.md) — Security Policy
- [Cli](12-cli.md) — Command-Line Shape

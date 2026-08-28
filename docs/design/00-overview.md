# Overview

## Product Thesis

> **PumpkinPi brings the projects and LLM capabilities of all your computers into one personal Hub, while keeping execution grounded on the Spokes where the real context lives.**

The governing principle is:

> **Unified at the Hub, situated on the Spoke, directed by intent.**

PumpkinPi gives one person a coherent place to maintain project intent and have agents realize that intent inside the real local contexts where the projects live.

The Hub must not blur away location. Work is unified through one experience, but it remains situated in a specific Project on a specific Spoke, running as a specific local identity with real capabilities and consequences.

## Product Model

User-facing model:

```text
Personal Hub
  └─ Projects
      └─ Intent Chat
          └─ Source of Intent
```

Situated/internal model:

```text
Personal Hub
  ├─ Clients
  └─ Spokes
      └─ Projects
          ├─ Source of Intent
          ├─ Intent Chat
          └─ Internal Sessions / Runs
              └─ implementation, validation, independent review, evidence, recovery
```

- **Hub**: one person's point of presence across their connected computers. It provides authentication, enrollment, presence, routing, continuity, and cross-Spoke visibility.
- **Spoke**: one connected computer. It contributes local projects, files, tools, compute, credentials, and execution context to the Hub.
- **Project**: a trusted working environment on one Spoke, rooted at a local directory, defined by a Source of Intent.
- **Source of Intent**: the canonical LLM-facing project definition: purpose, goals, constraints, decisions, validation strategy, current status, and open questions. It need not be directly human-readable; Intent Chat renders appropriate explanations and summaries.
- **Intent Chat**: the primary conversational interface for a Project. Its job is to evolve the Source of Intent and report outcomes, evidence, and divergences.
- **Session / Run**: internal persistent LLM execution situated in one Project. Sessions implement work, validation, and recovery, but they are not the primary user-facing abstraction.
- **Client**: a GUI, CLI, or API consumer connected to the Hub. A Client is a view into ongoing project intent and work, not the owner of execution lifetime.

The product should remain close to the LLM where it matters: the user converses naturally with project intent. PumpkinPi should not make the user manage artificial workers, tickets, queues, or raw protocol concepts.

## Topology

```text
Clients
   ⇅
Personal PumpkinPi Hub
   ⇅ outbound persistent connections
PumpkinPi Spokes
   ⇅
Projects / Sources of Intent / Internal Sessions
```

Spokes connect outbound to the Hub, avoiding inbound NAT and firewall configuration. Clients connect to the Hub rather than directly to each Spoke.

## Personal Hub Scope

The initial Hub belongs to one person. Authentication determines whether a Client or Spoke belongs to that personal Hub; the system does not yet model users sharing selected Spokes or Projects.

Multiuser support will be designed together with multitenancy. The initial design must not claim that partial access-grant machinery constitutes a safe multiuser model.

## Intent-Driven Work

PumpkinPi's central loop is:

```text
User clarifies intent in Intent Chat
  ↓
PumpkinPi updates the Project's Source of Intent
  ↓
PumpkinPi launches/coordinates situated implementation and validation work
  ↓
An independent reviewer assesses complete Project reality against complete intent
  ├─ findings return to another implementation iteration
  └─ no findings establishes satisfaction for that revision
  ↓
PumpkinPi reports evidence, outcomes, and divergences back through Intent Chat
```

The user maintains intent. PumpkinPi realizes intent. Agents perform situated work. PumpkinPi continues implementation and independent whole-Project review for however many iterations are required until the reviewer finds no fault, or work is explicitly paused, cancelled, or blocked.

Project initialization is therefore the process of assembling the initial Source of Intent, not simply choosing a directory and opening a blank chat.

## Current Session Implementation

The current implementation uses Pi to execute internal Sessions / Runs. Each active Session owns one Pi subprocess launched by its Spoke:

```bash
pi --mode rpc
```

Pi is an implementation choice, not a user-facing layer in the product model. Users interact with Projects, Intent Chat, Sources of Intent, providers, models, outcomes, and evidence; they do not select or manage the underlying engine. PumpkinPi is expected to support one Session implementation at a time.

The design should avoid needless Pi-specific concepts in normal product surfaces, while also avoiding speculative plugin or adapter architecture. If the implementation moves away from Pi, it will be migrated deliberately; that future possibility does not justify abstractions users do not need today.

Pi RPC speaks strict JSONL over stdin/stdout. The Spoke writes Pi RPC commands to stdin and reads events and responses from stdout.

PumpkinPi Session identity is distinct from Pi's internal identity. The Spoke tracks both:

```text
PumpkinPi session_id -> Pi process -> Pi sessionId/sessionFile/leafId/sessionName
```

Commands that can change Pi's internal Session binding (`new_session`, `switch_session`, `fork`, `clone`) must be denied unless handled by explicit PumpkinPi operations that update the Spoke Session registry atomically.

## Trust and Execution

Installing a Spoke means trusting the personal Hub to administer PumpkinPi capabilities on that computer.

A Spoke daemon may run as root so it can manage Projects owned by different local users. Each internal Session still has an explicit execution identity. By default, the Spoke launches Pi as the configured Project or Session user. Root Sessions require explicit local policy and explicit selection.

The user should always be able to understand where work runs, which Project it inhabits, which Source of Intent it is serving, and which local identity it uses.

## Design Constraints

- Rust implementation.
- No JavaScript, Python, or other scripting-language core components.
- One personal Hub with many Spokes.
- Spokes connect outbound to the Hub.
- Hub-issued setup keys are required for Spoke enrollment.
- Many Projects per Spoke.
- Each Project has one primary Intent Chat and one canonical Source of Intent.
- Many internal Sessions / Runs may exist per Project, but they are subordinate to Intent Chat and Source of Intent.
- Many Clients may observe and control a Project's Intent Chat and visible work state.
- A Client may work across all connected Spokes simultaneously.
- Client disconnect does not kill internal Sessions by default.
- Commands are serialized per internal Session, while Sessions run in parallel.
- The primary Client is a GUI.
- Normal product surfaces use PumpkinPi concepts, not raw transport or Pi JSON.
- Provider and model choices are visible; the underlying Session implementation is not a user choice.
- Reconnect, replay, diagnostics, crash recovery, and explicit execution context are core requirements.

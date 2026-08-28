# Responsibilities

## Product Boundary

The user owns intent and decisions through a Project's Intent Chat. PumpkinPi owns the machinery that represents that intent, decomposes it into work, executes and validates work, and reports outcomes.

Internal Sessions, runs, queues, commands, events, and agent topology are not normal user space.

### Hub Responsibilities

The personal Hub is the owner's unified point of presence and the system's control plane.

It owns:

- owner and Client authentication
- Spoke authentication and enrollment
- Spoke presence
- Client connections
- Spoke/Client routing
- Project and Intent Chat continuity/recent-work metadata
- cached human-facing Project status
- audit log
- global preferences and cached metadata

It does not execute internal Sessions and should not need access to Project files. It may retain encrypted/cached Source of Intent state or projections according to explicit persistence policy, but the authoritative location and availability semantics must be clear.

### Spoke Responsibilities

The Spoke daemon runs on a machine where Project reality exists.

It owns:

- local Project registry
- authoritative Source of Intent storage or durable synchronization endpoint
- local trust policy
- Project-context inspection during initialization
- Pi RPC process lifecycle
- internal Session / Run lifecycle
- orchestration and per-Session command queues
- implementation and validation execution
- evidence collection
- event fanout
- Project/Session metadata snapshots
- local persistence
- outbound Hub connection

The Spoke is the source of truth for local filesystem/Project access and observed execution reality.

### Intent Agent Responsibilities

The logical Intent Agent behind Intent Chat owns:

- interpreting user messages as proposed intent, questions, decisions, or requests
- asking clarifying questions
- reading and updating the Source of Intent
- generating human-readable projections of the Source of Intent
- deciding when intent is actionable
- requesting or coordinating internal implementation/validation work
- coordinating independent whole-Project review after each increment
- turning every reviewer finding into durable divergence and another realization iteration
- reconciling evidence and outcomes with intent
- surfacing divergence, ambiguity, blockers, and consequential choices
- declaring satisfaction only when current complete intent receives reviewer approval with no findings

This role may be implemented using one or more internal Sessions. Its logical identity and continuity belong to the Project, not to a disposable process.

### Client Responsibilities

Clients:

- authenticate to the personal Hub
- list and select Projects across Spokes
- present one primary Intent Chat per Project
- send user intent and decisions
- render human-readable summaries, questions, outcomes, evidence, and status
- display situated context and safety cues
- answer blocking interaction requests
- reconnect and replay Project/Intent Chat history
- expose internal runs, raw events, and diagnostics only as secondary detail

A Client is not bound to one Spoke or Project. A single Client connection multiplexes all Projects:

```text
Client
  ├─ home-laptop / app / Intent Chat
  ├─ home-laptop / dotfiles / Intent Chat
  ├─ work-desktop / backend / Intent Chat
  └─ lab-server / experiments / Intent Chat
```

Every command targeting work includes enough routing information, normally `spoke_id` and `project_id`; internal execution commands additionally carry `session_id` or `run_id`.

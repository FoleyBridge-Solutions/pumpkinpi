# Overview

## Terms

- **Hub**: central server that authenticates users/nodes and routes traffic.
- **Node**: a host/machine where projects are being worked on. Runs the PumpkinPi node daemon.
- **Project**: a trusted working directory on a node.
- **Session**: one Pi RPC agent session/process associated with a project.
- **Client**: UI, CLI, desktop app, mobile app, or API consumer connected to the hub.

## Topology

```text
Clients
   ⇅
PumpkinPi Hub
   ⇅ outbound persistent connections
PumpkinPi Nodes
   ⇅
Projects / Pi RPC sessions
```

Example:

```text
             ┌──────────────┐
             │     Hub      │
             │ central srv  │
             └──────┬───────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
    ┌───▼───┐   ┌───▼───┐   ┌───▼───┐
    │ Node  │   │ Node  │   │ Node  │
    │ home  │   │ work  │   │ lab   │
    └───┬───┘   └───┬───┘   └───┬───┘
        │           │           │
   projects     projects     projects
   sessions     sessions     sessions
```

Nodes connect outbound to the hub, avoiding NAT/firewall issues. Clients connect to the hub, not directly to nodes.

## Core Principle

PumpkinPi should not reimplement Pi.

Each active PumpkinPi session owns one Pi subprocess launched by the Node:

```bash
pi --mode rpc
```

Node daemons may run as root, e.g. from a root-owned service, so they can manage projects owned by different local users. A Pi session, however, should have an explicit execution identity. By default, the Node should launch each Pi subprocess as the configured project/session user rather than root. Root-run Pi sessions are allowed only when explicitly requested and authorized by local Node policy.

Installing a PumpkinPi Node still means trusting that computer as a fully-administered execution host. Even when Pi runs as an unprivileged user, the Node daemon may have enough privilege to start, stop, inspect, or reconfigure sessions.

Pi RPC speaks strict JSONL over stdin/stdout. PumpkinPi writes Pi RPC commands to stdin and reads Pi RPC events/responses from stdout.

PumpkinPi's session identity is distinct from Pi's internal session identity. The node must track both:

```text
PumpkinPi session_id -> Pi process -> Pi sessionId/sessionFile/leafId/sessionName
```

Commands that can change Pi's internal session binding (`new_session`, `switch_session`, `fork`, `clone`) must be denied unless handled by explicit PumpkinPi wrappers that update the node session registry atomically.

## Design Constraints

- Rust implementation.
- No JS/Python/scripting language core components.
- Nodes connect outbound to Hub.
- Hub-generated setup key required for node enrollment.
- Many nodes per Hub.
- Many projects per node.
- Many sessions per project.
- Many clients per session.
- Client disconnect does not kill sessions by default.
- Per-session command serialization.
- Client is a GUI.
- Parallel sessions across projects/nodes.
- Pi RPC remains the agent runtime boundary.
- Pi subprocess execution identity is explicit per project/session: default unprivileged user, optional root only by policy.

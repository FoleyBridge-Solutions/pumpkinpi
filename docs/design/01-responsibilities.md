# Responsibilities

### Hub Responsibilities

The Hub is the control plane and message router.

It owns:

- user authentication
- node authentication
- node enrollment
- node presence
- client connections
- node/client routing
- node ownership/access
- audit log
- global metadata

It should not run Pi and should not need access to project files.

### Node Responsibilities

The Node daemon runs on a machine where code exists.

It owns:

- local project registry
- local trust policy
- Pi RPC process lifecycle
- session lifecycle
- command queues
- event fanout
- project/session metadata snapshots
- local persistence
- outbound hub connection

The node is the source of truth for local filesystem/project access.

### Client Responsibilities

Clients:

- authenticate to the hub
- list accessible nodes/projects/sessions
- create or attach to sessions
- send session commands
- subscribe to events
- answer extension UI prompts
- display streamed output/tool events/status
- connect to all authorized nodes through the hub at the same time
- work on projects and sessions across multiple nodes simultaneously

A client is not bound to one node, one project, or one session. A single client connection should multiplex work across the whole authorized graph:

```text
Client
  ├─ Node: home-laptop
  │   ├─ Project: app
  │   │   ├─ Session: fix-tests
  │   │   └─ Session: refactor-api
  │   └─ Project: dotfiles
  │       └─ Session: update-config
  ├─ Node: work-desktop
  │   └─ Project: backend
  │       └─ Session: investigate-prod-bug
  └─ Node: lab-server
      └─ Project: experiments
          └─ Session: benchmark-models
```

The hub should treat a client connection as a multiplexed control/data channel. Every client command that targets work must include enough routing information, usually `node_id`, `project_id`, and `session_id`.


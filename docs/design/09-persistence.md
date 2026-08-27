# Persistence

### Node Storage

```text
/root/.pumpkinpi-node/
  config.toml
  node.key
  projects.json
  sessions.json
  logs/
```

Sensitive files should be `0600` where supported.

Node storage is root-owned when the Node daemon runs as root.

Node stores:

- node id
- hub URL
- private key or node credential
- project registry
- session metadata
- last known Pi session file paths

### Hub Storage

Hub database stores:

```text
users/accounts
nodes
node_enrollment_keys
node_public_keys / node_tokens
node access grants
user provider accounts / encrypted credentials
user provider usage/preferences metadata
project provider/model defaults
project metadata snapshots
session metadata snapshots
audit log
```

Hub should hash setup keys and bearer tokens if bearer tokens are used.

Hub metadata snapshots are caches, not source of truth. Node inventory messages should include monotonically increasing revision numbers or timestamps so the Hub can reconcile stale projects/sessions after reconnects, deletions, renames, crashes, and offline periods.

## Reconciliation Rules

On node reconnect or inventory refresh, the Node is authoritative for projects and sessions on that node. The Hub reconciles cached metadata using node inventory revision numbers and per-object `updated_at` timestamps.

Explicit cases:

- **Node offline edits**: when a node reconnects, the Hub marks cached objects stale until fresh inventory arrives. Newer Node inventory wins over Hub cache for node-owned fields.
- **Session deleted locally but cached by Hub**: if a session is absent from a complete inventory snapshot, the Hub marks its cached snapshot `stale` or `deleted_remote` and removes active subscriptions after notifying clients.
- **Project path renamed/moved**: project identity is `project_id`, not `cwd`. If the Node reports the same `project_id` with a new `cwd`, the Hub updates the snapshot and emits `project.updated`. If the old path no longer exists and no replacement is reported, mark the project `missing`.
- **Pi session file missing**: the Node marks the PumpkinPi session `missing`. The session remains listed for recovery/delete, but normal commands are rejected until the Pi file is restored or a wrapper intentionally creates a new Pi session binding.
- **Duplicate project/session names**: names are display labels only. IDs are authoritative. The Hub and clients must allow duplicate names and disambiguate by node/project/session ids and cwd where useful.
- **Stale client subscriptions**: when a project/session becomes stale, deleted, revoked, or missing, the Hub sends a terminal subscription event and removes or suspends the subscription. Clients must not silently retarget a subscription by name.

A partial inventory must be marked as partial and must not delete missing cached objects. Only a complete inventory snapshot may cause Hub cache objects to become stale/deleted due to absence.


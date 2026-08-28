# Persistence

### Spoke Storage

```text
/root/.pumpkinpi-spoke/
  config.toml
  spoke.key
  projects.json
  sources-of-intent/
  intent-chats/
  sessions.json
  evidence/
  logs/
```

Sensitive files should be `0600` where supported.

Spoke storage is root-owned when the Spoke daemon runs as root.

Spoke stores:

- spoke id
- hub URL
- private key or spoke credential
- Project registry and initialization state
- canonical Source of Intent payloads, revisions, and hashes
- durable Intent Chat timeline/cursors
- internal Session/Run metadata and intent revision bindings
- evidence and promoted outcomes
- last known Pi session file paths

### Hub Storage

The personal Hub database stores:

```text
owner identity and recovery metadata optional
client credentials
spokes
spoke_enrollment_keys
spoke_public_keys / spoke_tokens
provider accounts / encrypted credentials
provider usage/preferences metadata
project provider/model defaults
Project metadata and initialization snapshots
Source of Intent availability/revision metadata and permitted encrypted cache
Intent Chat projections/timeline cache
internal Session/Run metadata snapshots
recent Projects/work and Client preferences
audit log
```

The initial schema does not include sharing grants or tenant membership. Multiuser support will be designed together with multitenancy.

Hub should hash setup keys and bearer tokens if bearer tokens are used.

Hub metadata snapshots are caches, not source of truth. Spoke inventory messages should include monotonically increasing revision numbers or timestamps so the Hub can reconcile stale projects/sessions after reconnects, deletions, renames, crashes, and offline periods.

## Reconciliation Rules

On Spoke reconnect or inventory refresh, the Spoke is authoritative for local Projects, canonical Source of Intent state, evidence, and internal Sessions on that Spoke unless a future explicit replication design says otherwise. The Hub reconciles caches using inventory revisions, Source of Intent revisions/hashes, timeline cursors, and per-object timestamps.

Explicit cases:

- **Spoke offline edits**: when a spoke reconnects, the Hub marks cached objects stale until fresh inventory arrives. Newer Spoke inventory wins over Hub cache for spoke-owned fields.
- **Session deleted locally but cached by Hub**: if a session is absent from a complete inventory snapshot, the Hub marks its cached snapshot `stale` or `deleted_remote` and removes active subscriptions after notifying clients.
- **Project path renamed/moved**: project identity is `project_id`, not `cwd`. If the Spoke reports the same `project_id` with a new `cwd`, the Hub updates the snapshot and emits `project.updated`. If the old path no longer exists and no replacement is reported, mark the project `missing`.
- **Pi session file missing**: the Spoke marks the PumpkinPi session `missing`. The session remains listed for recovery/delete, but normal commands are rejected until the Pi file is restored or a wrapper intentionally creates a new Pi session binding.
- **Source of Intent mismatch**: revision/hash disagreement freezes conflicting writes. PumpkinPi preserves both states, marks intent `conflicted`, and uses Intent Chat to explain/recover rather than silently choosing by timestamp.
- **Source of Intent unavailable/corrupt**: ordinary intent-driven work is paused. Preserve last known human-readable projections and evidence, then offer repair/export diagnostics.
- **Duplicate Project/Session names**: names are display labels only. IDs are authoritative. Clients disambiguate with Spoke and cwd where useful.
- **Stale client subscriptions**: when a project/session becomes stale, deleted, revoked, or missing, the Hub sends a terminal subscription event and removes or suspends the subscription. Clients must not silently retarget a subscription by name.

A partial inventory must be marked as partial and must not delete missing cached objects. Only a complete inventory snapshot may cause Hub cache objects to become stale/deleted due to absence.


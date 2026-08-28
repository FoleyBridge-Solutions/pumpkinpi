# Persistence and Reconciliation

## Slice Authority

Each Slice uses SQLite in WAL mode as authoritative structured storage. It persists Slice identity/binding, Projects/trust/policy, Source revisions and bundles, Intent Chats/timelines, operations/objectives, Sessions/Runs/events, tool calls, interactions, evidence/artifacts, divergences/reviews, workspaces, provider account references, inventory revision, audit, and recovery state.

Large immutable bytes such as authoritative bundles, complete command output, file chunks, and exports may use a content-addressed artifact directory referenced transactionally from SQLite. Artifacts are written/hash-verified before committed references; orphan cleanup is recoverable.

No Redis, Hub connection, Node runtime, or external agent session store is required.

## Hub Authority

Hub SQLite persists owner-control credentials used by `slice gui`, Slice enrollment/public keys/status, encrypted provider accounts and key metadata, routing/subscription metadata, redacted audit, and explicitly stale-able caches of Slice inventories/snapshots. Hub cache never becomes Source/Project/execution authority.

## Slice UI State

Standalone TUI and `slice gui` persist separate non-authoritative preferences. GUI state includes owner-control credential reference, Hub URL, selection, drafts, cursors, recent remote Projects, subscription intent, and diagnostics. TUI state includes local selection, drafts, recent local Projects/Sessions, and display preferences. Neither replaces serve/TUI Project authority. Secrets use protected references.

## Transactions

Authority transitions are SQLite transactions with explicit expected state/revision. Filesystem transitions use prepared records and idempotent reconciliation:

- Source commit retains previous revision/artifact before current pointer advances;
- accepted user message and operation persist before acknowledgement;
- tool intent persists before consequential execution and result afterward;
- checkpoint commit records Git identity before review phase advances;
- approval and promotion use a prepared/promoting/promoted protocol recoverable across crash;
- cursor and inventory revisions are monotonic counters.

Atomic rename without fsync/schema/recovery is insufficient for authority.

## Schema Migration

Every database has schema/application version. Migrations are ordered Rust code, transactional where SQLite permits, backed up when destructive, idempotence-tested, and fault-injected. Unsupported future versions fail safely. Corruption opens diagnostics/recovery without silently creating empty authority.

The prerelease PumpkinPie/Slice rename performs a one-time migration from legacy paths/fields. It does not retain a permanent dual API or dual writer.

## Runtime Event Log

Session events are append-only per Session sequence. Context checkpoints and projections are derived. Crash recovery uses durable event/phase boundaries, not a provider socket or external session file. Hidden chain-of-thought is not stored.

Review evidence may journal append-only during active capture, then reconcile transactionally into evidence/artifact tables. Full-store rewrites per tool event are prohibited.

## Reconciliation

Slice inventory has a strictly monotonic revision and `complete` flag. Hub:

- rejects stale/out-of-order inventory;
- merges partial inventory without deleting omitted objects;
- reconciles absence only from complete inventory;
- marks cached state stale/offline on disconnect;
- refreshes all snapshot fields without lossy merges.

Slice GUI instances replay by durable cursor, deduplicate stable IDs, and receive explicit gaps when retained history cannot satisfy a cursor.

## Retention

Policy separately bounds diagnostics, provider raw fragments, command artifacts, caches, superseded worktrees, archived Sessions, and audit. Canonical Source revisions, owner messages/decisions, promotion/approval evidence, and unresolved recovery records are not deleted by ordinary cache cleanup.

Build caches are disposable, content-addressed by complete validity identity, size-bounded, and never authority. Terminal workspace/branch/cache cleanup is recorded and retryable.

## Failure Cases

- missing/corrupt database or artifact: block affected authority, preserve diagnostics/backups, offer verify/export/repair;
- Source mismatch/conflict: freeze canonical writes/realization and preserve both sides;
- missing/corrupt worktree: block with checkpoint/base diagnostics;
- interrupted promotion: reconcile prepared Git and database states idempotently;
- missing context checkpoint: rebuild from Session events or mark blocked, never invent history;
- Hub cache mismatch: defer to authenticated Slice inventory and retain visible stale/conflict state;
- disk full/fsync failure: do not acknowledge authority transition.

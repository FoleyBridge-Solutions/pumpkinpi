# Internal Sessions and Runs

Sessions and Runs are PumpkinPi execution machinery. They are documented because they are required for implementation, reliability, safety, and diagnostics; they are not the primary product object.

The user normally interacts only with a Project's Intent Chat. PumpkinPi creates, resumes, coordinates, and retires internal Sessions to maintain the Source of Intent and make Project reality conform to it.

## Multiple Projects and Internal Sessions

A Spoke can host many Projects, and each Project can have concurrent internal work:

```text
Spoke
  ├─ Project: /home/me/app
  │   ├─ Intent Chat / intent-maintenance Session
  │   ├─ implementation Run
  │   └─ validation Run
  └─ Project: /home/me/website
      ├─ Intent Chat / intent-maintenance Session
      └─ inspection Run
```

Each active Session has its own Pi RPC process. Commands are serialized per Session, not globally, while independent Sessions may run in parallel.

Client disconnect must not kill work by default. Multiple Clients may observe the same Project/Intent Chat, but they should not need to attach manually to each internal Session.

## Session Purpose and Intent Binding

Every internal Session/Run must have a declared purpose:

- `intent`: interpret conversation, update Source of Intent, and project it back to users
- `inspection`: gather context/evidence without primarily changing Project state
- `implementation`: change Project reality toward a Source of Intent revision
- `validation`: test claims and behavior produced by implementation
- `review`: independently inspect complete Project reality against a complete Source of Intent revision, returning every finding or approval with no findings
- `recovery`: diagnose or repair failed internal work

Implementation and validation Runs execute in a per-operation isolated Git worktree, never directly in the primary Project checkout. Each successful implementation iteration creates a durable checkpoint commit. Independent review examines that checkpoint; zero-finding approval promotes it to the primary checkout with an automatic fast-forward transaction. Primary-checkout drift or a non-fast-forward blocks promotion rather than overwriting work. No per-iteration permission gate is required.

Implementation, validation, and review Runs record the Source of Intent revision they serve. If intent changes materially while a Run is active, PumpkinPi must decide whether to continue, cancel, or mark its output as based on stale intent. It must not silently present stale work as satisfying current intent.

After each implementation/validation increment, an independent review Run evaluates the whole Project against the whole current Source of Intent. Every finding feeds another bounded implementation iteration. Project realization is satisfied only when review returns no findings and no required scope remains unreviewed. Iteration or resource limits pause work; they never imply success.

## Reporting Back to Intent Chat

Raw Session output remains internal detail by default. PumpkinPi should promote information to Intent Chat when it is:

- a clarification or decision needed from the user
- a meaningful progress/status transition
- a consequential safety prompt
- an outcome and its evidence
- a divergence between intent and reality
- a failure requiring user-visible recovery

## Pi Process Execution Identity

The Spoke daemon may run as root, but each Pi subprocess has explicit `run_as_user` / `run_as_root` settings.

Default behavior:

- Project Sessions run as the Project owner or configured `run_as_user`
- root Sessions are denied unless `allow_root_sessions` is true, the operation explicitly requires root, and local Spoke policy allows it
- effective user is recorded in Session metadata, evidence, and audit logs
- provider credentials delivered to Pi are accessible to that effective user and privileged Spoke daemon

## Process Death and Recovery

If a Pi subprocess exits unexpectedly, the Spoke must:

1. mark the internal Session `crashed`
2. record exit status/signal, stderr tail, timestamp, purpose, affected Source of Intent revision, and last known Pi metadata
3. broadcast normalized crash state to orchestration/subscribers
4. update Intent Chat with a concise explanation when the crash affects visible work
5. reject ordinary commands while crashed except lifecycle/diagnostic commands

Restart behavior is explicit and durable:

- uncommitted isolated changes are discarded to the last checkpoint
- persisted realization phase, iteration, findings, operation, and workspace binding are recovered
- active realization is queued and resumes automatically after Spoke authentication
- restart creates a new Pi process while preserving PumpkinPi operation identity
- already checkpointed changes are inspected from current isolated reality rather than blindly repeated in primary
- missing or corrupt worktrees block with retained diagnostics instead of recreating uncertain destructive state

Late observers should receive Project/Intent Chat state and promoted outcomes first. Detailed diagnostics may additionally replay durable internal Session entries.

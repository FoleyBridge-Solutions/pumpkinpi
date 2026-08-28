# Security Policy

PumpkinPie is initially a personal single-owner system, not partial multitenancy. Administrative Hub access controls enrolled capabilities, while Slice enforces situated filesystem/execution policy locally.

## Boundaries

- Hub owns `slice gui` owner-control auth, Slice enrollment, provider custody, routing, cache, and audit.
- Slice owns Project paths/trust, native runtime, tools, identity, evidence, and local authority.
- Provider clients have network/credential access but no direct authority transition.
- Project tool processes receive only declared mounts/environment/capabilities and no provider/Hub credentials.
- Slice GUI instances display/submit typed commands but are not canonical or execution authority.

## Project Containment

Canonicalize configured trusted roots and Project/repository/worktree paths. Treat symlinks, hard links, mount changes, filesystem identity, and path replacement explicitly. Read/write tools use containment-safe resolution and expected versions. Authoritative Source documents are protected from implementation mutation.

## Sandbox and Identity

Every tool execution records effective uid/gid/root, writable mounts, cwd, network policy, environment fingerprint, command/tool version, and Run. Capabilities are dropped by default. Root requires explicit Project setting, operation need, local policy, and visible context. Read-only roles cannot mutate Project reality or shared host surfaces.

Provider networking and credentials remain in Slice. Tool network access is denied or explicitly policy-scoped independently.

## Native Runtime Safety

Provider/model outputs are untrusted. Tool name/schema/policy, path/command limits, revision/context, interaction authorization, and output contracts are validated in Rust. Unknown fields/types at authority boundaries fail closed. Context summaries and model claims are not evidence.

## Secrets and Audit

Secrets are encrypted/referenced securely, delivered minimally, redacted before logs/events/artifacts, and never argv where avoidable. Hub administrative actions and consequential Slice operations produce redacted structured audit records. Store files/databases/keys have restrictive permissions.

## Limits and Recovery

Enforce per-Project/Slice/provider concurrency, tool budgets/timeouts/output limits, storage/cache retention, cancellation deadlines, and rate controls. Limits pause/fail visibly and never imply satisfaction. Crash/corruption/conflict preserves evidence and blocks uncertain authority.

## Rust-Only Supply Boundary

PumpkinPie ships no Node.js, JavaScript/TypeScript/Python application or test sidecar, external coding-agent runtime, or provider CLI dependency. Rust dependencies and external system tools remain versioned/audited. Release tests run without Node or the removed legacy agent installed.

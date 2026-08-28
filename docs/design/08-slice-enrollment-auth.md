# Slice Enrollment and Authentication

A `slice serve` endpoint has local identity before enrollment. Enrollment authorizes that identity to connect to one personal PumpkinPie Hub; it does not create Slice execution authority or transfer local state.

## Local Identity

On first `slice serve` initialization Slice creates:

- stable local `slice_id` using the `slice_` prefix;
- display name/hostname metadata;
- Ed25519 signing keypair;
- local SQLite authority and policy version.

The private key remains in secure local storage with mode/platform protection. GUI-only use does not create or reuse this endpoint identity. Resetting serve identity is explicit and does not silently orphan registered Projects/Sessions.

## Enrollment

The owner creates a pending binding on the Hub:

```bash
pumpkinpie-hub slice create --name framework-laptop
```

The Hub returns a one-use short-lived setup key. On the Slice:

```bash
slice enroll --hub https://hub.example.com --setup-key pps_setup_...
```

Slice sends its existing `slice_id`, public key, name, hostname, version, protocol version, and capabilities. The Hub validates setup scope/expiry/use, ensures identity is not bound inconsistently, stores the public key, and returns Hub identity/binding metadata. Local Projects remain local and inventory follows only after authenticated connection.

Setup keys are Hub-generated, one-Slice scoped, one-time, short-lived, revocable, hashed at rest, redacted, and never ongoing credentials.

## Connection Authentication

Slice establishes an outbound TLS WebSocket and sends hello with identity/version/capabilities. Hub returns a random challenge bound to connection, Hub identity, protocol, and expiry. Slice signs the transcript; Hub verifies stored public key and current status. Authentication returns heartbeat policy, inventory baseline, and capability agreement.

Challenges cannot be replayed and expire quickly. Plain HTTP/WebSocket is limited to explicit loopback development policy.

## Inventory

After authentication Slice sends a strictly monotonic revisioned complete inventory containing Project and Source/Intent availability plus recovery-relevant internal Session summaries. Partial updates identify themselves and never imply deletion by omission.

Hub caches inventory; Slice remains authority.

## Disable, Revoke, Rotation, Removal

```bash
pumpkinpie-hub slice disable slice_...
pumpkinpie-hub slice revoke slice_...
pumpkinpie-hub slice rotate-key slice_...
pumpkinpie-hub slice issue-setup-key slice_...
```

Disable rejects new routed work and disconnects according to policy while preserving binding. Revoke terminates connections and rejects future authentication. Rotation proves possession of current key or uses an explicit owner-mediated recovery flow; it is atomic and audited. Hub removal does not erase local Slice state.

Slice can unenroll locally, removing Hub binding/cached capability while preserving standalone Projects/Sessions. Re-enrollment is explicit.

## Slice GUI Owner Authentication

`slice gui` uses independently revocable owner-control credentials rather than endpoint identity or one permanently shared token. Authentication, rotation, revocation, secure storage references, session lifetime, and audit are Hub responsibilities. A GUI credential never authenticates a `slice serve` endpoint or local IPC session.

## Recovery and Security

Enrollment/key failures surface expiry, revoked/disabled state, clock/challenge issues, protocol mismatch, and next owner action without exposing keys. Keys, setup material, provider capabilities, and signatures are redacted from ordinary logs and evidence.

# Spoke Enrollment Auth

## Spoke Enrollment and Auth

Spoke auth must be hub-issued. A spoke cannot self-register and declare identity.

### Enrollment Flow

The owner creates a Spoke on their Hub:

```bash
pumpkinpi hub spoke create --name framework-laptop
```

Hub generates:

- `spoke_id`
- one-time setup/enrollment key

Example output:

```text
spoke_id: spoke_7f3a...
setup_key: pps_setup_...

Run on spoke:
pumpkinpi spoke enroll --hub https://hub.example.com --setup-key pps_setup_...
```

Spoke enrolls:

```bash
pumpkinpi spoke enroll \
  --hub https://hub.example.com \
  --setup-key pps_setup_...
```

Spoke sends:

```json
{
  "type": "spoke.enroll",
  "setup_key": "pps_setup_...",
  "hostname": "framework",
  "version": "0.1.0",
  "public_key": "..."
}
```

Hub validates the setup key and binds the spoke identity.

Hub returns:

```json
{
  "type": "spoke.enrolled",
  "spoke_id": "spoke_7f3a",
  "hub_url": "https://hub.example.com"
}
```

Spoke stores local config and credentials.

### Setup Key Requirements

Setup keys should be:

- generated only by Hub
- scoped to one spoke
- one-time use
- short lived, e.g. 10–30 minutes
- revocable
- stored hashed server-side
- never reused for ongoing authentication

### Ongoing Spoke Authentication

Preferred model: asymmetric key auth.

During enrollment:

- spoke generates local private/public keypair
- private key stays on spoke
- public key is sent to hub

During normal connection:

1. Spoke connects to Hub.
2. Spoke sends `spoke.hello`.
3. Hub sends challenge nonce.
4. Spoke signs nonce with private key.
5. Hub verifies signature using stored public key.
6. Hub marks spoke online.

Handshake:

```json
{
  "type": "spoke.hello",
  "spoke_id": "spoke_7f3a",
  "version": "0.1.0"
}
```

```json
{
  "type": "spoke.challenge",
  "nonce": "random_bytes_base64",
  "expires_at": "..."
}
```

```json
{
  "type": "spoke.auth",
  "spoke_id": "spoke_7f3a",
  "signature": "base64_signature"
}
```

```json
{
  "type": "spoke.authenticated",
  "heartbeat_interval_ms": 30000
}
```

After auth, the Spoke sends inventory. Primary inventory describes Projects and Source of Intent/Intent Chat availability; internal Session inventory supports orchestration and recovery:

```json
{
  "type": "spoke.inventory",
  "projects": [],
  "intent_state": [],
  "internal_sessions": []
}
```

Bearer tokens may be acceptable for local development. Remote hub deployments should use challenge-response auth.

## Revocation

Hub must support:

```bash
pumpkinpi hub spoke revoke spoke_7f3a
pumpkinpi hub spoke disable spoke_7f3a
pumpkinpi hub spoke rotate-key spoke_7f3a
pumpkinpi hub spoke issue-setup-key spoke_7f3a
```

On revocation:

- existing spoke connection is disconnected
- future spoke auth fails
- clients can no longer access the spoke


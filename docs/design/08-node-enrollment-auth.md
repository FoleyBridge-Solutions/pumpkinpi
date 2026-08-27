# Node Enrollment Auth

## Node Enrollment and Auth

Node auth must be hub-issued. A node cannot self-register and declare identity.

### Enrollment Flow

Admin creates a node on Hub:

```bash
pumpkinpi hub node create --name framework-laptop
```

Hub generates:

- `node_id`
- one-time setup/enrollment key

Example output:

```text
node_id: node_7f3a...
setup_key: ppn_setup_...

Run on node:
pumpkinpi node enroll --hub https://hub.example.com --setup-key ppn_setup_...
```

Node enrolls:

```bash
pumpkinpi node enroll \
  --hub https://hub.example.com \
  --setup-key ppn_setup_...
```

Node sends:

```json
{
  "type": "node.enroll",
  "setup_key": "ppn_setup_...",
  "hostname": "framework",
  "version": "0.1.0",
  "public_key": "..."
}
```

Hub validates the setup key and binds the node identity.

Hub returns:

```json
{
  "type": "node.enrolled",
  "node_id": "node_7f3a",
  "hub_url": "https://hub.example.com"
}
```

Node stores local config and credentials.

### Setup Key Requirements

Setup keys should be:

- generated only by Hub
- scoped to one node
- one-time use
- short lived, e.g. 10–30 minutes
- revocable
- stored hashed server-side
- never reused as runtime auth

### Runtime Node Auth

Preferred model: asymmetric key auth.

During enrollment:

- node generates local private/public keypair
- private key stays on node
- public key is sent to hub

During normal connection:

1. Node connects to Hub.
2. Node sends `node.hello`.
3. Hub sends challenge nonce.
4. Node signs nonce with private key.
5. Hub verifies signature using stored public key.
6. Hub marks node online.

Handshake:

```json
{
  "type": "node.hello",
  "node_id": "node_7f3a",
  "version": "0.1.0"
}
```

```json
{
  "type": "node.challenge",
  "nonce": "random_bytes_base64",
  "expires_at": "..."
}
```

```json
{
  "type": "node.auth",
  "node_id": "node_7f3a",
  "signature": "base64_signature"
}
```

```json
{
  "type": "node.authenticated",
  "heartbeat_interval_ms": 30000
}
```

After auth, node sends inventory:

```json
{
  "type": "node.inventory",
  "projects": [],
  "sessions": []
}
```

Bearer tokens may be acceptable for local development. Remote hub deployments should use challenge-response auth.

## Revocation

Hub must support:

```bash
pumpkinpi hub node revoke node_7f3a
pumpkinpi hub node disable node_7f3a
pumpkinpi hub node rotate-key node_7f3a
pumpkinpi hub node issue-setup-key node_7f3a
```

On revocation:

- existing node connection is disconnected
- future node auth fails
- clients can no longer access the node


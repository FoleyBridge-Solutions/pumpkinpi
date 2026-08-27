# Provider Authentication

Provider login happens once through the Client. Users should not have to log in to every Node separately.

The Client owns the login UX: OAuth browser flows, subscription login prompts, API-key entry, account selection, and provider connection management. The Hub persists the resulting provider account information for the user so it can be reused across Nodes and projects.

Pi supports provider credentials through:

- OAuth/subscription login via Pi interactive `/login`
- API keys stored in Pi `auth.json`
- provider environment variables
- custom provider config in `models.json`

Pi stores credentials by default in:

```text
~/.pi/agent/auth.json
~/.pi/agent/models-store.json
~/.pi/agent/models.json
```

Pi credential resolution order is:

1. CLI/runtime API key override
2. `auth.json`
3. environment variables
4. custom provider keys from `models.json`

PumpkinPi should make that Pi credential model invisible to end users where possible: the user logs in once in the Client, selects the provider/model in the Client, and PumpkinPi arranges for the Node-launched Pi process to use the selected provider account.

### Client-Initiated Provider Login Model

```text
Client
  → user logs in to provider once
Hub
  → stores provider account/credential material and preferences for the user
Project
  → stores default provider/model selection metadata
Node
  → receives the provider account material needed to launch/run Pi
Pi
  → uses that provider account for the session
```

The Hub should store provider accounts securely and remember which providers/models each user uses, which providers are available/preferred for each project, recent provider/model choices, and UI selection defaults.

Operational rule: if a user is authorized to access a PumpkinPi Node through the Hub, they have full administrative control over that Node's PumpkinPi capabilities. Node-launched Pi usually runs as the configured project/session user, with root sessions allowed only by policy, but any provider material delivered to the Node for execution must still be considered accessible to that Node, its privileged daemon, and the Pi subprocess effective user.

### Provider and Model Selection

Clients may request providers and models at project/session creation or during a session. Projects may also define default provider/model settings stored by the Hub as metadata:

```json
{
  "type": "session.create",
  "node_id": "node_home",
  "project_id": "proj_api",
  "name": "fix-tests",
  "provider": "anthropic",
  "model": "claude-sonnet-4-5:high"
}
```

PumpkinPi provider/model selectors are product-level identifiers. The Hub stores and presents these choices per user and per project. The Node translates them to Pi startup/RPC parameters such as `--provider`, `--model`, `set_model { provider, modelId }`, and `set_thinking_level`, using the selected provider account supplied through PumpkinPi.

### Hub-Owned Provider Account Store

Hub stores provider credentials encrypted at rest. The credential store uses envelope encryption, KMS/HSM or operator-managed master keys, rotation, backup/restore handling, and auditability without logging secret values.

```text
provider_accounts
  provider_account_id
  user_id
  provider_id
  display_name / account_label
  auth_type: api_key | oauth | subscription | external_secret_ref
  encrypted_secret
  available_models optional
  default_model optional
  created_at
  updated_at
  revoked_at
```

### Credential Safety Rules

- Provider login should happen once in the Client, not manually on every Node.
- Do not install a PumpkinPi Node on a machine unless you trust remote PumpkinPi users to administer that node.
- Hub may store provider credentials, provider names, account labels, model preferences, availability metadata, and project defaults.
- Hub must encrypt provider secret values such as API keys, OAuth tokens, refresh tokens, and subscription credentials at rest.
- Provider secrets should never be sent back to clients after initial entry/login.
- Node diagnostics and audit logs must avoid recording provider secrets from Pi config files, environment variables, credential payloads, or command output.


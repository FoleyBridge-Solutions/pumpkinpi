# Provider Authentication

Provider login happens once through the Client. The owner should not have to log in separately on every Spoke.

The Client owns the login UX: OAuth browser flows, subscription login prompts, API-key entry, account selection, and provider connection management. The personal Hub persists the resulting provider account information so it can be reused across Spokes and Projects.

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

1. CLI/process API key override
2. `auth.json`
3. environment variables
4. custom provider keys from `models.json`

PumpkinPi should make that Pi credential model invisible to end users where possible: the user logs in once in the Client, selects the provider/model in the Client, and PumpkinPi arranges for the Spoke-launched Pi process to use the selected provider account.

### Client-Initiated Provider Login Model

```text
Client
  → owner logs in to provider once
Personal Hub
  → stores provider account/credential material and preferences
Project
  → stores default provider/model selection metadata
Spoke
  → receives the provider account material needed to launch/run Pi
Pi
  → uses that provider account for the session
```

The Hub should store provider accounts securely and remember provider/model usage, Project preferences, recent choices, and UI selection defaults.

Operational rule: an authenticated Client of the personal Hub has administrative control over every enrolled Spoke's PumpkinPi capabilities. Spoke-launched Pi usually runs as the configured Project or Session user, with root Sessions allowed only by policy, but provider material delivered to a Spoke must still be considered accessible to that Spoke, its privileged daemon, and the Pi subprocess effective user.

### Provider and Model Selection

Projects may define default provider/model settings. Users can change these through Project settings or ask through Intent Chat; they do not choose providers separately for each internal Session during normal use.

```json
{
  "type": "project.model.set",
  "spoke_id": "spoke_home",
  "project_id": "proj_api",
  "provider": "anthropic",
  "model": "claude-sonnet-4-5:high"
}
```

PumpkinPi provider/model selectors are product-level identifiers. The Hub stores and presents choices globally and per Project. Orchestration applies them to internal Sessions, and the Spoke translates them to Pi startup/RPC parameters such as `--provider`, `--model`, `set_model`, and `set_thinking_level`.

### Hub-Owned Provider Account Store

Hub stores provider credentials encrypted at rest. The credential store uses envelope encryption, KMS/HSM or operator-managed master keys, rotation, backup/restore handling, and auditability without logging secret values.

```text
provider_accounts
  provider_account_id
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

- Provider login should happen once in the Client, not manually on every Spoke.
- Do not enroll a Spoke unless every authenticated Client of the personal Hub may administer it through PumpkinPi.
- Hub may store provider credentials, provider names, account labels, model preferences, availability metadata, and project defaults.
- Hub must encrypt provider secret values such as API keys, OAuth tokens, refresh tokens, and subscription credentials at rest.
- Provider secrets should never be sent back to clients after initial entry/login.
- Spoke diagnostics and audit logs must avoid recording provider secrets from Pi config files, environment variables, credential payloads, or command output.


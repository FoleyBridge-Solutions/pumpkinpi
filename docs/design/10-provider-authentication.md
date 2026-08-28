# Provider Authentication and Models

Slice's native Rust provider layer calls provider HTTPS APIs directly. PumpkinPie does not rely on an external agent, provider CLI, Node SDK, or language sidecar.

## Account Custody

Provider accounts may be:

- Hub-managed for use across enrolled Slices;
- Slice-local for standalone use;
- external platform-secret references.

Metadata uses product-level provider/account/model IDs. Secret material is encrypted at rest with envelope-key rotation or referenced from a platform credential service. Account label/ID selection is explicit; “first matching provider” is not sufficient.

## Standalone Login

```bash
slice auth login PROVIDER
slice auth set-key PROVIDER
slice auth list
```

Secrets are entered through terminal-safe hidden prompt, browser OAuth/device flow, protected stdin descriptor, or external credential reference. API keys are never positional argv values, ordinary shell history, timeline content, or diagnostic evidence.

OAuth callback/device flow is implemented in Rust. Refresh tokens remain in provider custody storage; access tokens are refreshed by the native provider client.

## Hub Delivery

For an enrolled Run requiring a Hub account:

```text
Slice GUI selects account reference
  -> Hub authorizes Project/Slice/model/purpose
  -> Hub delivers encrypted/scoped short-lived provider capability
  -> Slice native provider client uses it
```

Provider material is delivered only for actual model execution, not list/get/subscribe/remove/cancel commands. It is excluded from Project tool environments, provider-visible prompts, events, artifacts, command output, and audit details.

Slice-local and Hub accounts do not overwrite one another silently. Project/Session records state selected account reference and fallback policy.

## Provider Support

The native runtime initially supports Anthropic, OpenAI, Google Gemini, and OpenAI-compatible/OpenRouter APIs. A capability registry records models, context/output limits, streaming, tools, structured output, reasoning controls, modalities, pricing/usage metadata, and authentication methods.

Model selection is validated against account/provider capability before Run start. Raw provider model catalogs are normalized and cached with freshness/source.

## Secret Boundary

The privileged Slice process and native provider module can access necessary account capability. Project child tools cannot. Provider requests occur outside Project sandboxes. Redaction applies before persistence/logging and handles headers, URLs/query fields, JSON payloads, environment values, tool output, panic/error chains, and exported diagnostics.

## Rotation, Revocation, and Failure

Rotation is atomic and retains recoverable envelope metadata. Revoked/expired accounts stop new Runs and produce explicit blocked state; already in-flight behavior follows provider and owner policy. Refresh failure, insufficient scope, model denial, quota/rate limit, transport/TLS, and malformed provider streams are distinct typed errors.

No provider authentication failure may fall back silently to a different account/model with materially different privacy, cost, or behavior.

## Local Development

Environment-variable credentials may be accepted only under explicit development policy and are still excluded from tool subprocesses and diagnostics. Production packaging requires no provider-specific executable or non-Rust runtime.

# Native Rust Agent Runtime

## Decision

PumpkinPie is implemented entirely in Rust. Slice owns the complete coding-agent runtime: provider clients, streaming model loop, tool dispatch, sandbox supervision, evidence capture, context management, interactions, cancellation, persistence, and recovery. Production operation must not require Node.js, JavaScript, TypeScript, Python, Pi, a language sidecar, or a hosted agent SDK.

External operating-system and Project tools such as Git, Bubblewrap, Cargo, compilers, test runners, and `rg` may be executed under explicit policy. They are supervised tools, not PumpkinPie application components. First-party application logic, daemons, clients, migrations, fixtures, fake providers, and fault injectors are Rust.

## Crate Boundary

The workspace converges on:

```text
pumpkinpie-runtime   provider clients, model loop, native tools, evidence, context
pumpkinpie-protocol  typed Hub/Slice role wire and domain contracts
slice                standalone TUI, GUI Client, CLI, serve endpoint, enrollment
pumpkinpie-hub       personal Hub, routing, cache, accounts, audit
```

`pumpkinpie-runtime` is an internal product crate, not a general plugin framework. Slice is its production owner. There is one runtime implementation when migration completes.

## Polaris Assessment and Adoption Boundary

The native-runtime design was cross-checked against [Radiant AI Labs' Polaris](https://github.com/RadiantAILabs/polaris) at commit `5ff8464` (workspace 0.6.0): its typed systems/resources, graph verification phases and signatures, scoped contexts, provider-neutral content stream, tool schemas, lifecycle hooks/middleware, Sessions, and persistence. Polaris is Apache-2.0 and can be adapted with the required attribution, but PumpkinPie does not fork or depend on the complete framework as its runtime foundation.

The useful ideas are adopted below as PumpkinPie-owned contracts:

- place each check at the earliest phase where it is sound, without false-positive rejection;
- make stage inputs/outputs and capability requirements explicit rather than hiding them in a mutable working-state object;
- isolate child contexts with an allowlist and explicit share/copy/fresh semantics;
- normalize provider streams as indexed content-block lifecycle events;
- separate tool registration, model exposure, authorization, and execution;
- expose fixed lifecycle events and exactly-once middleware for telemetry and evidence;
- version capability contracts and pin a deterministic runtime manifest;
- test primitives, composition, and integrations at their corresponding layers.

The following Polaris choices are explicitly not inherited:

- an extensible plugin/ECS product architecture;
- in-memory `SystemContext` or serialized resource snapshots as durable Session authority;
- graph output merging by type with last-writer-wins collisions;
- advisory tool permissions or permission widening inside a registry;
- process-wide value inspection;
- hard graph timeout as sufficient cancellation or recovery;
- TypeScript bindings, dashboard components, HTTP Sessions API, file Session store, or shell executor.

Polaris graphs are useful precedent for an inspectable **transient execution plan**. They are not a substitute for PumpkinPie's SQLite-backed Run state machine, tool-effect journal, evidence model, sandbox, or promotion authority. Any Polaris code selectively adapted later must pass the Rust-only, security, persistence, provider-fidelity, and maintenance gates in the migration plan; conceptual adoption does not create a compatibility obligation.

## Runtime Composition and Verification

The runtime is a fixed composition of product-owned stages, not a dynamically extensible agent graph. Each role has a versioned `ExecutionPlan` describing:

```text
ExecutionPlan
  plan_id/version
  role/purpose
  required_inputs[]
  required_capabilities[]
  produced_outputs[]
  tool_catalog_version
  context_crossings[]
  stage_budgets[]
  lifecycle_events[]
```

A plan may branch or repeat internally, but a model cannot add executable stages, replace authority checks, or synthesize a new plan. The plan and its capability manifest are hashed into the Run and context checkpoint. Production startup emits a deterministic manifest of provider adapters, tool definitions/schema hashes, role plans, policy versions, and persistence schema; a checked fixture detects unintended drift.

Checks follow five verification phases:

1. **Rust compile time:** ownership, trait bounds, exhaustive typed states, schema generation, and stage signatures.
2. **Plan construction:** internal topology, reachable terminal states, bounded loops, unique stage identities, and declared inputs/outputs.
3. **Composition:** a role plan's required capabilities and output contract match its caller, tool catalog, provider adapter, and policy profile.
4. **Run start:** the live account/model, context, source/reality binding, sandbox support, budget, and required resources make some valid path possible.
5. **Execution:** exact selected-path facts, freshness, provider events, tool arguments, effects, and terminal output.

Every invariant is enforced at the earliest sound phase. Earlier phases must not reject a Run that later facts could make valid; interface derivation must not under-claim requirements; Run-start checks reject only impossibility; execution failures are typed rather than panics. An advisory preflight may never accept something that the corresponding mandatory start check rejects under the same facts.

Stage functions declare immutable inputs and typed outputs. Adjacent handoff values are outputs; immutable Run identity/configuration are inputs; accumulators such as conversation projection and evidence sets are narrow explicit state objects; durable authority lives only in repositories. A generic mutable `RuntimeState` with optional fields for every stage is forbidden because it hides dependencies and permits invalid partial states.

Parallel stages receive isolated child state, share only declared immutable handles, and return explicit typed results to a deterministic reducer. Results are ordered by stable call/stage identity, not completion time. Conflicting writes or duplicate output identities are errors; there is no last-writer-wins merge. Failure cancels sibling work where safe and commits no aggregate result until every required branch is durably resolved.

## Runtime Flow

```text
Slice-controlled Run request
  -> assemble immutable role/context checkpoint
  -> open or resume native Session
  -> stream provider response
  -> validate native tool calls
  -> execute tools through Slice policy and sandbox
  -> persist independently captured results/evidence
  -> continue provider turn
  -> validate final typed result
  -> return proposal to the orchestrator
```

The runtime never directly commits Source of Intent, promotes a worktree, marks satisfaction, or mutates orchestration authority. It returns untrusted typed proposals and evidence references.

## Native Session API

A native turn is conceptually:

```text
NativeTurnRequest
  session_id
  run_id
  project_id
  operation_id
  purpose
  source_of_intent_revision
  project_reality_version
  context_checkpoint
  prompt
  output_schema
  tool_policy
  provider_account_ref
  model
  execution_plan_id
  capability_manifest_id
  stage_budgets
  cancellation_id

NativeTurnResult
  structured_output
  final_projection
  event_range
  evidence_ids
  usage
  stop_reason
  context_checkpoint
```

Every field that can affect authority is supplied by Slice, not generated by the model. Provider-specific values are normalized at the runtime boundary. The Run-start validator resolves the plan against a versioned capability manifest rather than inferring support from a model-name string.

## Provider Layer

Slice calls provider HTTP APIs directly with Rust HTTP/TLS libraries. The initial required families are:

- Anthropic Messages;
- OpenAI Responses;
- Google Gemini content generation;
- OpenAI-compatible providers, including OpenRouter.

A normalized provider stream is an indexed content-block protocol:

```text
block_start(index, text | reasoning | tool_call{id, call_id, name})
block_delta(index, text | reasoning | signature | tool_arguments_json_fragment)
block_stop(index)
usage(cumulative)
message_stop(reason, final_usage, response_id, resolved_model)
```

The accumulator enforces legal start/delta/stop ordering, unique active indices and call IDs, bounded bytes, one terminal event, and no events after termination. Tool arguments are untrusted text until the corresponding `block_stop`; only then are they parsed and schema-validated. A partial, duplicated, reordered, malformed, cancelled, or unterminated block never reaches tool execution. Provider response/call IDs and provider-required opaque signatures are preserved separately for correct round trips but never treated as evidence or authority. Reasoning content is retained or replayed only when provider protocol and explicit data policy permit it.

Raw provider payloads may be retained only in redacted bounded diagnostics. Normalized final usage distinguishes full-price input, cache read/write, output, and reasoning tokens where available. Cost is an estimate tied to a captured rate-card version, never billing authority.

Provider behavior must include:

- streaming and cancellation;
- tool definitions and calls;
- provider-native structured output where supported;
- strict JSON-schema validation and one bounded correction turn otherwise;
- context-window preflight and provider-reported usage;
- retry classification, rate-limit handling, and bounded backoff;
- capability discovery/versioning, including streaming, tool choice, strict schemas, structured output, reasoning round-trip, prompt caching, image support, and context/output limits;
- API-key and OAuth token refresh without exposing secrets to tools.

Prompt caching is explicit. A cache plan identifies the byte-stable system/tool prefix and selected completed message boundaries by content hash. Adapters map that plan onto provider limits and report honored versus ignored breakpoints; cache behavior never changes canonical context or correctness.

PumpkinPie keeps one canonical JSON Schema for local validation and derives a provider-compatible projection. If a provider rejects schema features, the adapter records the exact weakened/removed constraints; it must not silently alter semantics. Every structured result and tool call is validated against the canonical local schema regardless of provider-native strict mode.

The provider layer never shells out to a language runtime or vendor CLI.

## Model/Tool Loop

Slice owns the loop rather than asking a provider or external agent to own tools:

1. Persist the accepted turn and exact context binding.
2. Start the provider stream.
3. Assemble each tool call from typed deltas.
4. Reject unknown, malformed, disallowed, stale, or over-budget calls.
5. Persist tool-call intent before consequential execution.
6. Execute through the native tool supervisor.
7. Persist the complete result and evidence before sending a model-visible projection.
8. Continue until a valid terminal structured result, owner interaction, cancellation, or failure.

The loop has explicit limits for provider calls, tool calls, generated bytes, context tokens, estimated spend, elapsed time, and correction attempts. Exhaustion is a typed non-success terminal state. Provider retry applies only to classified transient failures and honors server retry guidance plus jitter; it never blindly repeats a request whose remote outcome may already exist unless the provider contract supplies a safe idempotency mechanism.

Model prose never substitutes for a tool result. A tool result sent back to the model is a bounded projection of a durable full result.

## Native Tools

A build-time catalog freezes each tool's stable name/version, description, canonical input/output schema hashes, side-effect class, evidence contract, context requirements, and default policy. Each turn derives a narrower exposed catalog from role and Run policy. Registration, exposure to the model, authorization, owner confirmation, and execution are separate operations.

Authorization is mandatory at the final dispatcher immediately before execution. Unknown and denied tools are not exposed; hidden tools remain non-model internal operations and are not invocable through the model dispatch path. Policy composition is monotonic toward less authority. Only a persisted, scoped owner grant may widen authority, and the grant is revalidated for call identity, exact arguments or allowed pattern, reality version, expiry, and single-use semantics. The model cannot carry credentials or authority through generic tool context.

The minimum built-in set is:

- `read`: files, chunks, metadata, symlinks, exact aggregate hash;
- `list`: bounded directory inventory with types and hashes where requested;
- `search`: content/path search with line ranges and truncation metadata;
- `edit`: expected-version exact replacement with atomic commit and diff evidence;
- `write`: policy-checked atomic create/replace with before/after hashes;
- `bash`: sandboxed process execution with complete output artifact and lifecycle;
- typed Git status/diff helpers where they improve safety and evidence.

### Complete file observation

A large file is one observation assembled from verified chunks:

```text
FileObservation
  normalized_path
  file_type
  byte_len
  content_hash
  chunk_ids[]
  complete
  observed_at
  project_reality_version
```

Tool-output display limits must never make complete review impossible. Slice proves aggregate completion independently; the model may consume selected chunks while evidence retains the complete observation. Symlink path and target are explicit and containment policy is checked before following anything.

### Mutation

Edit/write calls require Project-root containment, expected old bytes or version, authoritative-document protection, atomic replacement, cancellation safety, and before/after evidence. Interactive direct-checkout mutation and autonomous isolated-worktree mutation use the same tools under different policy profiles.

### Command execution

A command result records exact argv or shell subject, cwd, effective identity, environment/toolchain fingerprint, start/finish/duration, exit status/signal, cancellation, output digest, complete output artifact, retained preview, resource locks, checkpoint, and cache identity. Provider secrets and Hub/Slice administrative credentials are absent from the command environment.

Tool definitions should be generated from Rust input types where practical, with compile-fail tests for unsupported signatures and schema tests for required/optional/default/nested/tagged-enum forms. Deserialization and canonical validation still occur at the dispatcher; generated schemas do not replace runtime checks.

## Sandbox and Identity

Slice is the policy authority. On Linux it may supervise external tools with Bubblewrap and kernel facilities while retaining a Rust implementation of policy and lifecycle.

- read-only roles cannot mutate Project reality;
- implementation/validation mutate only their assigned isolated worktree and declared caches;
- interactive mode follows the visible local mutation policy;
- private temporary storage is per Run;
- capabilities are dropped by default;
- root execution requires explicit Project and local policy;
- effective uid/gid, mounts, network policy, and writable surfaces are evidence;
- provider networking occurs in Slice, not inside Project tool sandboxes.

## Scoped Runtime Context

Runtime data has explicit scopes and crossing rules:

| Scope | Examples | Mutation/crossing rule |
|---|---|---|
| Process | immutable provider/tool/plan catalogs, clocks, repository handles | shared read-only handles; no Run credentials |
| Session | conversation projection, compaction lineage | isolated per Session and serialized through typed repositories |
| Run | identity, role, Source/reality binding, policy, budget | immutable after acceptance except typed durable transitions |
| Turn | prompt, provider accumulator, tool-call set | fresh per turn; discarded only after events are durable |
| Child stage | reviewer shard, parallel tool call, correction attempt | deny by default; explicitly share immutable, copy, or create fresh |

A child context cannot mutate a parent context. Every boundary declares which typed values are shared read-only, copied, initialized fresh, or excluded. Secrets, owner-control credentials, mutable repositories, and orchestration transition capabilities are excluded unless a specific non-model stage requires a narrow handle. Outputs cross a boundary only through a declared typed return and reducer.

This scoped context is an in-memory execution aid, never recovery authority. Restart reconstructs it from SQLite events/checkpoints and revalidates every external binding.

## Context and Compaction

A Session is an append-only typed event stream plus derived context checkpoints. Persist owner/model messages, tool calls/results, structured outcomes, usage, interaction boundaries, and compaction boundaries. Do not persist hidden chain-of-thought.

Compaction is derived and content-addressed. A checkpoint binds the included event range, Source revision, Project reality, role, provider/model, summary hash, and invalidated observations. Full durable events/evidence remain available for recovery and audit. A summary is never canonical intent or satisfaction evidence.

Implementation and warm-review Sessions may resume validated context. Implementer and reviewer contexts never mix. A final approval review uses a newly isolated cold Session and fresh evidence required by policy.

## Lifecycle, Middleware, and Inspection

The runtime publishes a fixed typed lifecycle for Run, turn, provider call, content block, tool call, interaction, compaction, and execution-plan stage: accepted, started, completed, failed, cancelled, and outcome-unknown where applicable. Events carry stable Run/Session/stage/call IDs, attempt, timing, plan/manifest version, and low-cardinality error kind. Lifecycle observers cannot mutate authority or inject model-visible data.

Middleware is reserved for cross-cutting behavior that must span an operation, such as tracing, redaction, budget accounting, and evidence timing. Each middleware receives a linear continuation that must be invoked exactly once or return a typed refusal; double invocation or silent drop is an error. Ordering is deterministic and part of the runtime manifest. Telemetry failure is isolated from execution unless the failed component is explicitly correctness-critical, such as the durable evidence writer.

Value inspection is compile-time opt-in per field/type and runtime opt-in per Session/Run, off by default. Redaction occurs before formatting or fan-out; secret-bearing types implement non-revealing formatting; rendering is byte-capped and panic-isolated. Redaction rules only accumulate during a Run and cannot be removed by a later listener. Inspection listeners receive already-policy-filtered records through one flat fan-out so they cannot bypass another listener's policy or recursively wrap one another. Hidden chain-of-thought is never inspected or exported.

## Interactions

The runtime exposes native typed interaction calls rather than extension-specific events:

```text
select | confirm | input | editor | notify | status
```

Blocking calls persist before broadcast, correlate to Run/tool call, accept the first valid response, enforce Slice-side timeout, reject duplicates/stale answers, and survive TUI/GUI disconnect. Notify/status are nonblocking.

## Persistence

Native runtime authority uses SQLite in WAL mode with explicit schema migrations and foreign keys. Required tables include Sessions, Runs, events, context checkpoints, provider usage, tool calls, tool artifacts, evidence, interactions, crash records, and cancellation state. Large output artifacts may live in a content-addressed file store referenced transactionally from SQLite.

Serialized context resources carry stable storage key, owning component, schema version, and content hash. Unknown or incompatible versions fail with a migration/recovery state rather than being silently omitted. A checkpoint becomes visible only when all required resources and artifacts are durable; post-turn checkpoint failure is a Run failure or explicit degraded state, never a warning followed by reported recoverability.

No Redis service is required. Optional caches are disposable and reconstructible; SQLite and content-addressed artifacts are authoritative on the Slice.

## Failure and Recovery

Provider, runtime, and tool failure are distinct typed states. Crash records retain purpose, revision, provider/model, last event, active tool, exit/signal where relevant, redacted diagnostic tail, time, and recovery policy.

After restart, Slice reconstructs exact phase and event boundaries, validates worktree/context/evidence bindings, resumes only idempotent pending work, and never repeats a committed authority transition blindly.

Cancellation and timeout are phase-aware. Dropping an async future is not proof that an external effect stopped. Consequential work follows `intent persisted -> dispatched -> outcome observed -> result committed`; cancellation at any boundary records which facts are known. An unobserved provider request, filesystem mutation, process, or remote operation becomes `outcome_unknown` and is reconciled before retry. Per-operation timeouts identify the active stage/call and route through typed recovery. A hard whole-Run deadline is only a final containment mechanism and cannot claim rollback of effects already dispatched.

## Testing Architecture

Tests follow the same boundaries as the runtime:

- compile-time tests cover macros, state transitions, trait bounds, and invalid tool/stage signatures;
- primitive unit tests exercise provider event accumulators, schema projections, tools, policy, reducers, and repositories without constructing a full Slice;
- plan tests validate topology, signatures, phase placement, bounded loops, context crossing, and deterministic manifests;
- property tests cover plan/signature composition, stream chunking equivalence, parallel result permutation, schema normalization, and event replay;
- integration tests compose runtime, SQLite, fake providers, sandboxed tools, interactions, and restart only where composition introduces behavior;
- conformance suites run every provider adapter and tool against the same contract, including malformed and cancellation paths.

Tests assert typed variants and full outputs rather than only success/failure. They synchronize on events rather than sleeps, never assume async or map iteration order, isolate all filesystem/process state, and explain every ignored platform test. Documentation examples for public internal contracts compile as Rust tests.

## Rust-Only Gate

Production and test source must contain no first-party JavaScript, TypeScript, Python, Node package manifest, language sidecar, or generated executable script. Fake providers, scripted streams, tool fixtures, migration tests, and fault injection are Rust.

Release acceptance includes operation on a clean machine with no Node.js or Pi installation and checks that no production process attempts to discover or launch them.

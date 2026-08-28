# Data Model

All IDs are stable opaque strings. Names are mutable human projections. Slice SQLite is authoritative for situated records; Hub storage is authoritative only for Hub accounts, enrollment, routing cache, and audit.

## Slice Serve Identity

An enrolled/situated endpoint identity is created for `slice serve`; GUI-only use does not claim an endpoint identity.

```text
Slice
  slice_id
  name
  hostname
  version
  public_key
  status: unenrolled | online | offline | disabled | revoked
  hub_binding optional
  capabilities
  policy_version
  inventory_revision
  created_at / enrolled_at / last_seen_at
```

One serve identity owns one local endpoint authority and can host many Projects. Standalone TUI may own local Projects without enrollment and can explicitly attach/migrate them to service authority; GUI state remains separate and non-authoritative.

## Project

```text
Project
  project_id
  slice_id
  name
  canonical_cwd
  repository_root optional
  branch/worktree metadata
  source_of_intent_id
  intent_chat_id
  initialization_status
  project_status
  realization_status
  trust_record
  run_as_user
  allow_root_sessions
  provider_account_ref optional
  default_provider/model
  local_policy_hash
  created_at / updated_at
```

Canonical path, filesystem identity, repository identity, and Slice identity prevent path-name ambiguity. A Project can be used through Slice TUI and PumpkinPie without duplication.

## Source of Intent

```text
SourceOfIntent
  source_of_intent_id
  project_id
  revision
  format/schema_version
  generated_payload
  authoritative_bundle optional
  content_hash
  status: absent | assembling | active | updating | conflicted | unavailable
  previous_revision
  created_at / updated_at
```

Every committed revision is retained or recoverably backed up. The authoritative bundle stores exact path, bytes/content-addressed artifact, byte length, individual hash, manifest closure, and aggregate hash. Generated payload supplements but cannot replace it.

## Requirement Graph

```text
RequirementIndex
  project_id
  source_revision/hash
  generator_version
  nodes[]
  completeness_assessment
  generated_at

RequirementNode
  requirement_id
  exact source path/span/hash
  kind
  text projection
  dependencies[]
  acceptance_criteria[]
```

It is derived, disposable, and traceable to exact intent. Missing projection scope remains unreviewed.

## Intent Chat and Timeline

```text
IntentChat
  intent_chat_id
  project_id
  source_revision
  status
  next_cursor
  created_at / updated_at / last_active_at

TimelineItem
  timeline_item_id
  project_id / intent_chat_id
  operation_id optional
  session_id/run_id optional
  source_revision optional
  cursor
  kind / visibility / status
  summary / content
  created_at / updated_at / completed_at
```

Primary items communicate intent, questions, outcomes, evidence, lifecycle, and failures. Detail/diagnostic items retain native runtime activity without making it the product mental model.

## Operation and Objective Package

```text
Operation
  operation_id / request_id
  project_id / intent_chat_id
  targeted source revision
  kind / authorization_basis
  status
  error/recovery
  created_at / updated_at / completed_at

ObjectivePackage
  objective_id
  operation_id / source revision
  divergence_ids[] / requirement_ids[]
  objective / scope[]
  validation_criteria[]
  rationale / authorization_basis
  state
```

User operations and Project realization are separate lifecycles. The orchestrator, not an implementation model, selects and persists bounded objectives.

## Native Session and Run

A Session is a persistent role-specific model/tool context. A Run is one bounded turn/attempt within it.

```text
Session
  session_id
  project_id
  purpose: intent | inspection | implementation | validation | review | approval_review | recovery | interactive
  source_revision optional
  parent_operation_id optional
  provider_account_ref / provider / model
  effective_user/root
  context_checkpoint_id optional
  status: starting | idle | running | blocked | stopped | crashed | missing | stale
  created_at / updated_at

Run
  run_id / session_id
  purpose / source_revision / reality_version
  objective_id optional
  output_contract_version
  tool_policy_hash
  event range
  structured_result optional
  usage
  stop_reason / crash_record_id optional
  started_at / completed_at
```

There are no external-agent session IDs/files or language-runtime fields.

## Native Session Events

```text
SessionEvent
  event_id / session_id / run_id
  sequence
  type
  correlation_id optional
  typed payload or redacted artifact reference
  visibility
  created_at
```

Types include model lifecycle/deltas, tool requested/started/progress/completed, interaction, retry/rate limit, compaction, usage, cancellation, crash, and structured result. Hidden reasoning content is not durably stored.

## Tool Call and Artifact

```text
ToolCall
  tool_call_id / run_id
  tool name/version
  typed arguments
  policy decision
  status
  requested/started/completed timestamps
  result_artifact_id optional
  evidence_ids[]

Artifact
  artifact_id
  kind / media_type
  content_hash / byte_len
  storage_path
  redaction/retention class
  created_at
```

Large command output and file chunks live in content-addressed artifacts referenced transactionally from SQLite.

## Evidence

```text
Evidence
  evidence_id
  project_id / run_id / tool_call_id optional
  source_revision / reality_version / checkpoint
  kind
  subject
  validity_key
  content/output hash
  capture metadata
  success/exit/signal/cancellation
  artifact references
  observed_at
```

Evidence is Slice-captured. Model prose can cite evidence but cannot manufacture it.

## Divergence and Review

```text
Divergence
  divergence_id / fingerprint
  project_id / source revision
  requirement_ids[] / affected components[]
  state: open | addressed | verified | reopened | superseded
  fault / evidence_ids[] / verification_criteria[]
  attempt/reopen counts
  first/last reality
  created_at / updated_at

Review
  review_id / run_id
  project_id / source revision / reality version
  scope
  obligations[] / obligation evidence bindings[]
  requirement coverage
  findings/divergence transitions
  unreviewed_required_scope[]
  verdict: findings | approved
  reviewer_context: warm | cold
  created_at
```

Only a current cold complete approval with zero findings and no unreviewed scope can satisfy the Project.

## Workspace

```text
Workspace
  workspace_id / operation_id / project_id
  primary root/cwd
  isolated root/cwd
  branch
  base/checkpoint commits
  status: active | approved | promoting | promoted | failed | removed
  cache identity
  created_at / updated_at
```

Promotion and recovery are transactional/idempotent. Terminal workspaces and bounded caches have explicit retention/cleanup.

## Interaction

```text
Interaction
  interaction_id
  project/session/run/tool correlation
  method
  typed payload/schema
  blocking
  timeout/deadline
  status
  accepted response/client
  created_at / resolved_at
```

The first valid response wins; duplicates and stale responses are rejected.

## Provider Account

Hub and local Slice accounts share product-level metadata but separate custody:

```text
ProviderAccount
  provider_account_id
  provider_id / label
  auth_type
  encrypted secret or external credential reference
  capabilities/scopes
  status / expiry
  created_at / rotated_at / revoked_at
```

Runtime delivery uses a scoped in-memory capability. Tool subprocesses never receive it.

## Slice GUI State

`slice gui` persists non-authoritative owner-control credential reference, selection, recent Projects, drafts, cursors, subscription intent, GUI preferences, and redacted diagnostics. This state is separate from TUI/serve authority even when modes share one installation. Every cache records freshness and authority origin.

## Schema and Time

SQLite schemas and wire protocols are explicitly versioned and migrated transactionally. Durable ordering uses monotonic per-stream sequence/cursor plus timestamps; wall-clock timestamps alone never determine causality or inventory revision.

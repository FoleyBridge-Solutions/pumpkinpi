# Data Model

### Personal Hub

The initial Hub belongs to one person. It remembers that person's connected Spokes, authenticated Clients, provider credentials, preferences, Projects, and recent Intent Chats/work.

```text
hub_id
owner identity optional
authenticated client credentials
enrolled spokes
client preferences
recently used spokes/projects
provider accounts / credentials
provider usage/preferences metadata
created_at
updated_at
```

An owner identity may be used for login and recovery, but it is not a tenant or sharing boundary. Multiuser access will be designed together with multitenancy.

The Hub should not store source files by default. Project contents remain on Spokes. Source of Intent, Intent Chat, and evidence may be cached by the Hub only according to explicit retention, confidentiality, and authority rules.

### Spoke

```text
spoke_id
name
hostname
version
status: offline | online | disabled | revoked
capabilities
created_at
enrolled_at
last_seen_at
revoked_at
public_key or token hash
```

### Project

A Project is a trusted working environment on a Spoke, defined by a Source of Intent and primarily accessed through Intent Chat.

```text
project_id
spoke_id
name
cwd
source_of_intent_id
intent_chat_id
initialization_status: uninitialized | inspecting | clarifying | ready | failed
default_pi_args
default_provider optional
default_model/settings optional
run_as_user optional
allow_root_sessions: bool
status: active | missing | stale | removed
trusted: bool
created_at
updated_at
```

### Source of Intent

The Source of Intent is canonical LLM-facing Project state. Its payload is deliberately representation-agnostic and does not need to be directly readable by users.

```text
source_of_intent_id
spoke_id
project_id
format/version
revision
canonical_payload or storage_ref
authoritative_bundle optional: manifest path, exact document bytes/paths/sizes/hashes, aggregate bundle hash
content_hash covering payload and authoritative bundle
status: absent | assembling | active | updating | conflicted | unavailable
created_at
updated_at
```

Updates must be revisioned and atomic. An internal run should record which Source of Intent revision it was serving. Human-readable summaries and diffs are projections, not canonical truth.

### Intent Chat

Each Project has one primary Intent Chat with stable identity.

```text
intent_chat_id
spoke_id
project_id
source_of_intent_revision
status: ready | waiting_for_user | updating_intent | working | blocked | stale
created_at
updated_at
last_active_at
```

The Intent Chat timeline contains user messages and LLM-generated projections: questions, decisions, progress, outcomes, evidence summaries, reviewer findings/approval, and explanations of Source of Intent changes.

### Internal Session / Run

An internal Session is one persistent LLM execution associated with a Project. It may serve intent maintenance, inspection, implementation, validation, independent review, or recovery. Pi-specific fields describe the current implementation and are not normal user-facing concepts.

```text
session_id
run_id optional
spoke_id
project_id
purpose: intent | inspection | implementation | validation | review | recovery
source_of_intent_revision optional
parent_operation_id optional
name internal
cwd
status: starting | idle | running | blocked | stopped | crashed | missing | stale
run_as_user
run_as_root: bool
pi_session_id optional
pi_session_file optional
pi_leaf_id optional
pi_session_name optional
created_at
updated_at
last_active_at
```

Each active Session owns a Pi child process, IO readers/writer, command queue, subscribers, recent event buffer, state cache, lifecycle watcher, and restart policy.

### Timeline / Evidence

Intent Chat has a normalized, replayable user-facing timeline. Internal Sessions have structured execution timelines that feed evidence and diagnostics.

```text
timeline_item_id
spoke_id
project_id
intent_chat_id optional
session_id optional
run_id optional
source_of_intent_revision optional
kind: user_intent | question | decision | intent_update | progress | outcome | evidence | tool_execution | extension_ui | lifecycle | error
visibility: primary | detail | diagnostics
status optional: queued | running | blocked | completed | failed | cancelled
summary optional
content optional
raw_event_ref optional
cursor / sequence
created_at
updated_at
completed_at optional
```

Raw internal activity should not automatically flood Intent Chat. PumpkinPi promotes only information useful for intent, decision, trust, outcome, or recovery.

### Review / Satisfaction Assessment

Independent review assesses complete observed Project reality against a complete current Source of Intent revision. Findings drive further realization; only approval with no findings and no required scope left unreviewed establishes satisfaction.

```text
assessment_id
spoke_id
project_id
source_of_intent_revision
observed_reality_version
review_run_id
reviewed_scope
checks
findings
supporting_evidence_ids
unreviewed_required_scope
verdict: findings | approved
status: current | stale
created_at
updated_at
```

Any material intent or Project-reality change makes incompatible approval stale.

### Command / Operation

A command is a user or system operation with a lifecycle. One high-level intent operation may create many internal commands and Sessions.

```text
operation_id
origin_client_id optional
origin_request_id optional
spoke_id
project_id optional
intent_chat_id optional
session_id optional
type
source_of_intent_revision optional
status: queued | accepted | running | blocked | completed | failed | cancelled | rejected | unknown
error optional
created_at
updated_at
completed_at optional
```

Every user-visible action that mutates intent or remote state should have an operation/timeline record so Intent Chat can acknowledge and explain it.

### Client

```text
client_id
connection_id
credential_id
project_subscriptions
connected_at
last_seen_cursors
```

One Client may subscribe to many Project Intent Chats across many Spokes, and each Intent Chat may have many observing Clients. Internal Session subscriptions are implementation/diagnostic subscriptions rather than the primary Client model.

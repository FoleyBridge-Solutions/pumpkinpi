# Client Architecture

The Client should be a typed Intent Chat application over a protocol actor, not a Session manager or UI directly mutating JSON blobs.

## Layers

```text
Intent Chat UI
  ↓ user intent / decisions
Application Store
  ↓ typed Project requests
Protocol Actor
  ↓ websocket envelopes
Hub
```

### UI Layer

The UI renders typed state and emits user-level intents:

- connect/disconnect
- select/create/remove Project
- send an Intent Chat message
- answer a question or consequential prompt
- cancel visible work
- inspect status/evidence/changed files
- open execution details or diagnostics

Internal Session creation, subscription, queueing, and routing are not ordinary UI actions.

### Application Store

The store owns:

- connection and personal Hub state
- Spoke/Project maps
- Source of Intent revision/availability metadata (not necessarily canonical payload)
- one Intent Chat state/timeline per Project
- Project subscriptions
- pending user operations
- projected progress/outcomes/evidence
- optional internal Run detail
- unread/activity state
- diagnostics and raw event retention

The Client should not treat a rendered Source of Intent summary as canonical state. It is an LLM projection associated with a revision.

### Protocol Actor

The protocol actor owns WebSocket IO, authentication, IDs, timeouts, reconnect, Project resubscription, cursor negotiation, typed envelope translation, and diagnostic retention. It does not own UI selection, interpret intent, or orchestrate Sessions.

## Typed Client Models

Minimum sketch:

```rust
enum ConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Reconnecting { attempt: u32 },
    Degraded { reason: String },
}

struct HubState {
    spokes: BTreeMap<SpokeId, SpokeSummary>,
    projects: BTreeMap<(SpokeId, ProjectId), ProjectSummary>,
    intent_chats: BTreeMap<(SpokeId, ProjectId), IntentChat>,
    pending_operations: BTreeMap<RequestId, PendingOperation>,
    subscriptions: BTreeSet<(SpokeId, ProjectId)>,
    run_details: BTreeMap<RunId, RunDetail>, // secondary/diagnostic
}

enum IntentChatItem {
    UserIntent(UserIntent),
    AssistantProjection(AssistantProjection),
    Question(Question),
    Decision(Decision),
    Progress(Progress),
    Outcome(Outcome),
    Evidence(EvidenceSummary),
    ConsequentialPrompt(PromptState),
    Error(RecoverableError),
}
```

`serde_json::Value` may remain at protocol and diagnostics boundaries only.

## Request Lifecycle

Every user action produces a pending operation:

```text
created → sent → acknowledged → working → completed
                   ├──────────→ waiting_for_user
                   ├──────────→ rejected
                   └──────────→ failed
sent ──────────────timeout────→ unknown
```

`intent.send` creates a user message immediately. Later events may show that intent was updated, work began, a question is pending, or an outcome was reached. The user should not have to follow child Session commands.

## Intent Chat Timeline Rules

- User messages are first-class durable items.
- Assistant streaming builds an in-progress projection item and finalizes to a snapshot.
- Source of Intent changes are explained through human-readable summaries/diffs associated with revisions; canonical payload need not be sent to the Client.
- Internal tool events do not flood the primary timeline.
- Meaningful progress is promoted into compact updates.
- Outcomes state what was attempted, whether current intent appears satisfied, and what evidence supports that claim.
- Divergences become explicit questions, proposals, or blockers.
- Consequential extension UI requests become blocking items.
- Raw events and full Run timelines remain secondary diagnostics.

Late subscribers receive enough replay data to reconstruct the same Intent Chat and visible Project state.

## Reconnect Behavior

On disconnect, existing Intent Chats remain visible but stale/live-paused; unsafe actions are disabled or explicitly queued.

On reconnect:

1. authenticate
2. refresh Spoke/Project inventory
3. refresh Source of Intent revision/availability metadata
4. resubscribe to prior Project Intent Chats
5. replay from each last durable Intent Chat cursor
6. refresh visible running-work summaries
7. mark each Project live after catch-up

If replay is unavailable, clearly mark the gap and ask PumpkinPi for a fresh Project/status projection rather than pretending continuity.

## Diagnostics

Diagnostics expose connection logs, redacted protocol envelopes, request status, Project/Intent revision and cursor state, internal Run/Session details, crashes, and capability mismatches.

Normal users see explanations and actions first. Raw orchestration is one level deeper.

## Client Persistence

Persist Hub URL history, credential references, last selected Project, recent Projects, UI preferences, draft message per Intent Chat, last cursors, and diagnostics preferences. Secrets use platform credential storage where available.

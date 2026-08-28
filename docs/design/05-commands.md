# Commands

## Command Boundary

Normal Clients issue Project/Intent Chat commands. Session commands are an internal orchestration and diagnostics API. The public product must not require the user to create, name, attach to, or queue internal Sessions.

## User-Facing Commands

### Hub / Project Discovery

```json
{"type":"hub.status"}
{"type":"spoke.list"}
{"type":"project.list"}
{"type":"project.get","spoke_id":"spoke_home","project_id":"proj_api"}
```

### Project Initialization

```json
{"type":"project.initialize","spoke_id":"spoke_home","cwd":"/home/me/app","name":"app"}
{"type":"project.initialization_status","spoke_id":"spoke_home","project_id":"proj_app"}
{"type":"project.remove","spoke_id":"spoke_home","project_id":"proj_app"}
```

Initialization inspects local context and opens Intent Chat to assemble the initial Source of Intent. It is a lifecycle, not a single synchronous directory-registration call.

### Intent Chat

```json
{"type":"intent.send","spoke_id":"spoke_home","project_id":"proj_app","message":"The CLI should support JSON output"}
{"type":"intent.cancel","spoke_id":"spoke_home","project_id":"proj_app","operation_id":"op_123"}
{"type":"intent.subscribe","spoke_id":"spoke_home","project_id":"proj_app","cursor":"42"}
{"type":"intent.get_projection","spoke_id":"spoke_home","project_id":"proj_app","projection":"summary"}
```

`intent.send` may clarify intent, update the Source of Intent, initiate work, answer a question, or request an explanation. The Intent Agent interprets the message in Project context; the user need not choose an internal command category.

`intent.get_projection` asks an LLM to render canonical Source of Intent state into a human-readable summary, explanation, diff, or other supported projection. It does not expose canonical storage as the normal UI.

## Internal Session Commands

Internal commands include Session create/list/subscribe/stop/restart/delete/send and full routing metadata. They are used by PumpkinPi orchestration and diagnostics, not ordinary Intent Chat UI.

The Session payload uses PumpkinPi command types translated by the Spoke into documented Pi RPC commands, including prompt/steer/follow-up, abort/clear queue, model/settings/state queries, direct bash, session stats, entry retrieval, extension UI responses, and compaction/retry controls.

Pi commands that mutate internal Session binding (`new_session`, `switch_session`, `fork`, `clone`) remain denied unless a PumpkinPi wrapper atomically updates the Session registry.

Danger is not limited to direct RPC `bash`: prompts can induce tools, extensions can execute immediately, and skills/templates can change behavior. UX, evidence, and audit should distinguish direct bash, agent tool execution, extension commands, and prompt/skill expansion.

## Intent Revision Semantics

Operations that implement or validate intent record the Source of Intent revision they target.

If a newer revision appears:

- harmless inspection may continue and report its revision
- implementation/validation must be assessed for staleness
- incompatible work should be cancelled or superseded
- outcomes must not be claimed against a revision they did not evaluate

Source of Intent writes use optimistic revision checks or equivalent atomic serialization to prevent lost updates.

## Per-Session Queue Priority

Internal commands are serialized per Session, but cancellation and UI responses must not wait behind normal work:

1. lifecycle/emergency stop and process cleanup
2. blocking `extension_ui_response`
3. cancellation: abort bash/retry, clear queue, abort
4. normal prompts, steering, settings, queries, and bash

Rules:

- UI responses are valid only for pending request IDs.
- “Stop everything” clears queued continuations before aborting active work.
- stop attempts graceful shutdown then kills after timeout.
- ordinary commands are rejected for crashed, missing, stopped, or stale Sessions.
- cancelling an internal Run must produce a coherent visible outcome in Intent Chat when it originated from user intent.

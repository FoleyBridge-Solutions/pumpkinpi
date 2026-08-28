# Protocol

## Protocol Layers

### Pi RPC Layer

Pi RPC command example:

```json
{"id":"req-1","type":"prompt","message":"Fix tests"}
```

Pi RPC event example:

```json
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"..."}}
```

Pi RPC uses JSONL framing:

- one JSON object per LF-delimited line
- split only on `\n`
- strip optional `\r`
- do not split on Unicode line separators

### PumpkinPi API / Envelope Layer

PumpkinPi uses its own API between Clients, Hub, and Spokes. The Spoke is the Pi adapter: it translates PumpkinPi commands into Pi RPC commands, reads and understands Pi RPC events, updates Spoke-side session state, and sends only normalized PumpkinPi events/snapshots upstream.

External PumpkinPi messages need routing metadata and protocol metadata.

All PumpkinPi envelopes should include or negotiate:

```text
protocol_version
message id for request/response messages
spoke_id / project_id when targeting user-visible Project work
session_id / run_id when targeting internal execution
source_of_intent_revision where relevant
capabilities optional
```

Unknown command types should be rejected by default. Raw Pi events should not be forwarded as the public PumpkinPi API. Unknown Pi event fields may be stored in Spoke diagnostics, but public Hub/Client events should remain normalized and versioned.

Normal Client to Hub traffic targets Intent Chat rather than an internal Session:

```json
{
  "id": "client-req-1",
  "type": "intent.send",
  "spoke_id": "spoke_home",
  "project_id": "proj_api",
  "message": "The failing tests should be fixed without changing public behavior"
}
```

PumpkinPi may translate this into Source of Intent updates and one or more internal Session commands. An internal Hub-to-Spoke command may look like:

```json
{
  "id": "hub-req-99",
  "type": "session.send",
  "project_id": "proj_api",
  "session_id": "sess_tests",
  "command": {
    "type": "prompt",
    "message": "Fix the failing tests"
  }
}
```

Spoke to Pi:

```json
{"type":"prompt","message":"Fix the failing tests"}
```

Pi to Spoke:

```json
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"..."}}
```

Spoke to Hub:

```json
{
  "type": "session.output_delta",
  "spoke_id": "spoke_home",
  "project_id": "proj_api",
  "session_id": "sess_tests",
  "message_id": "msg_123",
  "role": "assistant",
  "delta": "..."
}
```

Hub forwards normalized PumpkinPi events to subscribed clients.

Because Clients can work across all enrolled Spokes simultaneously, the Hub must preserve the full routing envelope on every event. Clients should never infer target context from connection state alone. Primary Client events route by Project/Intent Chat; internal execution and diagnostic events additionally route by Session/Run.

The Spoke should process Pi RPC events into PumpkinPi state and event types. For example, it should assemble assistant message deltas, treat Pi `message_end.message` as authoritative for the final message snapshot, translate tool/bash lifecycle events into stable PumpkinPi tool events, and emit session status changes such as `session.running`, `session.idle`, `session.crashed`, and `session.stopped`.

## Client Multiplexing

A Client connection is a multiplexed channel to all enrolled Spokes. The Client should be able to:

- list every Spoke and Project it can access
- subscribe to many Project Intent Chats concurrently
- send intent to different Projects without separate connections
- receive interleaved Project status/outcome events
- correlate primary events by `spoke_id` and `project_id`
- optionally inspect internal Sessions/Runs with their additional IDs
- resume after reconnect using durable cursors where possible

Example interleaved client workflow:

```json
{"id":"1","type":"intent.send","spoke_id":"spoke_home","project_id":"proj_app","message":"Make the test suite pass"}
```

```json
{"id":"2","type":"intent.send","spoke_id":"spoke_work","project_id":"proj_backend","message":"Investigate this production error without deploying changes"}
```

The hub may route these commands to different spoke WebSockets concurrently. Responses and events from both sessions may arrive interleaved, so clients must demultiplex by envelope IDs.


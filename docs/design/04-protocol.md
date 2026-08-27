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

PumpkinPi uses its own API between Clients, Hub, and Nodes. The Node is the Pi adapter: it translates PumpkinPi commands into Pi RPC commands, reads and understands Pi RPC events, updates Node-side session state, and sends only normalized PumpkinPi events/snapshots upstream.

External PumpkinPi messages need routing metadata and protocol metadata.

All PumpkinPi envelopes should include or negotiate:

```text
protocol_version
message id for request/response messages
node_id / project_id / session_id when targeting work
capabilities optional
```

Unknown command types should be rejected by default. Raw Pi events should not be forwarded as the public PumpkinPi API. Unknown Pi event fields may be stored in Node diagnostics, but public Hub/Client events should remain normalized and versioned.

Client to Hub:

```json
{
  "id": "client-req-1",
  "type": "session.send",
  "node_id": "node_home",
  "project_id": "proj_api",
  "session_id": "sess_tests",
  "command": {
    "type": "prompt",
    "message": "Fix the failing tests"
  }
}
```

Hub to Node:

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

Node to Pi:

```json
{"type":"prompt","message":"Fix the failing tests"}
```

Pi to Node:

```json
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"..."}}
```

Node to Hub:

```json
{
  "type": "session.output_delta",
  "node_id": "node_home",
  "project_id": "proj_api",
  "session_id": "sess_tests",
  "message_id": "msg_123",
  "role": "assistant",
  "delta": "..."
}
```

Hub forwards normalized PumpkinPi events to subscribed clients.

Because clients can work across all authorized nodes simultaneously, the hub must preserve the full routing envelope on every event. Clients should never infer target context from connection state alone.

The Node should process Pi RPC events into PumpkinPi state and event types. For example, it should assemble assistant message deltas, treat Pi `message_end.message` as authoritative for the final message snapshot, translate tool/bash lifecycle events into stable PumpkinPi tool events, and emit session status changes such as `session.running`, `session.idle`, `session.crashed`, and `session.stopped`.

## Client Multiplexing

A client connection is a multiplexed channel to all authorized nodes. The client should be able to:

- list every node it can access
- list projects per node
- list sessions per project
- subscribe to many sessions concurrently
- send commands to different sessions without opening separate client connections
- receive interleaved events from multiple sessions
- correlate each event by `node_id`, `project_id`, and `session_id`
- resume after reconnect using durable cursors where possible

Example interleaved client workflow:

```json
{"id":"1","type":"session.send","node_id":"node_home","project_id":"proj_app","session_id":"sess_tests","command":{"type":"prompt","message":"Fix tests"}}
```

```json
{"id":"2","type":"session.send","node_id":"node_work","project_id":"proj_backend","session_id":"sess_bug","command":{"type":"prompt","message":"Investigate this error"}}
```

The hub may route these commands to different node WebSockets concurrently. Responses and events from both sessions may arrive interleaved, so clients must demultiplex by envelope IDs.


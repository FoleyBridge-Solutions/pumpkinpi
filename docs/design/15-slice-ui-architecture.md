# Slice GUI Architecture

`slice gui` is the PumpkinPie graphical Client. It is a mode of the same all-Rust Slice executable that also provides the standalone TUI (`slice`) and execution endpoint (`slice serve`). Shared packaging and code do not collapse their authority roles.

There is no separate PumpkinPie Client executable or `client` product. The existing native GUI is migrated into the `slice` crate and launched only by `slice gui`.

## Role Separation

```text
slice [PATH]
  standalone TUI + local native runtime/tools

slice serve
  local Project authority + scheduler + Hub endpoint channel + local IPC

slice gui
  typed GUI store + Hub owner-control channel
```

A person may run any combination. `slice gui` can control Projects on enrolled serve endpoints, including one on the same machine, but does not gain local filesystem/runtime authority merely by co-location. Local direct attachment is explicit and uses the same authenticated protocol/policy.

An installation can therefore hold distinct credentials:

- serve identity key: proves one enrolled execution endpoint;
- owner-control credential: authorizes GUI actions across the personal Hub;
- local IPC credential/peer policy: attaches TUI/diagnostics to local service.

They are never interchangeable.

## GUI Layers

```text
Dioxus native GUI
  -> typed application store
  -> protocol actor
  -> Hub owner-control WebSocket/HTTP
```

The GUI does not mutate JSON blobs, Slice SQLite authority, provider streams, or Project files directly.

## Typed Store

State includes Hub connection/auth, enrolled Slices, Projects, Source metadata, Intent Chats/timelines, pending requests/messages, operations, interactions, reviews, divergences, requirement metadata, Sessions/diagnostics, provider accounts/models, freshness, and preferences.

Routes use `slice_id + project_id`. Every cached object records authority/freshness and explicit offline/stale/gap state.

## Protocol Actor

The actor owns request IDs, authentication, timeouts, reconnect, subscriptions, cursors, replay/deduplication, route translation, and redacted diagnostics. Hub creates collision-safe routed IDs for endpoint forwarding.

UI components emit typed product commands and never handle provider or endpoint raw payloads.

## Optimistic Lifecycle

Sending intent creates a visible item keyed by exact request ID. Transport receipt/accepted status does not remove it. It is atomically represented only when the correlated durable timeline item arrives; correlated failure marks it failed with retry/edit actions. Concurrent messages cannot erase one another.

```text
composing -> sending -> accepted/pending durable representation -> represented
                    -> failed | unknown
```

## Information Architecture

The GUI preserves PumpkinPie's Project/Intent mental model:

- left: Slices, Projects, recent/attention state;
- center: selected Intent Chat and composer;
- right: Source revision/status, situated path/machine/user/trust/provider/model/risk, realization/review/divergence/evidence;
- secondary diagnostics: internal Sessions/Runs/tools/retries/crashes.

It does not become a generic standalone coding TUI; that is `slice` mode.

## Interactions

Pending native interactions appear in exact Project/operation/Run context. GUI sends method-specific typed answers and retains pending state until authoritative resolution/rejection. The endpoint Slice enforces first-valid response and timeout.

## Reconnect

On disconnect GUI keeps cached data visibly stale, disables unsafe actions, preserves drafts/pending/desired subscriptions/cursors, and reconnects with backoff. After auth it reconciles inventory/snapshots and replays. Gaps are explicit; a fresh snapshot does not pretend missing history was replayed.

Closing `slice gui` never stops accepted endpoint work.

## Persistence

GUI persistence is non-authoritative: Hub URL, protected owner credential reference, selection, recents, drafts, cursors, subscription intent, UI/accessibility preferences, and bounded diagnostics. It is separate from serve/TUI authority databases even when modes share a machine.

## Native GUI

`slice gui` remains Rust and native (current Dioxus-native direction or a later Rust-native toolkit selected explicitly). It cannot require browser/electron/Node assets or a language sidecar.

## Tests

Rust tests cover reducers, request correlation, optimistic continuity, reconnect/replay/dedup/gaps, complete snapshot merges, stale action policy, interactions, owner-versus-endpoint credential separation, local/remote same-machine routing, preferences, and GUI lifecycle independence from endpoint work.

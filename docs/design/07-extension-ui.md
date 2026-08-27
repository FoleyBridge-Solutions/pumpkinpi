# Extension Ui

## Extension UI Handling

Pi RPC can emit `extension_ui_request`.

There are two classes of extension UI requests:

- dialog methods (`select`, `confirm`, `input`, `editor`) block Pi until a matching `extension_ui_response` is sent
- fire-and-forget methods (`notify`, `setStatus`, `setWidget`, `setTitle`, `set_editor_text`) do not expect a response

PumpkinPi needs ownership rules:

1. Prefer the client that initiated the command that caused the request when causality is known.
2. If unknown, broadcast to attached clients.
3. Accept first valid response for dialog methods.
4. Drop stale responses after Pi has timed out or the request is no longer pending.

Pi handles dialog timeouts agent-side when a timeout is provided. PumpkinPi should forward timeout metadata to clients but does not need to implement the timeout itself.


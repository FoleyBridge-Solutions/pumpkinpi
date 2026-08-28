# Extension Ui

## Extension UI Handling

Pi RPC can emit `extension_ui_request`.

There are two classes of extension UI requests:

- dialog methods (`select`, `confirm`, `input`, `editor`) block Pi until a matching `extension_ui_response` is sent
- fire-and-forget methods (`notify`, `setStatus`, `setWidget`, `setTitle`, `set_editor_text`) do not expect a response

PumpkinPi should translate consequential or blocking requests into Intent Chat rather than exposing which internal Session produced them.

Ownership rules:

1. Associate the request with its Project, internal Run, and affected intent operation.
2. Prefer the Client that initiated the Intent Chat operation when causality is known.
3. If unknown, broadcast the Project-level blocking item to Clients observing that Intent Chat.
4. Accept the first valid response for dialog methods.
5. Route the response back to the originating internal Session.
6. Drop stale responses after Pi has timed out or the request is no longer pending.

Fire-and-forget status/widgets from internal Sessions should not automatically become primary UI. Promote them only when useful for Project progress, intent, trust, or outcome.

Pi handles dialog timeouts agent-side when a timeout is provided. PumpkinPi should forward timeout metadata to clients but does not need to implement the timeout itself.


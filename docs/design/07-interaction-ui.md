# Native Interaction UI

The native runtime exposes typed interaction requests independent of provider and front end.

## Methods

Blocking:

- `select`
- `confirm`
- `input`
- `editor`

Nonblocking:

- `notify`
- `status`

Each request records Project, operation, Session, Run, ToolCall where applicable, method-specific schema, deadline, visibility, and authorization context.

## Lifecycle

1. Slice persists the request before broadcasting it.
2. Blocking runtime work enters `blocked` while provider/event consumption continues safely.
3. Local TUI and observing `slice gui` instances may render the request.
4. Slice validates method-specific response shape and authority.
5. The first valid response wins transactionally.
6. Duplicates and responses after timeout/cancellation/resume are rejected as stale.
7. Slice records and broadcasts resolution, then resumes the exact Run.

Timeout is enforced by Slice, not delegated to a provider or UI. Disconnecting the rendering TUI/GUI does not lose the request. Nonblocking methods never appear as required actions and never stall the model loop.

Consequential confirmation requests describe Slice/machine, cwd/worktree, effective user/root state, intended mutation/tool capability, provider/model, Source revision, and why confirmation is required. Ordinary active-intent increments do not invent confirmation gates absent policy.

# Sessions

## Multiple Projects and Sessions

A node can host many projects:

```text
Node
  ├─ Project: /home/me/app
  │   ├─ Session: fix-tests
  │   └─ Session: refactor-auth
  ├─ Project: /home/me/website
  │   └─ Session: landing-page
  └─ Project: /home/me/dotfiles
      └─ Session: config-work
```

Multiple sessions can run in parallel. Each active session has its own Pi RPC process.

Commands are serialized per session, not globally:

```text
session A queue → pi process A
session B queue → pi process B
session C queue → pi process C
```

Multiple clients may attach to the same session at the same time.

Client disconnect must not kill the session by default.

## Pi Process Execution Identity

The Node daemon may run as root, but each Pi subprocess has an explicit `run_as_user` / `run_as_root` setting.

Default behavior:

- project sessions run as the project owner or configured project `run_as_user`
- root Pi sessions are denied unless `allow_root_sessions` is true for the project and the requesting user is authorized for root sessions on that node
- the effective user is recorded in session metadata and audit logs
- provider credentials delivered to the Pi process are accessible to that effective user and to the privileged Node daemon

## Process Death and Recovery

If a Pi subprocess exits unexpectedly, the Node must:

1. mark the PumpkinPi session `crashed`
2. record exit status/signal, stderr tail, timestamp, and last known Pi session metadata
3. broadcast `session.crashed` to subscribers
4. reject ordinary `session.send` commands while crashed, except lifecycle commands such as `session.restart`, `session.stop`, `session.delete`, and diagnostic queries

Restart behavior is explicit, not implicit:

- `session.restart` starts a new Pi RPC process for the same PumpkinPi `session_id`
- if `pi_session_file` still exists, the Node should launch/switch Pi to that file when supported and then refresh `get_state` / `get_entries`
- if the Pi session file is missing, the session becomes `missing` and requires either restore, clone/new-session wrapper, or delete
- the PumpkinPi `session_id` remains stable across restart; Pi's internal `sessionId`, `sessionFile`, `leafId`, and `sessionName` must be refreshed after restart
- auto-restart may be added later as a per-session policy, but the default should be no auto-restart to avoid repeating destructive tool actions

Late subscribers to a restarted session should receive a metadata snapshot plus durable entries from Pi where possible, not only the in-memory pre-crash event ring buffer.


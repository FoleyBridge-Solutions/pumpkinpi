# Data Model

### Account / User

A client authenticates to a Hub account. The Hub account is where PumpkinPi remembers the user's global state.

The Hub should persist:

```text
user_id / account_id
email / username
auth identities
accessible nodes
client preferences
recently used nodes/projects/sessions
provider accounts / credentials
provider usage/preferences metadata
created_at
updated_at
```

The Hub account should remember enough metadata to make the client experience seamless across devices:

- which nodes belong to or are shared with the user
- project/session metadata announced by those nodes
- provider accounts created from one-time client login
- provider usage metadata and preferred providers/models
- default session settings
- UI preferences
- audit/history metadata

The Hub should not store source files by default. Project contents remain on nodes. Session event history may be cached by Hub only according to explicit retention policy.

### Node

```text
node_id
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

A project is a trusted working directory on a node.

```text
project_id
node_id
name
cwd
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

### Session

A session is one Pi agent conversation/runtime associated with a project.

```text
session_id
node_id
project_id
name
cwd
status: starting | idle | running | stopped | crashed | missing | stale
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

Each active session runtime owns:

```text
Pi child process
stdin writer
stdout reader
stderr reader
per-session command queue
attached/subscribed clients
recent event ring buffer
last durable Pi entry id / leaf id
state cache
lifecycle watcher
restart policy
```

### Client

```text
client_id
user_id
connection_id
subscriptions
connected_at
```

Client subscriptions are many-to-many. One client may subscribe to many sessions across many nodes, and one session may have many subscribed clients.

```text
client_id -> set<(node_id, project_id, session_id)>
session   -> set<client_id>
```


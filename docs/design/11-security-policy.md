# Security Policy

PumpkinPi uses a simple trust model: access to a Node means administrative access to that Node through PumpkinPi. There is no fine-grained authorization model for individual projects, sessions, prompts, shell commands, tools, or provider credential use on that Node. Provider accounts and project provider/model defaults are still stored and selectable through the Hub.

Node-side enforcement:

- project allowlist / trusted cwd list for discoverability and safety rails
- max concurrent sessions
- max sessions per project
- no arbitrary project cwd without trust
- canonical path checks, symlink handling, and containment rules for trusted projects
- Pi subprocesses run as the configured project/session user by default; root Pi subprocesses require explicit local policy
- The Node daemon may run as root and can launch/manage sessions for local users, so Node access must still be treated as administrative machine access

Hub-side enforcement:

- user authentication
- whether a logged-in user has access to a Node
- audit logging with redaction rules for prompts, tool output, paths, command args, and provider data


# Security Policy

PumpkinPi initially uses a personal-Hub trust model. Every authenticated Client of the Hub may exercise PumpkinPi's full administrative capabilities on every enrolled Spoke through Intent Chat and secondary administrative surfaces. There is no fine-grained authorization model for individual Projects, Source of Intent updates, internal Sessions, prompts, shell commands, tools, or provider credential use.

This is intentionally not a partial multiuser design. Sharing and user-specific authorization will be designed together with multitenancy.

Spoke-side enforcement:

- project allowlist / trusted cwd list for discoverability and safety rails
- max concurrent internal Sessions/Runs
- max internal Sessions/Runs per Project
- no arbitrary project cwd without trust
- canonical path checks, symlink handling, and containment rules for trusted projects
- Pi subprocesses run as the configured Project/internal Session user by default; root Pi subprocesses require explicit local policy
- The Spoke daemon may run as root and can launch/manage internal Sessions for local users, so Spoke access must still be treated as administrative machine access

Hub-side enforcement:

- owner authentication and recovery where configured
- Client credential authentication and revocation
- Spoke enrollment, authentication, disablement, and revocation
- audit logging with redaction rules for prompts, tool output, paths, command args, and provider data


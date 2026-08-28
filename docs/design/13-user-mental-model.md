# User Mental Model

PumpkinPi brings the Projects and LLM capabilities of all of one person's computers into a personal Hub while keeping execution grounded on the Spokes where the real context lives.

The user's governing mental model is **unified at the Hub, situated on the Spoke, directed by intent**.

The user is not thinking "I am managing sessions and queues across machines." They are thinking something closer to:

> My projects each have one living Intent Chat. I use that chat to explain what the project should be. PumpkinPi keeps the project moving toward that intent on the right machine, in the right directory, with visible evidence and safety cues.

## The Primary User Object Is Intent Chat

The only primary user-facing object inside a Project is its **Intent Chat**.

Intent Chat is the conversational interface to the Project's **Source of Intent**. Through it, the user:

- states goals
- changes direction
- answers questions
- approves decisions
- asks for work
- receives progress and outcome summaries
- sees important evidence
- resolves ambiguities or divergences

The user should not need to understand or manage implementation sessions, validation runs, queues, worker agents, or protocol resources during normal use. Those are PumpkinPi's responsibility.

## Source of Intent

A Project is defined by a **Source of Intent**: a durable LLM-facing representation of the project's purpose, goals, constraints, decisions, validation strategy, current status, and open questions.

The Source of Intent is not user space and does not need to be directly human-readable. It is maintained conversationally through Intent Chat. Because project intent is broad, ambiguous, and evolving, the best interface for it is an LLM conversation. When the user needs to understand it, the LLM renders an appropriate summary, explanation, diff, or question into Intent Chat.

PumpkinPi's job is to make project reality conform to the Source of Intent. It iterates implementation and independent whole-Project review however many times are required until the reviewer finds no fault, returning to the user when intent is unclear, unsafe, contradicted by reality, blocked, or approved as satisfied.

## The World Contains Places

A Spoke is not merely a network endpoint. To the user, it is a place where work can happen.

Examples:

- home laptop
- work desktop
- lab server
- homelab box
- cloud VM
- family machine
- CI-like build host
- GPU workstation

Each place has a personality and trust profile: nearby or remote, personal or work-owned, powerful or scarce, online or intermittent, secret-bearing or disposable.

The product should let users recognize a Spoke as a real machine with consequences, not just an ID.

## Machines Contain Real Context

Users care about Spokes because the context is already there:

- source trees
- local dependencies
- checked-out branches
- build caches
- editor/project config
- credentials
- shells and PATH
- local services
- databases
- Docker sockets
- language toolchains
- hardware devices
- private network access

PumpkinPi's promise is: **intent can be realized inside the same context where the work already lives.**

Before consequential work starts, the user should be able to see:

```text
Machine: work-desktop
Project: /home/psi/backend
Source of Intent: backend project definition
Branch: main
Run as: psi
Provider/model: default work account / chosen model
Trust: project allowed by spoke policy
Risk: can edit files and run tools in this environment
```

## Projects Are Trust Boundaries and Work Domains

A Project is not just a cwd. It is a named work domain that says:

> I am comfortable letting PumpkinPi realize this Source of Intent here under these rules.

A Project carries:

- path identity
- human name
- Source of Intent
- Intent Chat
- owner/default run user
- provider/model defaults
- safety policy
- root allowance policy
- history and evidence
- expected tools
- repository/worktree metadata where available

Users may have many projects on one machine, each with different expectations: dotfiles, backend, experiments, kernel trees, websites, production-adjacent services.

## Work Has Continuity

The killer mental model is continuity:

> I can clarify intent here, leave, and return elsewhere to see what PumpkinPi did, what changed, what remains unclear, and whether reality now matches intent.

This implies:

- closing the client should not feel dangerous
- network drops should be recoverable
- Intent Chat should keep its identity
- the Source of Intent should be durable
- work timelines should replay with gap detection
- important outcomes should be summarized back into Intent Chat
- failure should preserve enough evidence to continue

## Agency Must Be Legible Without Becoming User Space

PumpkinPi is about delegating agency into machines, but orchestration should not become the user's workspace.

The user needs to know:

- what intent PumpkinPi is pursuing
- where work is happening
- what it is allowed to do
- whether it is running, blocked, failed, or done
- what evidence supports its conclusion
- when it needs supervision

The user does not need to manage the internal topology of agents and sessions unless something requires diagnostics or explicit recovery.

## Power Requires Situated Safety

The trust model is intentionally simple: access to a Spoke is administrative power through PumpkinPi. But users still need situated safety cues.

They need to see:

- which machine is being controlled
- which project path is active
- which user the process runs as
- whether root is possible or active
- whether secrets/providers are available
- whether the spoke is personal/work/shared/production-like
- whether a request is likely to run tools or edit files
- whether a prompt is asking for a consequential choice

Safety is not only permission checks. It is comprehension at the moment of action.

## The Hub Unifies Without Relocating Work

The personal Hub is the coherent place from which the owner reaches all connected Spokes, Projects, Intent Chats, and current work. It provides authentication, routing, presence, cached metadata, audit, recent activity, and cross-device continuity.

But Project reality and live execution remain situated on Spokes.

Implications:

- source files are not uploaded to PumpkinPi by default
- offline spokes mean stale/cached visibility, not live control
- hub metadata can be useful but may be stale
- recovery often requires the spoke to reconnect

## Many Projects Can Be Moving at Once

Users may have multiple Projects with ongoing intent-driven work:

```text
home-laptop / dotfiles      updating config, needs review
work-desktop / backend      fixing auth race, tests running
lab-server / experiments    benchmarking, waiting for GPU
cloud-vm / website          migration blocked on confirmation
```

The Hub must support awareness of concurrent work without making the user manage a fleet of sessions.

## Interruption Is Normal

Remote agent work will be interrupted by laptop sleep, mobile network changes, spoke reboot, hub restart, process crash, provider failure, extension UI timeout, long-running shell commands, and user device switching.

A resumed Project should answer through Intent Chat:

- What intent were we pursuing?
- What happened while I was away?
- What changed?
- What evidence was collected?
- Is anything blocked or unsafe?
- What should I decide next?

## Names Are Human Handles, IDs Are Reality

Users think in names: "framework", "backend", "fix tests", "GPU box".

The system must think in stable IDs: `spoke_id`, `project_id`, `intent_chat_id`, internal `session_id`, timeline cursors, and Pi metadata.

The product should use names for recognition but preserve IDs for correctness. Duplicate names are normal. Paths, hostnames, and status help disambiguate.

## The User Maintains Intent

The core interaction is supervision of delegated work through intent:

- clarify intent
- assign outcomes
- observe summarized progress
- answer questions
- redirect
- cancel
- inspect evidence
- recover
- decide when done

Chat is not merely an input mode. Intent Chat is the Project interface.

## Emotional Contract

PumpkinPi should feel:

- calm under concurrency
- explicit about risk
- reliable across disconnects
- respectful of machine boundaries
- fast to resume
- transparent when confused
- powerful without being slippery

It should not feel:

- like a chat demo
- like a raw protocol console
- like a session manager
- like a remote shell with hidden automation
- like work disappears when the window closes
- like the user has to remember where danger is

## Design Implications

1. Treat Intent Chat as the only primary Project surface.
2. Treat Source of Intent as the canonical project definition.
3. Treat Spokes as places with trust, presence, and capability.
4. Treat Projects as trusted work domains, not bare paths.
5. Treat internal Sessions as execution machinery, not user workspace.
6. Treat timelines as evidence and history, not raw text streams.
7. Treat reconnect/replay as core behavior.
8. Treat diagnostics as user-facing recovery tools.
9. Keep execution context visible at every consequential action.
10. Expose intent, outcomes, decisions, risk, and evidence; hide orchestration unless needed.

# Source of Intent

The **Source of Intent** is the canonical, durable representation of what a Project is, what it is trying to become, and how PumpkinPi should judge whether situated work satisfies that intent.

It is broader than a task list, less brittle than a formal specification, and more operational than a static design document. It is the living project definition. It does **not** need to be directly readable or pleasant for users to edit.

PumpkinPi's central product loop is:

```text
User clarifies intent through Intent Chat
  ↓
Source of Intent is updated
  ↓
PumpkinPi performs situated implementation / validation work
  ↓
An independent reviewer checks complete Project reality against complete intent
  ├─ findings drive another implementation iteration
  └─ no findings establishes satisfaction for that revision
  ↓
Evidence and divergences are reported back through Intent Chat
  ↓
Intent is refined or accepted
```

The user maintains intent. PumpkinPi realizes intent. Agents perform situated work. Realization continues for however many implementation-and-review iterations are needed until independent review finds no fault, or progress requires the owner or is explicitly paused, cancelled, or blocked.

The normative interpretation, authorization, reconciliation, evidence, and satisfaction semantics for this loop are defined in [Intent Interpretation and Realization](16-intent-orchestration.md).

## Intent Chat

Each Project has one primary user-facing conversational surface: its **Intent Chat**.

The Intent Chat is the conversational interface to the Project's Source of Intent. It is not one arbitrary session among many. It is the place where the user:

- explains what the project should be
- answers clarifying questions
- makes decisions
- changes goals and constraints
- reviews proposed Source of Intent updates
- asks for work to be done
- receives outcome summaries and evidence
- resolves divergences between desired intent and project reality

The user should not normally manage implementation sessions, validation jobs, queues, worker agents, raw transcripts, or orchestration topology. Those are PumpkinPi internals surfaced only when needed for trust, progress, or diagnostics.

## Source of Intent Representation

The Source of Intent is an LLM-facing project definition, not a user-authored document surface.

The exact representation can evolve. V1 stores conversational intent as structured Markdown plus an optional content-addressed authoritative document bundle. When a Project declares design documents as authority, their exact bytes, paths, sizes, individual hashes, manifest closure, and aggregate bundle hash are canonical; an LLM-generated synthesis may supplement that bundle but can never replace it. Future representations may include structured records, agent memory, embedded summaries, decision graphs, or validation criteria.

Activation fails if a declared document is missing, unlinked from the closed manifest, outside the Project, non-UTF-8, changed during ingestion, or absent from an agent's typed path/hash coverage. Implementation and review Runs must cover every bundled document. They may not modify authoritative documents; detected modifications are rejected and restored from the canonical bundle.

Conceptually, it should preserve things like:

- project purpose
- current goals and non-goals
- desired external behavior
- architecture and important concepts
- constraints
- safety and trust boundaries
- development commands and validation strategy
- decisions
- open questions
- current status

When the user needs to inspect or understand the Source of Intent, PumpkinPi should ask an LLM to project it into an appropriate human-readable explanation, summary, diff, or question inside Intent Chat. Human-readable views are outputs of the system, not necessarily the canonical storage format.

A raw Source of Intent viewer/editor is not a primary product requirement. The canonical representation may be exportable for diagnostics and recovery, but the normal interface is always the LLM through Intent Chat.

## Project Initialization

Creating a Project means assembling its initial Source of Intent, not merely registering a directory.

A good initialization flow:

1. User selects a Spoke and directory.
2. PumpkinPi inspects local context where allowed: repository metadata, file layout, README/design docs, package manifests, test commands, current branch, and local policy.
3. Intent Chat asks focused clarifying questions.
4. PumpkinPi assembles the initial Source of Intent.
5. Intent Chat presents a human-readable summary, questions, and proposed assumptions.
6. User confirms, corrects, or extends that projection through Intent Chat.
7. PumpkinPi updates the canonical Source of Intent.
8. The Project becomes ready for normal intent-driven work.

The user should never be dropped into a blank generic chat for a new Project without help constructing the Source of Intent.

## Execution Is Subordinate to Intent

Implementation and validation sessions exist to make project reality conform to the Source of Intent.

They may:

- inspect files and local state
- propose Source of Intent amendments
- implement changes
- run tests or other validation commands
- gather evidence
- report blockers
- summarize outcomes

But they are not the user's primary workspace. Their outputs should be condensed back into Intent Chat as progress, decisions, evidence, or requested clarification.

## Evidence and Divergence

PumpkinPi should continuously distinguish:

- intent: what the Source of Intent says should be true
- reality: what local files, commands, tests, and tools show is true
- evidence: why PumpkinPi believes reality does or does not match intent
- divergence: where reality conflicts with intent or intent is too vague

When divergence is found, PumpkinPi should not silently improvise forever. It should return to Intent Chat with a clear question, proposed design update, or implementation choice.

## User-Facing Principle

PumpkinPi should expose intent, outcomes, decisions, risk, and evidence.

PumpkinPi should hide orchestration, agent topology, queues, protocol details, and implementation-session mechanics unless they are needed to explain progress, safety, failure, or recovery.

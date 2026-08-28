# Intent Interpretation and Realization

## Purpose

This document defines the behavioral core of PumpkinPi: how a Project-level conversation becomes durable intent, how that intent governs situated work, and how PumpkinPi determines what has or has not been accomplished.

PumpkinPi's goal is:

> **Implement the active Source of Intent, through however many implementation-and-review iterations are required, until a reviewer can find no fault in Project reality against that Source of Intent.**

The central invariant is:

> **A Source of Intent describes the desired state of a Project. A user message is evidence about or an instruction concerning that intent. A Run is only one bounded attempt to move reality toward it. A review is an independent assessment of the whole result. None of these are interchangeable.**

PumpkinPi must not collapse broad project intent into a disposable prompt, treat a projection as canonical state, or treat one completed Run as proof that the Project satisfies its Source of Intent. It continues the realization loop until independent review returns no findings, the owner pauses or cancels it, a decision is required, or policy makes further work impossible. Resource or iteration limits may pause work; they must never be converted into success.

## Governing Model

PumpkinPi operates across four distinct layers:

```text
Owner conversation
  ↓ interpretation
Canonical Source of Intent
  ↕ comparison
Observed Project reality + evidence
  ↓ bounded action
Internal inspection / implementation / validation Runs
```

Each layer has different authority:

- The **owner conversation** supplies goals, corrections, decisions, questions, and authorization.
- The **Source of Intent** is the durable definition of what should be true. Authoritative Project documents are retained as an exact content-addressed closed bundle; generated synthesis supplements but never replaces their bytes.
- **Observed reality and evidence** describe what PumpkinPi has grounds to believe is true.
- **Runs** inspect or change reality for a declared purpose and intent revision.

Assistant prose is a human-facing projection. It is never, by itself, a Source of Intent commit, an observation, evidence, authorization, or proof of satisfaction.

## Project Intent Is Not a Task

Project intent may be broad, specific, long-lived, and only partially satisfied. It can include product behavior, architecture, constraints, non-goals, trust boundaries, quality criteria, operating policy, validation requirements, decisions, and open questions.

A task has a terminal lifecycle. Project intent does not become complete merely because a task or Run terminates.

PumpkinPi must therefore maintain separate state for:

- Source of Intent revision and status;
- current observations about Project reality;
- known divergences between intent and reality;
- user-visible operations;
- bounded internal Runs;
- evidence and validation results;
- satisfaction assessments associated with a specific intent revision.

A broad Source of Intent may govern many operations and Runs over time.

## Interpreting an Intent Chat Turn

An Intent Chat message is interpreted in Project context. It may contain one or more communicative acts:

- **clarify** — add specificity to existing intent;
- **correct** — replace an incorrect assumption or requirement;
- **decide** — resolve an open question or consequential choice;
- **reference context** — identify Project material that should inform intent, such as design documents;
- **request projection** — ask what PumpkinPi currently believes;
- **request inspection** — ask PumpkinPi to observe reality without changing it;
- **request realization** — prioritize or redirect work intended to change reality;
- **request validation** — ask whether reality satisfies some or all current intent;
- **answer** — respond to a pending PumpkinPi question;
- **redirect or cancel** — alter or stop visible work.

These acts are not mutually exclusive. Interpretation must preserve all material acts rather than reducing the message to one `execute` boolean.

### Required Context

The Intent Agent cannot interpret a turn from only the latest message and canonical payload. It requires a controlled Project context containing, as relevant:

- current Source of Intent revision, generated payload, and exact authoritative document bundle/manifest;
- initialization state;
- recent Intent Chat decisions and unresolved questions;
- active operations and their target revisions;
- current Project identity and situated execution context;
- typed inspection results or evidence relevant to the turn;
- known divergences and satisfaction assessments.

During initialization, referenced local material must be inspected by an inspection Run or trusted typed inspector and returned as observations before the Intent Agent assembles intent. An Intent Agent that is forbidden from reading Project context cannot compensate by launching an implementation Run and asking it to write a pretend Source of Intent in prose.

## Intent Maintenance Pipeline

Every Intent Chat turn follows a Spoke-controlled pipeline.

### 1. Durably acknowledge

Persist the user message and accepted operation before asynchronous interpretation. Associate both with the Source of Intent revision the user observed or targeted.

### 2. Interpret without mutating authority

The Intent Agent emits a typed proposal containing:

- interpreted communicative acts;
- a complete Source of Intent proposal when a canonical change is warranted;
- assumptions introduced or removed;
- focused questions, if required;
- a human-facing projection;
- proposed inspection, realization, or validation objectives;
- the basis for activation, pause, prioritization, or any exceptional authorization claim.

The model proposes. The Spoke validates and commits.

### 3. Obtain missing context

If intent depends on referenced Project material that has not been inspected, schedule an `inspection` Run. Inspection is read-only with respect to Project reality. Its observations return to the serialized intent-maintenance lane, which resumes the same operation and produces a new typed proposal.

Inspection output must not be presented as implementation, and it must not directly mutate canonical intent.

### 4. Commit intent atomically

When a valid Source of Intent proposal exists, the Spoke first verifies complete path/hash coverage of every authoritative document and rejects lossy, missing, changed, duplicate, or out-of-manifest material. It then:

1. checks the expected base revision and hash;
2. validates representation limits and required invariants;
3. writes the complete new payload atomically;
4. increments the revision;
5. records the previous revision or recovery backup;
6. appends an `intent_update` timeline item;
7. updates Intent Chat revision metadata;
8. reassesses active Runs for staleness.

No timeline projection or Run output may claim a revision that was not committed by this path.

### 5. Project the result to the owner

The Intent Agent explains what it understood, what changed, what remains uncertain, and whether any work is proposed or underway. This explanation is explicitly associated with the canonical revision it projects.

A brief projection is acceptable only when it accurately represents the consequence of the turn and does not hide material uncertainty. The complete canonical intent remains durable even when the user-facing explanation is concise.

### 6. Activate and realize intent

Intent assembly and intent realization remain separate state transitions, but the owner does not have to issue a task for every increment. An **active** Source of Intent is the Project's standing instruction to make reality conform to it. Activating or confirming that Source authorizes continued implementation and review within its constraints, Project trust policy, and visible execution boundary.

Rules:

- PumpkinPi does not mutate Project reality while intent is `absent`, `assembling`, `updating`, `conflicted`, or `unavailable`.
- Project initialization authorizes bounded read-only inspection needed to assemble initial intent.
- Once sufficient intent is confirmed or otherwise made `active`, PumpkinPi begins or resumes realization automatically.
- Clarifications, corrections, and decisions update the governing revision; they do not create isolated implementation tasks.
- User requests may prioritize, redirect, pause, resume, or narrow realization, but are not required to authorize every bounded increment.
- Consequential ambiguity, an unresolved owner decision, or a policy boundary blocks affected mutation and produces a question.
- The owner can pause or cancel realization explicitly. A pause is not satisfaction.

For example, when PumpkinPi asks what a newly initialized Project should achieve and the owner replies, “Use the design documents already drafted,” the expected behavior is:

1. treat the documents as referenced context for initial intent;
2. inspect them without changing Project files;
3. assemble and commit comprehensive canonical intent;
4. project that understanding and any genuine questions;
5. activate the Source of Intent when it is sufficiently established;
6. begin iterative realization against that active revision;
7. continue implementation and independent review until review finds no fault or work becomes explicitly paused, cancelled, or blocked.

## Realization Is Reconciliation

When situated work is authorized, PumpkinPi does not send the whole Source of Intent to one Run and equate the Run's response with completion. It starts or resumes a reconciliation operation against a specific intent revision.

```text
Load active intent revision
  ↓
Inspect relevant reality
  ↓
Identify evidence-backed divergence
  ↓
Choose a bounded objective
  ↓
Implement and validate
  ↓
Review the complete Project against the complete Source of Intent
  ├─ findings → record divergence → choose next bounded objective → iterate
  ├─ no findings → record reviewer approval and satisfaction
  ├─ blocked / owner decision required → pause visibly
  ├─ failed with retained evidence → recover or pause visibly
  └─ stale because intent changed → rebase on the new revision
```

### Bounded Objectives

Every implementation operation owns an isolated Git worktree and branch rooted at the primary Project HEAD. Iterations checkpoint changes there; reviewers are read-only against those checkpoints. Reviewer approval automatically fast-forwards the clean, unchanged primary checkout. Failures and cancellation roll uncommitted isolated changes back without touching primary reality.

Every implementation Run receives:

- a stable Run and parent operation ID;
- declared `implementation` purpose;
- exact Source of Intent revision;
- a bounded objective derived from a known divergence or explicit request;
- relevant constraints and situated context;
- explicit validation criteria;
- execution identity and policy;
- instructions for returning typed observations, changes, evidence, questions, and residual divergence.

A broad project definition is governing context, not itself an executable prompt.

### Inspection Before Change

PumpkinPi must have enough current evidence to choose a bounded objective safely. Existing evidence may be reused if its freshness and scope are adequate. Otherwise an inspection Run precedes implementation.

### Validation After Change

A successful process exit or fluent assistant response is not validation. Validation must produce evidence appropriate to the objective: tests, checks, diffs, observed behavior, or an explicit reason validation could not be performed.

Validation Runs are distinct when independent assessment is materially valuable. Small bounded changes may combine implementation and validation operationally, but evidence and result assessment remain typed and separate.

### Reassessment

Run results return to the orchestrator, not directly to the primary timeline as authoritative claims. The orchestrator checks:

- whether the targeted intent revision is still current;
- whether reported changes and evidence are internally consistent;
- whether validation criteria were actually evaluated;
- what divergence remains;
- whether a consequential decision is needed;
- what the next bounded increment should address.

Only then does PumpkinPi promote a user-facing incremental outcome or proceed to review.

### Independent whole-Project review

After each implementation/validation increment, and whenever reality may already conform, PumpkinPi runs a reviewer role that is independent of the implementing Run. The reviewer receives the complete current Source of Intent, relevant Project reality, prior findings, and verifiable evidence. It inspects the whole Project against the whole intent revision, not merely the latest diff or bounded objective.

The reviewer returns a typed result:

- `findings`: every discovered mismatch, omission, regression, unsupported claim, quality defect, or unfulfilled requirement;
- evidence supporting each finding;
- checks performed and relevant scope not inspected;
- `approved` only when it found no fault and no required scope remained unreviewed.

Reviewer findings become durable divergences and feed the next implementation iteration. PumpkinPi does not impose a success-oriented iteration limit. Context, cost, or policy limits may checkpoint and pause the loop, but the Project remains unsatisfied and resumes later.

The reviewer cannot make approval easier by weakening or rewriting the Source of Intent. Proposed intent changes return to Intent Chat as explicit proposals. Disputed, contradictory, or unevaluable findings trigger further inspection or an owner question rather than being silently discarded.

## Satisfaction Semantics

Satisfaction is an evidence-backed assessment, not a status inferred from operation completion.

A satisfaction assessment includes:

```text
assessment_id
project_id
source_of_intent_revision
scope: bounded_objective | requested_outcome | whole_project
state: unknown | diverged | partially_satisfied | satisfied | blocked | stale
claims
supporting_evidence_ids
known_residual_divergences
assessed_at
```

Rules:

- Whole-Project satisfaction requires a current independent whole-Project review with `approved` and zero findings.
- Reviewer approval is valid only for the exact Source of Intent revision and observed Project state it assessed.
- A bounded objective may be satisfied while substantial intent remains unrealized; its completion triggers review or another iteration, not Project success.
- A requested outcome may be satisfied only if its validation criteria were evaluated.
- Unknown, unavailable, stale, or materially unreviewed evidence cannot produce `satisfied`.
- Any material Source of Intent or Project reality change invalidates incompatible approval and resumes reconciliation.
- Prose generated by the same Run that performed work is a claim requiring corroboration, not evidence by itself.
- A reviewer that reports no findings while identifying unreviewed required scope has not approved the Project.

## Operation and Project Status Are Different

A user-visible conversational operation reaches a terminal state when that interaction has concluded. The Project's realization lifecycle remains active across as many internal implementation, validation, and review Runs as necessary. Neither a conversational operation nor a child Run determines overall Project satisfaction.

Examples:

- An intent-clarification operation can complete after revision 4 is committed while realization continues against revision 4.
- An implementation Run can complete after one validated increment while reviewer findings drive the next increment.
- A review Run can complete with findings and therefore continue realization.
- Realization can block waiting for the owner while unrelated Project work continues.
- Realization becomes satisfied only after a current whole-Project reviewer approval with no findings.

Timeline language must state scope precisely:

- Good: “Updated Source of Intent to revision 4 from the referenced design documents.”
- Good: “Implemented typed replay cursors for revision 4; tests passed; reconnect recovery remains.”
- Good: “Inspection completed; current reality diverges from revision 4 in three areas.”
- Bad: “Completed against intent revision 4” when only a Run ended.
- Bad: “Revision 4” in generated content when canonical revision 4 was never committed.

## Typed Control Boundary

The control boundary must represent intent maintenance and realization explicitly. A minimal conceptual contract is:

```text
IntentTurnProposal
  base_revision
  acts[]
  source_update optional
  assumptions[]
  question optional
  projection
  proposed_actions[]

ProposedAction
  Inspect { objective, scope, reason }
  Realize { requested_outcome, scope, validation, authorization_basis }
  Validate { claims, scope, validation }
  CancelOrSupersede { operation_ids, reason }

RunResult
  purpose
  target_revision
  objective
  observations[]
  changes[]
  evidence[]
  divergences[]
  questions[]
  objective_assessment

ReviewResult
  target_revision
  observed_reality_version
  reviewed_scope[]
  checks[]
  findings[]
  evidence[]
  unreviewed_required_scope[]
  verdict: findings | approved
```

The actual protocol may use richer enums and records, but it must preserve these distinctions. It must not use an `execute: bool` plus free-form `work_request` as the governing orchestration contract.

All model-produced values are untrusted proposals. The Spoke validates IDs, revision binding, allowed transitions, requested scope, authorization, evidence references, and policy before changing durable state or launching work.

## Concurrency and Intent Changes

Each Project has one serialized intent-maintenance lane. Inspections and bounded Runs may execute concurrently subject to policy, but all canonical Source of Intent commits serialize through that lane.

When intent changes:

1. compare the new revision with each active Run's objective and constraints;
2. continue harmless inspection if useful, preserving its target revision;
3. cancel or supersede incompatible pending work;
4. allow safe near-complete work to finish only when policy permits;
5. mark all returned claims and evidence with their actual revision;
6. never promote stale results as satisfying current intent.

Cancellation propagates from a visible operation to its pending actions, child Runs, queued Pi commands, and eventual timeline state.

## Questions and Human Supervision

PumpkinPi asks the owner when:

- intent is insufficient to choose safely;
- a consequential interpretation lacks authorization;
- reality contradicts a governing assumption;
- multiple materially different choices satisfy intent;
- local policy requires confirmation;
- evidence cannot support a requested conclusion;
- continuing realization would violate Source of Intent constraints or Project policy.

Questions should be focused and explain why the answer matters. PumpkinPi should not ask the owner to decide internal orchestration details it can safely determine itself.

## Durable Records

The Spoke-authoritative Project state must retain enough structure to reconstruct why work occurred and what it established:

- canonical Source of Intent revisions and hashes;
- Intent Chat items and decisions;
- operations and authorization basis;
- inspection observations and freshness;
- divergences;
- Run objectives and revision bindings;
- changes and evidence;
- reviewer findings, review coverage, approvals, and satisfaction assessments;
- questions, cancellations, supersession, and stale results.

Hub caches and Client projections do not become authoritative merely because the Spoke is offline.

## Failure and Recovery

If interpretation, inspection, implementation, or validation fails, PumpkinPi records the failure at the correct layer.

- Intent proposal failure leaves canonical intent unchanged.
- Inspection failure leaves observations unknown and explains why intent assembly or realization is blocked.
- Implementation failure records attempted scope and retained changes/evidence without claiming satisfaction.
- Validation failure distinguishes “validation infrastructure failed” from “Project behavior failed validation.”
- Reviewer failure or incomplete coverage cannot approve the Project; realization remains unsatisfied and retries or pauses visibly.
- Orchestrator restart reconstructs pending operations, realization phase, findings, and isolated workspace; it rolls back to the last checkpoint and resumes automatically instead of blindly repeating changes.
- Source conflict freezes canonical writes and ordinary realization until repaired.

## Required Acceptance Scenarios

### Broad existing design during initialization

Given a repository containing extensive design documents and a Project awaiting initial intent, when the owner says, “Use the design documents already drafted,” then PumpkinPi:

- inspects all relevant design material;
- commits a comprehensive Source of Intent revision;
- retains the breadth, specificity, constraints, and acceptance criteria of that material;
- presents a useful projection associated with the committed revision;
- activates sufficiently established intent and begins iterative realization within Project policy;
- never stores the proposed canonical document merely as an implementation outcome;
- does not claim completion until independent whole-Project review finds no fault.

### Intent correction during realization

A correction updates canonical intent, explains the revision, invalidates incompatible review approval, and rebases realization on the new governing state.

### Iterative realization

An active broad Source of Intent produces evidence-backed divergence, bounded implementation and validation, then independent whole-Project review. Every reviewer finding drives another iteration. The cycle continues without treating an iteration limit as success.

### Reviewer approval

Project realization reaches `satisfied` only when a reviewer inspects the complete required scope against the current complete Source of Intent and returns `approved` with no findings and no required scope left unreviewed.

### Unsupported completion claim

A Run that returns confident prose but no valid evidence cannot produce a satisfied assessment.

### Material change during work

A material intent revision causes active work to continue, cancel, or become stale through an explicit recorded decision. Its old result cannot satisfy the new revision.

### Restart between phases

Restart after intent commit, inspection, implementation, or validation resumes from durable state without losing acknowledged messages, duplicating canonical revisions, or blindly repeating changes.

## Prohibited Shortcuts

The following are product-boundary failures, even if they make a demonstration appear to work:

- supplying only the latest user message and Source payload to the Intent Agent when local context is required;
- asking an implementation Run to manufacture canonical intent;
- parsing control transitions from arbitrary assistant prose;
- representing all action as one boolean and one free-form prompt;
- marking a Project ready while its Source of Intent remains placeholder or assembling;
- using a Run's final response as evidence of filesystem changes or validation;
- treating child Run completion as satisfaction of broad Project intent;
- reviewing only the latest increment rather than the whole Project against the whole Source of Intent;
- converting an iteration, context, time, or cost limit into successful completion;
- allowing the implementer to approve its own work without independent review;
- allowing generated content to claim an uncommitted revision;
- reporting “completed against intent” without stating scope and evidence;
- mutating Project reality while intent is not active or while a required owner decision or policy boundary is unresolved.

## Implementation Gate

Intent orchestration is not ready to implement until executable contract tests can represent all of the following without interpreting free-form prose as control state:

1. context request and inspection resumption;
2. Source of Intent proposal and atomic commit;
3. activation of sufficiently established intent and automatic realization;
4. pause, resume, cancellation, and policy blocking;
5. bounded objective and validation criteria;
6. typed observations, evidence, divergence, and satisfaction assessment;
7. independent whole-Project review, findings, and approval;
8. repeated implementation-review iterations with no false success at resource limits;
9. question/block/resume and supersession;
10. stale-revision and stale-approval handling;
11. restart recovery between every phase.

A deterministic fake agent and fake Pi process should prove these state transitions before real model behavior is used as evidence that the orchestration works.

# Overview

## Product Thesis

> **PumpkinPie brings the projects and LLM capabilities of all your computers into one personal Hub, while keeping execution grounded on Slices where the real context lives.**

**Unified at the Hub, situated on the Slice, directed by intent.**

PumpkinPie gives one person a coherent place to maintain Project intent and have native agents realize it inside the real local contexts where Projects live. **Slice** is the situated agent: a standalone Rust TUI/CLI and an enrollable execution endpoint. The Hub is valuable but optional for local Slice use.

PumpkinPie is not primarily a remote shell, worker dashboard, chat wrapper, or CI host. Its primary product object is a Project's **Intent Chat**, backed by a durable **Source of Intent** and repeated evidence-backed reconciliation of Project reality.

## Components

### PumpkinPie Hub

The personal Hub provides one authenticated control plane across Slices:

- Slice enrollment, authentication, disable/revoke, and inventory cache;
- provider-account custody and capability delivery;
- multiplexed Project/Intent subscriptions;
- routing, replay, stale/offline projections, and redacted audit;
- no direct Project filesystem authority.

### Slice

Slice is one native Rust executable with selectable roles:

- `slice [PATH]`: standalone TUI coding agent with no Hub requirement;
- `slice serve`: local situated execution authority, service/local IPC, and enrolled Hub endpoint;
- `slice gui`: graphical PumpkinPie Hub client;
- headless CLI turns and local realization commands.

Serve/TUI share the native provider/runtime/tool implementation without becoming competing authorities. GUI shares protocol/domain packaging but remains a non-authoritative Hub view/controller. A person can use any combination on one or several machines. A serve endpoint can host many Projects and retains local authority offline or unenrolled.

### Project

A Project binds a stable identity to a canonical local path, Slice, Source of Intent, Intent Chat, execution identity, trust/policy, provider/model defaults, repository/worktree metadata, and durable history.

## Core Loop

```text
Owner clarifies intent in Intent Chat
  -> Slice validates and commits Source of Intent
  -> inspect current situated reality
  -> select a coherent bounded objective
  -> implement in an isolated worktree
  -> execute deterministic validation and capture evidence
  -> independent whole-Project review against complete intent
       findings -> durable divergence -> next iteration
       no findings and complete scope -> cold approval -> promote
       question/policy/failure/stale -> pause visibly
```

Active intent is standing authorization within Project trust and local policy. The loop continues for however many iterations are needed. Resource limits may pause but never imply success.

Interactive `slice` coding turns use the same runtime and tools under a visible direct-checkout policy, but bounded interactive completion is not whole-Project satisfaction.

## Rust-Only Native Runtime

PumpkinPie application and test logic is Rust. Slice owns provider HTTP streaming, tool calls, context, interactions, persistence, and recovery. It does not launch or embed an external coding agent, Node.js runtime, JavaScript/TypeScript/Python sidecar, or provider CLI.

External Project/system tools such as Git, Bubblewrap, Cargo, compilers, tests, and `rg` may run under Slice supervision. Provider credentials remain in Slice's native provider client and are excluded from Project tool processes.

See [Native Rust Agent Runtime](17-native-runtime.md).

## Local and Connected Use

```text
slice .
  -> local Project and Session authority
  -> optionally: slice enroll --hub ...
  -> the same Projects appear through `slice gui`
  -> local TUI, GUI, and other authorized GUI instances observe one event history
```

Enrollment does not migrate authority to the Hub, duplicate Sessions, or weaken local path trust. Closing `slice gui` does not stop accepted work. Hub disconnect leaves local use available and cached remote views explicitly stale.

## Trust Boundary

This is initially a personal single-owner system. Administrative access to the Hub controls enrolled PumpkinPie capabilities, but every consequential operation remains situated and legible:

- Slice/machine;
- canonical Project path and worktree;
- effective user/root state;
- provider/model/account reference;
- writable surfaces, network/tool capability, and risk;
- Source revision and operation.

Slice enforces trusted roots, symlink containment, execution identity, sandbox policy, provider isolation, and root restrictions locally. Hub trust cannot override local policy silently.

## Product Constraints

- Hub routing never becomes Project filesystem authority.
- Slice remains independently useful without Hub connectivity.
- Source of Intent is distinct from conversation, projections, Run output, and evidence.
- Model output is an untrusted proposal.
- Tool results become evidence only through Slice capture.
- Implementers cannot approve their own work.
- Review is whole-Project after every implementation increment.
- Final approval uses a cold independent Session.
- Normal product surfaces hide internal orchestration unless needed for trust, progress, or diagnostics.
- Stable IDs and durable cursors govern identity; names remain user-friendly projections.
- No permanent legacy runtime, protocol, executable, state path, or compatibility API remains after the prerelease migration.

## Quality Bar

PumpkinPie should feel like delegated work that remains understandable:

- immediate durable acknowledgement;
- visible intent/revision and situated context;
- useful progress without raw event noise;
- precise scope and evidence language;
- reconnect/replay without duplication;
- explicit offline, stale, blocked, conflict, crash, and recovery states;
- native local responsiveness in Slice TUI;
- no requirement to install a language runtime or external coding-agent framework.

# CLI

One user-facing Rust tool, `slice`, selects the desired role.

## Standalone TUI and Local Agent

```bash
slice [PATH]
slice run [PATH] [--model MODEL] [--json] PROMPT
slice realize [PATH]
slice auth login PROVIDER
slice auth set-key PROVIDER
slice auth list
slice project init/list/status/remove
slice session list/resume/archive/export
slice evidence list/show/export
slice doctor
slice reset --yes
```

`slice [PATH]` is the primary standalone terminal coding-agent experience. It requires no Hub. `slice realize` uses full Source-of-Intent reconciliation.

## GUI Client

```bash
slice gui [--hub URL]
```

`slice gui` launches the native graphical PumpkinPie Client and connects with owner-control credentials. GUI state/credentials remain distinct from endpoint identity even when `slice serve` runs on the same machine.

## Serve Endpoint

```bash
slice serve [--hub URL]
slice enroll --hub URL --setup-key KEY
slice unenroll
slice serve doctor
```

`slice serve` owns local situated execution authority, background recovery, local IPC, and enrolled endpoint connectivity. TUI may explicitly attach to it for local Projects.

## Hub Administration

```bash
pumpkinpie-hub serve --listen ADDRESS --public-url URL
pumpkinpie-hub slice create/list/disable/revoke/rotate-key
pumpkinpie-hub provider set/list/revoke
pumpkinpie-hub owner issue/revoke/list
```

Remote owner administration may also be exposed inside `slice gui`; behavior/contracts remain shared and typed. There is no separate `pumpkinpie` Client binary.

## Output and Safety

Human output is default; `--json` uses versioned product schemas, never raw provider payloads. Commands identify role, target Slice, Project path, effective identity, provider/model/account reference, Source revision, and risk before consequential action.

Secrets use hidden prompt, browser/device flow, protected descriptor, explicit development environment reference, or platform credential service—not positional argv.

## Naming and Compatibility

Final executables are `slice` and `pumpkinpie-hub`. Legacy names/environment variables and the separate Client binary/crate are removed after one-time state migration, not retained as aliases.

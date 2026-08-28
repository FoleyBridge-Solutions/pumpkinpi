# CLI

The CLI should preserve the same product model as the GUI: Projects are primarily interacted with through Intent Chat. Internal Session commands belong under diagnostics/development surfaces rather than the normal workflow.

## Command-Line Shape

```bash
# Hub administration
pumpkinpi hub serve --listen 0.0.0.0:8080
pumpkinpi hub spoke create --name framework
pumpkinpi hub spoke list
pumpkinpi hub spoke revoke spoke_abc
pumpkinpi hub spoke issue-setup-key spoke_abc

# Spoke
pumpkinpi spoke enroll --hub https://hub.example.com --setup-key pps_setup_x
pumpkinpi spoke serve
pumpkinpi spoke serve --hub https://hub.example.com

# Project / Intent Chat
pumpkinpi project init --spoke framework /home/me/app
pumpkinpi project list
pumpkinpi project status proj_app
pumpkinpi chat proj_app
pumpkinpi intent send proj_app "The CLI should support JSON output"
pumpkinpi intent summarize proj_app

# Secondary evidence and recovery surfaces
pumpkinpi project evidence proj_app
pumpkinpi project diagnostics proj_app

# Optional local-only development mode
pumpkinpi spoke serve --local-only --listen 127.0.0.1:4242
```

`pumpkinpi chat` is the primary interactive CLI experience. It renders human-readable LLM projections; it should not dump canonical Source of Intent storage or expose raw Pi RPC by default.

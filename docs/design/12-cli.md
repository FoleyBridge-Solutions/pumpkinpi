# Cli

## Command-Line Shape

Recommended commands:

```bash
# Hub
pumpkinpi hub serve --listen 0.0.0.0:8080
pumpkinpi hub node create --name framework
pumpkinpi hub node list
pumpkinpi hub node revoke node_abc
pumpkinpi hub node issue-setup-key node_abc

# Node
pumpkinpi node enroll --hub https://hub.example.com --setup-key ppn_setup_x
pumpkinpi node serve
pumpkinpi node serve --hub https://hub.example.com

# Optional local-only development mode
pumpkinpi node serve --local-only --listen 127.0.0.1:4242
```


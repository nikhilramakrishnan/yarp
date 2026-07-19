# yarp (Go)

The local-first rewrite of yarp: a fast terminal client in **one static,
cross-platform binary**. No accounts, no cloud, no telemetry — your shell,
your terminal, your data in `~/.yarp/`.

```
cd go
CGO_ENABLED=0 go build ./cmd/yarp
./yarp
```

- **Blocks** — every command becomes a searchable block (command, cwd,
  exit code, output), recorded via OSC 133 shell integration for bash, zsh,
  fish, and PowerShell.
- **ctrl-g overlay** — fuzzy palette over history + workflows + actions,
  block browser, theme picker, and a local-AI pane. Selections are *typed*
  at your prompt; yarp never runs a command for you.
- **Local LLMs only** — auto-discovers Ollama / LM Studio / llama.cpp on
  localhost (or `YARP_LLM_BASE_URL`). No API keys, anywhere. Persistent
  memory lives in a plain `~/.yarp/memory.jsonl`.
- **Personal backups** — `yarp backup` snapshots `~/.yarp` to a directory
  you choose, your own Google Drive (bring-your-own OAuth client,
  `yarp backup gdrive-auth`), or any rclone remote.
- **Warp-compatible assets** — existing Warp/yarp workflow YAML and theme
  YAML files work unchanged in `~/.yarp/workflows` and `~/.yarp/themes`.

Run `yarp doctor` to check your setup, `yarp help` for the full CLI.

Testing: `go test ./...` for units; `./e2e/run.sh` drives the real binary
on a pty through bash, zsh, fish, and pwsh and asserts blocks are recorded.

Design and rationale: see `specs/go-rewrite/PRODUCT.md` and `TECH.md`.

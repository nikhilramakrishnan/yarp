<h1 align="center">Yarp</h1>

<p align="center">
  <em>"Yarp."</em><br/>
  — Michael "Lurch" Armstrong, <em>Hot Fuzz</em>
</p>

<p align="center">
  Not everything is as it seems in Sandford.
</p>

---

## What this is

A fast, local-first terminal client in **one static, cross-platform Go
binary**. No accounts, no cloud, no telemetry — your shell, your terminal,
your data in `~/.yarp/`.

yarp runs your shell on a pty inside the terminal you already use and adds
the good ideas on top:

- **Blocks** — every command becomes a searchable block (command, cwd, exit
  code, output), recorded via OSC 133 shell integration for bash, zsh, fish,
  and PowerShell.
- **ctrl-g overlay** — fuzzy palette over history + workflows + actions, a
  block browser, a theme picker, and a local-AI pane. Selections are *typed*
  at your prompt; yarp never runs a command for you.
- **Local LLMs only** — auto-discovers Ollama / LM Studio / llama.cpp on
  localhost (or `YARP_LLM_BASE_URL`). No API keys, anywhere. Persistent
  memory lives in a plain `~/.yarp/memory.jsonl`.
- **Personal backups** — `yarp backup` snapshots `~/.yarp` to a directory
  you choose, your own Google Drive (bring-your-own OAuth client,
  `yarp backup gdrive-auth`), or any rclone remote.
- **Warp-compatible assets** — existing Warp/yarp workflow YAML and theme
  YAML files work unchanged in `~/.yarp/workflows` and `~/.yarp/themes`.

## Build

```bash
CGO_ENABLED=0 go build ./cmd/yarp   # one static binary, any GOOS/GOARCH
./yarp                              # start a session (overlay on ctrl-g)
yarp doctor                         # check shells, model runtimes, data dir
```

## Test

```bash
go test ./...      # unit suites
./e2e/run.sh       # drives the real binary on a pty through bash/zsh/fish/pwsh
```

## History

This began as a personal, unaffiliated fork of Warp's Rust codebase and was
rewritten from scratch in Go as a single local-first binary. The original
Rust tree was removed once the rewrite was verified; it lives on in git
history, and `specs/` keeps the record of everything built along the way
(see `specs/go-rewrite/` for what was kept, replaced, and dropped).

## Status

Unstable and personal, but you can use it if you like it.

## Acknowledgements

We love Warp and what they've done for the developer experience. We stand on
the shoulders of giants.

To Edgar Wright, Simon Pegg, and Nick Frost — for *Hot Fuzz* (2007).

> "PUB?"

# PRODUCT.md — yarp, rewritten as a single Go binary

## One-liner

yarp becomes a free, local-first Warp: one static, cross-platform Go binary
that makes your existing terminal block-aware, searchable, themeable, and
quietly assisted by a local LLM — with zero accounts, zero cloud, zero
telemetry.

## Why

The Rust codebase is a full GPU-rendered terminal with a hosted backend woven
through it (auth, telemetry, object sync, teams, cloud agents). The product we
actually want is much smaller: a superb terminal experience for one local
user. A ground-up Go rewrite gets us:

- **One binary, every platform.** `CGO_ENABLED=0 go build` produces a static
  ~10 MB executable for Linux/macOS/Windows (amd64/arm64). No installer, no
  runtime, no GPU stack.
- **Speed and simplicity.** yarp does not re-render the terminal. It runs your
  shell on a pty and tees the bytes through untouched — your native terminal
  (iTerm2, Windows Terminal, kitty, GNOME Terminal…) does what it is already
  great at. yarp adds the Warp ideas on top: blocks, palette, AI, themes.
- **Total decoupling.** Nothing phones home. There is nothing to log into.

## What we keep from the prior work (and where it went)

Prior yarp work is inventoried across `specs/` and `YARP.md`. Disposition:

| Prior work | Disposition in the rewrite |
|---|---|
| Local-first architecture (YARP.md): `~/.yarp/` data dir, `OssAiClient`, `OssObjectClient` stub, env-configured LLM provider | **Kept and completed.** `~/.yarp/` is the entire universe of user data. The `YARP_LLM_BASE_URL` / `YARP_LLM_MODEL` env contract carries over verbatim. |
| Blocks / output capture (terminal bootstrap, block list) | **Kept**, re-based on the open OSC 133 standard instead of the private OSC 9277/9278 JSON protocol. |
| Workflows (Warp YAML format, `{{arg}}` placeholders) | **Kept.** Same YAML schema; existing files drop into `~/.yarp/workflows` unchanged. |
| Themes (Warp YAML schema) | **Kept.** Same schema in `~/.yarp/themes`; applied to the host terminal via OSC 4/10/11/12. Warp's public theme repo remains compatible. |
| Command palette + fuzzy matching (`fuzzy_match` crate, command_palette) | **Kept**, as the ctrl-g overlay. |
| History persistence (SQLite/diesel) | **Replaced** with per-session JSONL under `~/.yarp/history` — greppable, diffable, trivially backed up. |
| AI assistant (`ai_assistant`, NL→command) | **Kept in spirit**: one session-aware assistant on a local model, plus persistent memory. |
| `YARP_SHELL_PATH` dev affordance | **Kept.** |
| Cloud agents / agent management (`agent_management`, `cloud_agent_config`, `ambient_agents`, `agent_sdk`, agent-council, REMOTE-*, cloud-mode-* specs) | **Removed.** yarp manages no agents. |
| Auth/accounts, telemetry (Rudder), billing/teams/referrals, shared sessions, hosted Drive/object sync, GraphQL/Firebase/websocket backends | **Removed.** |
| Hosted AI providers with API keys (`api_keys.rs`, Anthropic/OpenAI paths) | **Removed.** Local runtimes only; no API-key surface exists. |
| Drive (hosted, shared object store) | **Replaced** by personal backup connectors (below). |

## The product

### 1. A very good terminal, first

`yarp` launches your shell (bash, zsh, fish, pwsh — else plain passthrough)
inside your terminal. Everything behaves exactly as before: prompts, vim,
ssh, ctrl-c, resize, colors. Shell integration is injected automatically and
records **blocks**: command, cwd, start/end time, exit code, and
ANSI-stripped output (capped, default 256 KB).

### 2. One overlay, one hotkey

`ctrl-g` (configurable) opens the only piece of chrome yarp has, drawn on the
alternate screen of your own terminal:

- **Palette** — fuzzy search across history commands, workflows, and actions.
  Enter *types* the selection at your prompt; yarp never auto-executes.
- **Blocks** — browse recent commands, view full output, insert or copy
  (OSC 52) a command.
- **Ask yarp** — chat with a local model that sees your recent blocks and cwd.
  Proposed commands are numbered for one-key insertion. `/remember <fact>`
  (or the model's own `REMEMBER:` directive) writes to persistent memory.
- **Themes** — apply/persist a theme; live via OSC.
- **Back up now** — one-keystroke snapshot.

Esc closes; the terminal is exactly as you left it.

### 3. Local LLM support, no keys

yarp auto-discovers OpenAI-compatible runtimes on localhost (Ollama :11434,
LM Studio :1234, llama.cpp :8080) or uses `llm.endpoint` / `YARP_LLM_BASE_URL`.
There is deliberately no field anywhere for a hosted API key. If no runtime is
found, AI features degrade to a setup hint; everything else works.

Memory is a human-readable `~/.yarp/memory.jsonl` the user can edit or delete.

### 4. Personally backable storage

All state lives in `~/.yarp/` (settings.json, history/, themes/, workflows/,
memory.jsonl). `yarp backup` snapshots it to tar.gz and ships it to targets
the *user* owns:

- a local/removable/NAS directory (`backup.dir`),
- **Google Drive** via the user's own OAuth client + device flow
  (`yarp backup gdrive-auth`) — yarp ships no credentials, scope is
  `drive.file` only,
- any **rclone** remote (`backup.rclone_remote`) when rclone is installed.

`yarp backup restore <archive>` round-trips.

### 5. CLI surface

`yarp` · `history [q]` · `ai <q>` · `memory list|add|clear` · `backup …` ·
`themes [set]` · `workflows` · `init <shell>` · `doctor` · `version`.

## Non-goals

- No GPU rendering, tabs, panes, or windowing — the host terminal and tmux
  already do this well.
- No agent orchestration, autonomy modes, or tool-running AI. The assistant
  suggests; the user executes.
- No sync protocol. Backup is a file you own.
- No plugins/MCP in v1.

## Validation

- Unit suites per package (vt scanner, blocks store, fuzzy, workflows,
  themes, config, memory, backup, key decoding).
- End-to-end: drive the binary under a pty (`script -qc`), assert blocks are
  recorded with correct cmd/cwd/exit/output; overlay opens/closes cleanly;
  trailing input still reaches the shell.
- AI: stub OpenAI-compatible SSE server; assert discovery, streaming,
  REMEMBER persistence.
- Cross-compile check: linux/amd64, windows/amd64, darwin/arm64.

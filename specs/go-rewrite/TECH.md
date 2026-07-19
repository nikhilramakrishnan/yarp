# TECH.md — yarp Go rewrite

## Architecture in one paragraph

yarp is a **pty tee**, not a terminal emulator. It puts the user's terminal
in raw mode, runs the shell on a pseudo-terminal, and copies bytes both ways.
On the way through, a streaming VT scanner extracts shell-integration events
(OSC 133 block boundaries, OSC 7 cwd, OSC 6973 command text) and accumulates
an ANSI-stripped copy of output for persistence. All chrome is a single
alternate-screen overlay toggled by a hotkey. This is what makes a full
rewrite tractable, fast (no rendering pipeline), and portable (rendering is
the host terminal's problem).

## Module map (repo root)

```
cmd/yarp            CLI dispatch (run, history, ai, memory, backup, themes,
                    workflows, init, doctor, version)
internal/config     ~/.yarp dir + settings.json (defaults, atomic save)
internal/termio     Pty interface; creack/pty (unix) / ConPTY (windows)
internal/shellhook  per-shell integration snippets + injection (bash --rcfile,
                    zsh ZDOTDIR shim, fish -C, pwsh -Command)
internal/vt         streaming OSC/CSI scanner, event extraction, ANSI strip
internal/blocks     Block model, JSONL session store, load/search/prune
internal/fuzzy      palette scorer (replaces fuzzy_match crate)
internal/ui         session loop, key decoder, overlay (palette/blocks/AI/
                    themes/workflow-args), raw-ANSI renderer
internal/workflows  Warp workflow YAML (schema-compatible)
internal/themes     Warp theme YAML → OSC 4/10/11/12; embedded defaults
internal/llm        OpenAI-compatible client, localhost discovery, SSE
                    streaming, memory.jsonl store
internal/agent      the one agent: context assembly, REMEMBER directive,
                    command suggestions (insert-only)
internal/backup     tar.gz snapshot/restore/prune; local dir, Google Drive
                    (device flow, user's own OAuth client), rclone
```

Dependencies (all small, no cgo): `creack/pty`, `UserExistsError/conpty`,
`golang.org/x/term`, `gopkg.in/yaml.v3`.

## Key decisions

1. **OSC 133 over the private protocol.** The Rust bootstrap spoke
   JSON-over-DCS (OSC 9277/9278/9279, hex-encoded payloads, bash-preexec
   vendored). The Go hooks emit the FinalTerm/OSC 133 standard (`A` prompt,
   `C` pre-output, `D;exit`) plus OSC 7 (cwd) and one private sequence,
   `OSC 6973;cmd;<base64>`, for the exact command line (OSC 133 doesn't carry
   it; base64 dodges all quoting). The scanner also understands VS Code's
   `OSC 633;E`, so environments already emitting it get command text for free.
   Sequences pass through to the host terminal, which ignores or exploits
   them — blocks even work over ssh by `yarp init <shell>` on the remote.

2. **Concurrency: channels in, one loop owns state.** Two reader goroutines
   (pty, stdin) send owned chunks into a select loop that owns every mutable
   structure — no locks. AI calls run in a goroutine and report back via
   delta/done channels. Resize is a 400 ms poll of `term.GetSize` (works on
   all three OSes; no SIGWINCH portability code).

3. **Overlay on the alternate screen.** Opening the overlay withholds child
   output in a buffer (still scanned, so blocks complete under the overlay)
   and replays it on close. If output arrived while covered, yarp double-
   resizes the pty to nudge full-screen apps to repaint. Rendering is a full
   repaint per keystroke of ~rows strings — trivial at terminal sizes.

4. **Insert, never execute.** Every path that hands the user a command
   (palette, workflows, blocks, AI suggestions) writes it to the pty
   *without* a newline, with embedded newlines flattened. The user always
   presses Enter.

5. **JSONL over SQLite.** Session files under `~/.yarp/history/` named
   `YYYYMMDD-HHMMSS-<pid>.jsonl`, one block per line. Sorted filenames give
   chronology; corrupt lines/files are skipped, never fatal; retention is
   mtime-based pruning (`history_days`, default 90). Removes the
   diesel/migration machinery outright.

6. **LLM = OpenAI-compatible only, localhost by default.** Discovery probes
   `/v1/models` on Ollama/LM Studio/llama.cpp ports (1.5 s timeout each);
   `llm.endpoint` or `YARP_LLM_BASE_URL` overrides (the env names are the
   Rust `local_backend` contract). Streaming is plain SSE parsing. Memory
   recall is keyword-overlap + recency over `memory.jsonl` — deliberately not
   a vector DB.

7. **Google Drive without shipping credentials.** The user creates their own
   OAuth client (TV/limited-input type), pastes id+secret once
   (`yarp backup gdrive-auth`); yarp runs the device flow, stores the refresh
   token in settings.json, uploads with scope `drive.file` via multipart
   REST. stdlib HTTP only.

8. **Windows.** `termio` build-tags ConPTY; pwsh integration reports the
   command line from history at the next prompt (PowerShell has no preexec),
   which still yields correctly labeled blocks. Raw mode via `x/term` works
   on Windows 10+ terminals.

## Behavioral details worth knowing

- Capture folds `\r` → `\n` (progress bars become lines) and swallows the LF
  of CRLF; output is trimmed of trailing newlines and capped
  (`block_output_limit_kb`, default 256) with a `truncated` flag. Only 7-bit
  escape forms are recognized: bytes 0x9c/0x9d are UTF-8 continuation bytes
  in real output, never C1 controls.
- The bash DEBUG-trap preexec is gated the way bash-preexec does it: an
  at-prompt flag armed by the last PROMPT_COMMAND entry and cleared by the
  first, so neither yarp's own hooks nor the user's PROMPT_COMMAND (starship,
  direnv, `history -a`) are ever recorded as commands.
- The palette hotkey only fires on a byte in ground state — a small stdin
  sequence tracker ignores BEL/ESC inside terminal query replies and
  bracketed pastes. In the overlay, unknown CSI (paste markers, focus
  events) decode to an inert key, never Esc; a lone ESC is disambiguated
  from a split sequence by a 60 ms timer; and when the overlay closes
  mid-batch, remaining input is forwarded to the pty as raw bytes.
- pwsh reports command + exit code from history at the next prompt (deduped
  by history Id); the session records these as output-less blocks since
  PowerShell has no preexec hook to open a capture window.
- Inserted text is sanitized of every control character (CR would execute);
  AI deltas/results carry a generation counter so a cancelled request can
  never write into a newer question's transcript; backups run off-loop.
- A prompt arriving while a command is open (hooks half-installed, shell
  crashed) drops the dangling capture instead of mislabeling it.
- Unknown shells run with no hooks: yarp degrades to a plain passthrough
  terminal with palette/AI/themes still available.
- `settings.json` is written atomically (tmp + rename); partial files load
  with defaults filled.

## Testing

`go test ./...` covers: scanner state machine (split chunks, ESC-\ terminator,
capture cap, 633 unescape, OSC 7), block store round-trip/dedup/prune, fuzzy
ranking, workflow/theme schema compatibility (fixtures copied verbatim from
the Rust integration tests), config defaults/round-trip, memory recall,
snapshot/restore/prune (including exclusion of backups/runtime and
path-escape rejection), and key decoding (split sequences, UTF-8).

E2E: `e2e/run.sh` drives the real binary on a pty through **bash, zsh,
fish, and pwsh** (installed locally) and asserts blocks are recorded with
correct labels and exit codes. The driver (`e2e/driver.py`) behaves like a
real terminal — it answers DSR/DA queries, which PSReadLine requires — so
the test is faithful where plain piped stdin is not. Verified end-to-end:
the bash PROMPT_COMMAND gating, the zsh .zshenv shim and PROMPT_SP output
cleanup, fish events, and pwsh history-based reporting (hook-sourcing and
empty prompts excluded). The AI path is exercised against a stub
OpenAI-compatible SSE server. Cross-compile gates: `GOOS=windows`,
`GOOS=darwin` builds.

Remaining untested-at-runtime surface: Windows ConPTY itself (this rewrite
was validated on Linux; pwsh hook logic is shell-side and shared).

## Build & release

```
CGO_ENABLED=0 go build -ldflags "-s -w -X main.version=$(git describe)" ./cmd/yarp
```

Release = the same command in a matrix over GOOS/GOARCH. No packaging step,
no assets to bundle (themes ship in code; fonts/SVGs of the old Rust app are
not needed by a terminal-native client).

## Legacy code removal

Once the rewrite was verified end-to-end across all four shells, the Rust
tree (app/, crates/, build configs, Rust-bound agent skills) was deleted and
the Go module hoisted to the repo root. The Rust code remains recoverable
from git history; `specs/` stays as the record of what it did.

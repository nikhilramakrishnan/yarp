# Agent Council — smoke test plan

End-to-end validation for `/agent <prompt>` after the rich-content
integration lands. Run against a debug build of yarp.

## Preconditions

- At least one of `claude`, `codex`, `gemini` resolvable via
  `~/.local/bin` or `zsh -ilc 'command -v <bin>'`.
- `~/.yarp/personas.json` either absent (falls back to
  `Roster::default_sandford`) or contains a default team with at least
  one CLI-backed persona.
- `cargo build -p yarp` is clean.

## Golden path

1. Launch yarp.
2. Open a terminal tab in any cwd.
3. Type `/agent how should we ship this?` (or any short prompt).
4. **Expect within ~1s:** A new rich-content block appears in the
   active terminal's block list immediately after the slash-command
   block, titled "Sandford NWA on the case: how should we ship this?"
   with one card per detected CLI persona.
5. **Expect each card to transition through phases:**
   - "starting" briefly while child process is spawning.
   - "thinking…" while reasoning chunks stream in (claude only — codex
     and gemini skip this).
   - "streaming…" while output text streams in.
   - "done" when the CLI's `result` event fires.
6. **Expect output to be markdown-rendered** — `**bold**` shows bold,
   `` `code` `` shows monospaced inline, lists render with bullets.
7. **Expect a Verdict card** at the bottom after every peer reaches
   "done", containing the lead's synthesised take.

## Failure-mode checks

- **Spawn failure** (e.g. rename a CLI binary out of PATH while persona
  is configured): card shows "failed: spawn failed: …". Other cards
  still proceed.
- **CLI exits non-zero** (force a syntax error): card shows
  "failed: exit N". Verdict still runs over surviving peers' takes.
- **Cancel-while-streaming**: scroll away or close the tab mid-stream.
  Children should die (kill_on_drop). Use `pgrep claude` to confirm
  no orphans within ~1s.
- **No CLI personas**: rename all CLI binaries out of PATH, restart
  yarp so detection re-runs, then `/agent foo`. Should fall through
  to the existing `EnterAgentView` (full-screen agent UI), not the
  council view.

## Negative checks

- Open a fresh tab, do **not** run `/agent`. Expect no council blocks
  in the block list.
- Run `/agent` with **no prompt** (just `/agent`). Should route to
  `EnterAgentView` (single-detective flow), not the council.

## Performance sanity

- Drain timer fires at 50ms. With 3 CLIs each emitting ~10 chunks/sec,
  drain handler should take <1ms per tick. If the UI feels janky,
  profile with `cargo flamegraph` or yarp's built-in profiler.

## Known gaps (not failure modes)

- No collapsible Thinking pane — the full thinking buffer renders inline.
- No timing display — `elapsed()` was removed during cleanup.
- No tool-use rich rendering — tool calls show as terse `• name: input` lines.
- Multiple concurrent councils stack; no cap on how many run at once.

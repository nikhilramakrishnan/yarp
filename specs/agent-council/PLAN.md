# Agent Council — native chat UI plan

## Goal

`/agent <prompt>` should render the multi-agent fan-out as a native Yarp UI
surface with explicit **thinking** and **output** phases per persona, plus a
final **verdict** card — instead of dumping a bash subprocess into a terminal
block.

Two pillars:

1. **Custom council view** — own the surface; do not impersonate
   `BlocklistAIHistoryModel` / `AIBlock`.
2. **Structured CLI streams** — parse each persona's JSON event stream
   (`claude -p --output-format stream-json --verbose`, `codex exec --json`,
   `gemini` with its JSON mode) instead of raw TTY.

## Layout

New module: `app/src/agent_council/`.

| File              | Responsibility                                                                                       |
|-------------------|------------------------------------------------------------------------------------------------------|
| `mod.rs`          | Module entry. Re-exports controller + view. No logic.                                                |
| `state.rs`        | `CouncilState`, `PersonaCard`, `CardPhase` (`Pending`, `Thinking`, `Streaming`, `Done(reason)`).     |
| `event_stream.rs` | Per-CLI JSON event parsing. Maps each line of stdout → `CouncilEvent`. Pure function, no I/O.        |
| `controller.rs`   | `CouncilController` — owns `CouncilState`, spawns one child per persona, drives state on each event. |
| `view.rs`         | `CouncilView` — yarpui `View` rendering the cards stacked vertically.                                |

## Data flow

```
slash_commands → Event::EnterAgentCouncil { prompt }
              → terminal/view.rs: open CouncilView in pane
              → CouncilController::start(prompt)
                  for each persona with binary:
                      spawn(command::r#async::Command, --json mode)
                      pipe stdout → BufReader::lines()
                      task: for each line → event_stream::parse(persona, line) → ctx.update(...)
              → on all done: synthesise via lead persona (same pipeline)
              → render: CouncilView reads CouncilState, renders one PersonaCard per persona
```

State updates use `ModelHandle<CouncilController>::update`; each card is
streamed live (each text delta appends to the card's output buffer).

## Event stream parsing

Each CLI emits JSON lines. We normalise to a small enum:

```rust
enum CouncilEvent {
    ThinkingStarted,
    ThinkingDelta(String),
    ThinkingEnded,
    OutputStarted,
    OutputDelta(String),
    OutputEnded,
    ToolCall { name: String, summary: String },   // optional, render compact
    Finished { ok: bool, reason: Option<String> },
}
```

Per-CLI event mapping:

- **claude (`-p --output-format stream-json --verbose`)**:
  - `{"type":"system","subtype":"init",...}` → ignore.
  - `{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"..."},...]}}`
    → `ThinkingDelta` per chunk.
  - `{"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}`
    → `OutputDelta`.
  - `{"type":"assistant","message":{"content":[{"type":"tool_use",...}]}}`
    → `ToolCall { ... }`.
  - `{"type":"result", "subtype":"success"|...}` → `Finished`.
- **codex (`exec --json`)**:
  - Codex emits `{"type":"item.delta","item":{"role":"assistant","content":...}}`
    style events. Treat any `assistant` text delta as `OutputDelta`. Codex
    doesn't expose thinking by default — fold any reasoning trace into output
    or skip.
  - Final `{"type":"task.completed"}` → `Finished`.
- **gemini**: simplest fallback — line-buffer plain stdout into `OutputDelta`s
  and emit `Finished` on EOF. Upgrade to JSON later if/when Gemini's CLI
  exposes one.

Skill-level invariant: parser is **infallible** — any unrecognised line yields
`OutputDelta(line)` so we never lose user-visible content.

## View

Each persona = one card:

```
┌────────────────────────────────────────────┐
│ [icon] Sgt Angel (claude)         ▌ Thinking │ ← collapsible thinking pane
│   reasoning text here...                     │
├────────────────────────────────────────────┤
│ Output                                       │
│   streaming markdown text...                 │
└────────────────────────────────────────────┘
```

Cards stack vertically. Final synthesis card at the bottom rendered after all
peers reach `Done`. Thinking pane is collapsible (default collapsed once card
finishes; expanded while streaming).

Borrow visual idioms from `child_agent_status_card.rs` (status icon by phase,
dismiss button, click-to-expand) and from agent_view's streaming markdown
rendering.

## Wiring

1. `app/src/personas.rs` — add `cli_invocations()` that returns
   `Vec<(Persona, ProgramAndArgs)>` for live CLI personas in **JSON mode**
   (e.g. `claude` → `["-p","--output-format","stream-json","--verbose"]`).
   Replace `oneshot_args_for` callers used by `build_council_command`. Keep
   `build_council_command` only as the no-UI fallback path or delete it once
   the council view is the default.

2. `app/src/terminal/input.rs` — add `Event::EnterAgentCouncil { prompt: String }`.

3. `app/src/terminal/view.rs` — handle `InputEvent::EnterAgentCouncil` by
   opening `CouncilView` in the active pane (pane-level overlay or a new
   block-list slot — TBD; lean toward "modal-like overlay over the terminal
   view", same pattern that ssh/agent flows use).

4. `app/src/terminal/input/slash_commands/mod.rs:385-393` — replace the
   `try_execute_command(&council_cmd, ctx)` branch with
   `ctx.emit(Event::EnterAgentCouncil { prompt })`. Keep `build_council_command`
   as a degraded fallback only if no pane is available, or drop it.

## Process spawning

Use `command::r#async::Command::new_with_process_group(bin)` so the council
view's "cancel" can kill the whole tree. `Stdio::piped()` for stdout +
stderr. Spawn one Yarp foreground task per persona that reads lines and
forwards them to the controller via a channel or direct
`controller.update_in(ctx, ...)`.

Cancellation: dropping the controller kills children via `child.kill()`.

## Open questions

- **Pane vs overlay**: does the council view replace the terminal block list
  for the active terminal, or open as a modal pane stack? Default: pane stack
  (matches `EnterAgentView`).
- **Persistence**: should council runs survive restart? First cut: no.
- **Synthesis prompt**: today it's a string template in
  `build_council_command`. Move it into `controller.rs` and pass via stdin
  rather than as an argv arg (avoids huge command lines).

## Out of scope (first cut)

- Tool-use rendering inside cards (just show summary line).
- Resuming an interrupted council.
- Multi-team selection in the UI (still uses `Roster::default_team`).
- Token/cost accounting per persona.

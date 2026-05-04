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

## Wiring (landed)

1. `app/src/personas.rs` — `cli_invocations()` returns invocations for
   live CLI personas in JSON mode; `streaming_args_for(basename)` is
   the canonical lookup. Legacy bash path (`build_council_command`,
   `oneshot_args_for`, `shell_escape`) deleted in `e9774db7`.

2. `app/src/terminal/input.rs` — `Event::EnterAgentCouncil { prompt }`
   landed in `9832af78`.

3. `app/src/terminal/view.rs` — `InputEvent::EnterAgentCouncil` builds
   the controller, calls `start(ctx)`, wraps in a `CouncilView`, and
   inserts via `insert_rich_content(RichContentType::AgentCouncil, …)`
   (`11975ffc`).

4. `app/src/terminal/input/slash_commands/mod.rs` — emits the event
   when CLI personas exist; falls through to `EnterAgentView` when not.

## Process spawning

`command::r#async::Command` with `kill_on_drop(true)`. `Stdio::piped()`
on stdout, `Stdio::null()` on stderr. One yarp task per persona reads
lines into an mpsc channel; a 50ms drain timer in `CouncilController`
applies events to state on the main thread. Dropping the controller
aborts the tasks, which drops the `Child`, which kills the process.

## Resolved questions

- **Render surface**: custom rich-content block (Option 3) — see
  research findings inline in commit `11975ffc`.
- **Persistence**: not in first cut.
- **Synthesis prompt**: assembled in `controller.rs::build_synth_prompt`
  and passed as the trailing positional argv (claude/codex argv length
  limits aren't a concern for the prompts we generate).

## Out of scope (first cut)

- Tool-use rendering inside cards (just show summary line).
- Resuming an interrupted council.
- Multi-team selection in the UI (still uses `Roster::default_team`).
- Token/cost accounting per persona.

## Status

Built and committed:
- `state.rs`, `event_stream.rs` (parser + 4 unit tests, all green)
- `controller.rs` (process spawning, mpsc channel, 50ms drain timer,
  synthesis pass; mirrors `OrchestrationEventPoller`'s pattern)
- `view.rs` (vertical card stack, phase labels, thinking + output panes
  rendered via `markdown_parser::parse_markdown` + `FormattedTextElement`)
- `personas::cli_invocations()` and `streaming_args_for()` in JSON modes
- `Event::EnterAgentCouncil { prompt }` plumbed through
  `terminal/input.rs` → `terminal/view.rs` (stub handler) →
  `slash_commands/mod.rs` (emits the event when CLI personas exist;
  falls through to `EnterAgentView` otherwise) [9832af78]

**Render surface decision:** custom rich-content block, *not* a pane
push or a modal. Research found that `EnterAgentView` itself uses
rich-content + controller subscription (PLAN.md previously claimed it
used a pane push — that was wrong). `AIBlock` is the proof that a
streaming, controller-backed view works through `insert_rich_content`.
Lowest line count, matches the existing mental model of "/agent output
appears in the terminal block list."

In flight (Task 3):
- `RichContentType::AgentCouncil` in `terminal/model/rich_content.rs`
- `RichContentMetadata::AgentCouncil { prompt }` in
  `terminal/view/rich_content.rs`
- Replace the stub handler with `insert_rich_content(...)` call

Remaining after Task 3:
- Smoke test: `/agent how should we ship this?` opens a card stack
  with live thinking + output from claude/codex/gemini, plus a synth
  verdict at the bottom.

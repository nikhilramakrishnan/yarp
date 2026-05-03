//! Owns the council run: spawns one async task per persona, drains their
//! events into `CouncilState`, and (when all peers finish) runs a synthesis
//! pass through the lead.
//!
//! Modeled on `OrchestrationEventPoller`'s drain-timer pattern: producer
//! tasks push `CouncilEvent`s into an `mpsc::UnboundedSender`; a periodic
//! timer in this model drains the receivers and applies events to state.

use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use futures::channel::mpsc;
use futures::AsyncBufReadExt as _;
use futures::StreamExt as _;
use yarpui::r#async::{SpawnedFutureHandle, Timer};
use yarpui::{Entity, ModelContext};

use crate::agent_council::event_stream::{parse_line, CliKind, CouncilEvent};
use crate::agent_council::state::{CardPhase, CouncilState, PersonaCard};
use crate::personas::{cli_invocations, streaming_args_for, synthesiser, Team};

const DRAIN_INTERVAL_MS: u64 = 50;

/// One persona's spawn recipe. Captured at controller construction so we
/// don't re-walk the roster on every `start` / synthesis pass.
struct OwnedInvocation {
    program: String,
    streaming_args: Vec<String>,
    binary_basename: String,
    badge: String,
    name: String,
}

pub struct CouncilController {
    pub state: CouncilState,
    invocations: Vec<OwnedInvocation>,
    synth_invocation: Option<OwnedInvocation>,
    receivers: Vec<PersonaReceiver>,
    /// Producer task handles. Dropped on controller drop, which cancels the
    /// futures and (because we set `kill_on_drop`) kills the children.
    _tasks: Vec<SpawnedFutureHandle>,
    /// Drain timer handle. Re-armed each tick.
    _drain_handle: Option<SpawnedFutureHandle>,
}

struct PersonaReceiver {
    /// Index into `state.cards`. `None` means the synthesis card.
    card_index: Option<usize>,
    rx: mpsc::UnboundedReceiver<CouncilEvent>,
    finished: bool,
}

impl Entity for CouncilController {
    type Event = ();
}

impl CouncilController {
    /// Build a controller seeded with one card per CLI persona on the team.
    /// Caller is responsible for then calling `start` to spawn processes.
    pub fn new(prompt: String, team: &Team) -> Self {
        let invocations: Vec<OwnedInvocation> = cli_invocations(team)
            .into_iter()
            .map(|inv| OwnedInvocation {
                program: inv.program,
                streaming_args: inv.streaming_args,
                binary_basename: inv.binary_basename,
                badge: inv.persona.badge.clone(),
                name: inv.persona.name.clone(),
            })
            .collect();
        let cards: Vec<PersonaCard> = invocations
            .iter()
            .map(|inv| PersonaCard::new(inv.badge.clone(), inv.name.clone(), inv.binary_basename.clone()))
            .collect();
        let synth_invocation = synthesiser(team).and_then(|lead| {
            let binary = lead.binary.as_deref()?;
            let basename = std::path::Path::new(binary)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned();
            let args = streaming_args_for(&basename)
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            Some(OwnedInvocation {
                program: binary.to_owned(),
                streaming_args: args,
                binary_basename: basename,
                badge: lead.badge.clone(),
                name: lead.name.clone(),
            })
        });
        let state = CouncilState {
            prompt,
            cards,
            verdict: None,
        };
        Self {
            state,
            invocations,
            synth_invocation,
            receivers: Vec::new(),
            _tasks: Vec::new(),
            _drain_handle: None,
        }
    }

    /// Spawn one process per persona. Each writes events into a channel that
    /// the drain timer drains back into state on the main thread.
    pub fn start(&mut self, ctx: &mut ModelContext<Self>) {
        if self.invocations.is_empty() {
            log::warn!("council: no CLI personas to dispatch");
            return;
        }
        // Take the invocations out so we can borrow self mutably while
        // iterating; put them back after.
        let invocations = std::mem::take(&mut self.invocations);
        for (idx, inv) in invocations.iter().enumerate() {
            let (tx, rx) = mpsc::unbounded();
            self.receivers.push(PersonaReceiver {
                card_index: Some(idx),
                rx,
                finished: false,
            });

            let program = inv.program.clone();
            let args = inv.streaming_args.clone();
            let kind = CliKind::from_basename(&inv.binary_basename);
            let prompt = self.state.prompt.clone();

            let handle = ctx.spawn(
                async move {
                    drive_persona(program, args, kind, prompt, tx).await;
                },
                |_me, _, _ctx| {},
            );
            self._tasks.push(handle);

            if let Some(card) = self.state.cards.get_mut(idx) {
                card.phase = CardPhase::Spawned;
            }
        }
        self.invocations = invocations;
        self.start_drain_timer(ctx);
    }

    fn start_drain_timer(&mut self, ctx: &mut ModelContext<Self>) {
        let handle = ctx.spawn(
            async move {
                Timer::after(Duration::from_millis(DRAIN_INTERVAL_MS)).await;
            },
            |me, _, ctx| {
                me.drain(ctx);
                if !me.is_complete() {
                    me.start_drain_timer(ctx);
                } else {
                    me._drain_handle = None;
                }
            },
        );
        self._drain_handle = Some(handle);
    }

    fn drain(&mut self, ctx: &mut ModelContext<Self>) {
        let mut any = false;
        for r in &mut self.receivers {
            if r.finished {
                continue;
            }
            loop {
                match r.rx.try_next() {
                    Ok(Some(ev)) => {
                        any = true;
                        apply_event(&mut self.state, r.card_index, ev, &mut r.finished);
                    }
                    Ok(None) => {
                        // Channel closed without a Finished event — treat as
                        // a clean EOF.
                        if !r.finished {
                            apply_event(
                                &mut self.state,
                                r.card_index,
                                CouncilEvent::Finished {
                                    ok: true,
                                    reason: None,
                                },
                                &mut r.finished,
                            );
                        }
                        break;
                    }
                    Err(_) => break, // empty
                }
            }
        }
        if any {
            ctx.notify();
        }
        // Once every persona is in a terminal phase, kick off synthesis. The
        // synth_invocation gets `take`n on entry, so subsequent ticks see
        // None and skip — no separate "started" bookkeeping needed.
        if self.state.all_done() && self.synth_invocation.is_some() {
            self.start_synthesis(ctx);
        }
    }

    fn is_complete(&self) -> bool {
        self.receivers.iter().all(|r| r.finished)
    }

    fn start_synthesis(&mut self, ctx: &mut ModelContext<Self>) {
        let Some(inv) = self.synth_invocation.take() else {
            return; // no CLI lead; verdict stays None
        };
        let kind = CliKind::from_basename(&inv.binary_basename);
        let synth_prompt = build_synth_prompt(&self.state);

        let mut card = PersonaCard::new(
            inv.badge.clone(),
            inv.name.clone(),
            inv.binary_basename.clone(),
        );
        card.phase = CardPhase::Spawned;
        self.state.verdict = Some(card);

        let (tx, rx) = mpsc::unbounded();
        self.receivers.push(PersonaReceiver {
            card_index: None, // None == verdict
            rx,
            finished: false,
        });

        let program = inv.program;
        let args = inv.streaming_args;
        let handle = ctx.spawn(
            async move {
                drive_persona(program, args, kind, synth_prompt, tx).await;
            },
            |_me, _, _ctx| {},
        );
        self._tasks.push(handle);
        // Drain timer is re-armed by its own callback when !is_complete(),
        // so we don't arm it here — the new receiver flips is_complete()
        // back to false and the next tick will keep the loop alive.
    }
}

fn apply_event(
    state: &mut CouncilState,
    card_index: Option<usize>,
    ev: CouncilEvent,
    finished: &mut bool,
) {
    let card: &mut PersonaCard = match card_index {
        Some(i) => match state.cards.get_mut(i) {
            Some(c) => c,
            None => return,
        },
        None => match state.verdict.as_mut() {
            Some(c) => c,
            None => return,
        },
    };
    match ev {
        CouncilEvent::ThinkingDelta(s) => {
            card.thinking.push_str(&s);
            card.phase = CardPhase::Thinking;
        }
        CouncilEvent::OutputDelta(s) => {
            card.output.push_str(&s);
            card.phase = CardPhase::Streaming;
        }
        CouncilEvent::ToolCall { name, summary } => {
            // Trim summary; some tool inputs are large JSON blobs.
            let summary = if summary.len() > 200 {
                format!("{}…", &summary[..200])
            } else {
                summary
            };
            card.tool_calls.push(format!("{name}: {summary}"));
        }
        CouncilEvent::Finished { ok, reason } => {
            card.phase = if ok {
                CardPhase::Done
            } else {
                CardPhase::Failed(reason.unwrap_or_else(|| "process failed".into()))
            };
            *finished = true;
        }
    }
}

/// Compose the synthesiser's prompt out of the prompt + every card's output.
fn build_synth_prompt(state: &CouncilState) -> String {
    let mut s = String::new();
    s.push_str("You are the lead of the Sandford NWA on this case:\n\n");
    s.push_str(&state.prompt);
    s.push_str("\n\n");
    for card in &state.cards {
        s.push_str(&format!("{} {} said:\n", card.badge, card.name));
        s.push_str(card.output.trim());
        s.push_str("\n\n");
    }
    s.push_str(
        "Identify points of agreement and disagreement, name the trade-off, \
         and deliver a tight final verdict. Cut the fluff.",
    );
    s
}

/// One persona's lifecycle: spawn the child, stream stdout line-by-line into
/// the channel as `CouncilEvent`s, then send `Finished` on EOF or error.
async fn drive_persona(
    program: String,
    args: Vec<String>,
    kind: CliKind,
    prompt: String,
    tx: mpsc::UnboundedSender<CouncilEvent>,
) {
    // Send stderr to /dev/null. If the CLI fails, the non-zero exit status
    // surfaces as Finished { ok: false, reason: "exit N" }; we don't need
    // the stderr text at the UI layer, and capturing it would mean either
    // spawning a thread per persona to drain a pipe or wiring a second
    // async read into this function. Re-add capture if a real diagnostic
    // need shows up.
    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .arg(&prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.unbounded_send(CouncilEvent::Finished {
                ok: false,
                reason: Some(format!("spawn failed: {e}")),
            });
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = tx.unbounded_send(CouncilEvent::Finished {
                ok: false,
                reason: Some("no stdout pipe".into()),
            });
            return;
        }
    };
    let reader = futures::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    while let Some(line_res) = lines.next().await {
        match line_res {
            Ok(line) => {
                for ev in parse_line(kind, &line) {
                    if tx.unbounded_send(ev).is_err() {
                        return; // controller went away; stop reading
                    }
                }
            }
            Err(e) => {
                let _ = tx.unbounded_send(CouncilEvent::Finished {
                    ok: false,
                    reason: Some(format!("read error: {e}")),
                });
                return;
            }
        }
    }

    // Stdout closed — wait for exit so we know success/failure.
    match child.status().await {
        Ok(status) if status.success() => {
            let _ = tx.unbounded_send(CouncilEvent::Finished {
                ok: true,
                reason: None,
            });
        }
        Ok(status) => {
            let _ = tx.unbounded_send(CouncilEvent::Finished {
                ok: false,
                reason: Some(format!("exit {}", status.code().unwrap_or(-1))),
            });
        }
        Err(e) => {
            let _ = tx.unbounded_send(CouncilEvent::Finished {
                ok: false,
                reason: Some(format!("wait failed: {e}")),
            });
        }
    }
}

//! Native council UI for `/agent <prompt>`.
//!
//! `/agent` emits `Event::EnterAgentCouncil`, which inserts a
//! `CouncilView` as a custom rich-content block in the active terminal's
//! block list. Each detected CLI persona (claude/codex/gemini/…) runs in
//! a child process with its JSON-streaming flag set; the controller
//! parses events and the view renders a card per persona with Thinking
//! and Output phases, capped by a synthesised Verdict card.
//!
//! See `specs/agent-council/PLAN.md` for design notes and `SMOKE.md`
//! for the end-to-end test plan.

pub mod controller;
pub mod event_stream;
pub mod state;
pub mod view;

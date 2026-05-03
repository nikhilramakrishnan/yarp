//! Native council UI for `/agent <prompt>`.
//!
//! Once wired into the slash command, this module replaces the
//! terminal-block bash fan-out in `personas::build_council_command` with
//! a Yarp view that renders each persona as a card with explicit Thinking
//! and Output phases, plus a final synthesised verdict.
//!
//! Today the legacy bash path is still live; the wire-up step is tracked
//! in `specs/agent-council/PLAN.md`.

pub mod controller;
pub mod event_stream;
pub mod state;
pub mod view;

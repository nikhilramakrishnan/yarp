//! Native council UI for `/agent <prompt>`.
//!
//! Replaces the terminal-block bash fan-out (`personas::build_council_command`)
//! with a Yarp view that renders each persona as a card with explicit
//! Thinking and Output phases, plus a final synthesised verdict.
//!
//! See `specs/agent-council/PLAN.md` for the design.

pub mod controller;
pub mod event_stream;
pub mod state;

// Wired up incrementally:
// pub mod view;

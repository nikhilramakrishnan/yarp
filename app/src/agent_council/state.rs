//! State for the council view: per-persona cards plus the synthesis card.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardPhase {
    /// Process not yet spawned.
    Pending,
    /// Process spawned, no events yet.
    Spawned,
    /// Receiving thinking deltas.
    Thinking,
    /// Receiving output deltas.
    Streaming,
    /// Process finished cleanly.
    Done,
    /// Process failed or was cancelled.
    Failed(String),
}

impl CardPhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed(_))
    }
}

/// One card per persona on the case.
#[derive(Debug, Clone)]
pub struct PersonaCard {
    pub badge: String,
    pub name: String,
    pub binary_basename: String,
    pub phase: CardPhase,
    pub thinking: String,
    pub output: String,
    /// Compact summaries of any tool_use events the CLI emitted.
    pub tool_calls: Vec<String>,
}

impl PersonaCard {
    pub fn new(badge: String, name: String, binary_basename: String) -> Self {
        Self {
            badge,
            name,
            binary_basename,
            phase: CardPhase::Pending,
            thinking: String::new(),
            output: String::new(),
            tool_calls: Vec::new(),
        }
    }
}

/// Top-level council state. Owned by `CouncilController`.
#[derive(Debug, Clone)]
pub struct CouncilState {
    pub prompt: String,
    pub cards: Vec<PersonaCard>,
    /// The synthesiser's card. Spawned only after every member of `cards` is
    /// in a terminal phase. `None` if no synthesiser is configured for the
    /// active team.
    pub verdict: Option<PersonaCard>,
}

impl CouncilState {
    pub fn all_done(&self) -> bool {
        !self.cards.is_empty() && self.cards.iter().all(|c| c.phase.is_terminal())
    }
}

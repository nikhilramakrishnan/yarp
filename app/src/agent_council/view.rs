//! Renders the council: one card per persona stacked vertically, plus a
//! final verdict card once the lead has synthesised.
//!
//! Live updates: the view subscribes to its `CouncilController` and calls
//! its own `ctx.notify()` on every emit; the controller emits `()` whenever
//! it drains new events.

use yarpui::elements::{Container, CrossAxisAlignment, Element, Flex, ParentElement, Text};
use yarpui::fonts::{Properties, Weight};
use yarpui::{AppContext, Entity, ModelHandle, SingletonEntity, View, ViewContext};

use crate::agent_council::controller::CouncilController;
use crate::agent_council::state::{CardPhase, PersonaCard};
use crate::appearance::Appearance;

pub struct CouncilView {
    controller: ModelHandle<CouncilController>,
}

impl Entity for CouncilView {
    type Event = ();
}

impl CouncilView {
    pub fn new(
        controller: ModelHandle<CouncilController>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&controller, |_this, _, _evt, ctx| {
            ctx.notify();
        });
        Self { controller }
    }
}

impl View for CouncilView {
    fn ui_name() -> &'static str {
        "CouncilView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let controller = self.controller.as_ref(app);
        let state = &controller.state;

        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        // Header: the case being investigated.
        column.add_child(
            Container::new(
                Text::new(
                    format!("Sandford NWA on the case: {}", state.prompt),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_style(Properties::default().weight(Weight::Bold))
                .finish(),
            )
            .with_margin_bottom(8.)
            .finish(),
        );

        for card in &state.cards {
            column.add_child(render_card(card, appearance));
        }

        if let Some(verdict) = state.verdict.as_ref() {
            column.add_child(
                Container::new(
                    Text::new(
                        "Verdict",
                        appearance.ui_font_family(),
                        appearance.ui_font_size(),
                    )
                    .with_style(Properties::default().weight(Weight::Bold))
                    .finish(),
                )
                .with_margin_top(12.)
                .with_margin_bottom(4.)
                .finish(),
            );
            column.add_child(render_card(verdict, appearance));
        }

        column.finish()
    }
}

fn render_card(card: &PersonaCard, appearance: &Appearance) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    // Header row: badge + name + binary + phase label.
    let header = format!(
        "{} {} ({}) — {}",
        card.badge,
        card.name,
        card.binary_basename,
        phase_label(&card.phase),
    );
    col.add_child(
        Container::new(
            Text::new(header, appearance.ui_font_family(), appearance.ui_font_size())
                .with_style(Properties::default().weight(Weight::Bold))
                .finish(),
        )
        .with_margin_bottom(4.)
        .finish(),
    );

    // Thinking pane (only shown if non-empty).
    if !card.thinking.is_empty() {
        col.add_child(
            Container::new(
                Text::new(
                    "Thinking",
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_style(Properties::default().weight(Weight::Bold))
                .finish(),
            )
            .with_margin_top(2.)
            .finish(),
        );
        col.add_child(
            Container::new(
                Text::new(
                    card.thinking.clone(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .finish(),
            )
            .with_margin_bottom(4.)
            .finish(),
        );
    }

    // Output (always shown so empty cards still have a slot).
    col.add_child(
        Container::new(
            Text::new(
                if card.output.is_empty() {
                    "…".to_owned()
                } else {
                    card.output.clone()
                },
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .finish(),
        )
        .with_margin_bottom(2.)
        .finish(),
    );

    // Tool-call chips, one per row, terse.
    if !card.tool_calls.is_empty() {
        for tc in &card.tool_calls {
            col.add_child(
                Container::new(
                    Text::new(
                        format!("• {tc}"),
                        appearance.ui_font_family(),
                        appearance.ui_font_size(),
                    )
                    .finish(),
                )
                .finish(),
            );
        }
    }

    // Failure reason, if any.
    if let CardPhase::Failed(reason) = &card.phase {
        col.add_child(
            Container::new(
                Text::new(
                    format!("Failed: {reason}"),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .finish(),
            )
            .with_margin_top(2.)
            .finish(),
        );
    }

    Container::new(col.finish())
        .with_margin_top(6.)
        .with_margin_bottom(6.)
        .finish()
}

fn phase_label(phase: &CardPhase) -> &'static str {
    match phase {
        CardPhase::Pending => "pending",
        CardPhase::Spawned => "starting",
        CardPhase::Thinking => "thinking…",
        CardPhase::Streaming => "streaming…",
        CardPhase::Done => "done",
        CardPhase::Failed(_) => "failed",
    }
}


//! Renders the council as native-style terminal blocks.
//!
//! Each persona becomes its own rich-content block (`PersonaBlock`) and the
//! synthesised verdict is one more block (`VerdictBlock`). Both use the same
//! chrome convention as `agent_view::zero_state_block`: full-width strip
//! with the standard horizontal terminal padding, vertical breathing room,
//! and a 1px top border in the theme outline color so adjacent blocks read
//! as separate entries in the block list — same visual paradigm as a shell
//! command block.
//!
//! Both block types subscribe to their `CouncilController` and `notify` on
//! every emit, so streaming output repaints live.

use markdown_parser::parse_markdown;
use pathfinder_color::ColorU;
use yarpui::elements::{
    Border, Container, CrossAxisAlignment, Element, Empty, Flex, FormattedTextElement,
    ParentElement, Text,
};
use yarpui::fonts::{Properties, Weight};
use yarpui::{AppContext, Entity, ModelHandle, SingletonEntity, View, ViewContext};

use crate::agent_council::controller::CouncilController;
use crate::agent_council::state::{CardPhase, PersonaCard};
use crate::appearance::Appearance;
use crate::terminal::view::PADDING_LEFT;

/// Standard vertical padding inside a council block. Matches
/// `zero_state_block`'s `CONTAINER_VERTICAL_PADDING` so council blocks read
/// as siblings of other terminal block widgets.
const VERTICAL_PADDING: f32 = 16.;

/// One block per persona in the fan-out. Stacks naturally in the block list
/// alongside the verdict block and any other terminal blocks.
pub struct PersonaBlock {
    controller: ModelHandle<CouncilController>,
    card_index: usize,
}

impl Entity for PersonaBlock {
    type Event = ();
}

impl PersonaBlock {
    pub fn new(
        controller: ModelHandle<CouncilController>,
        card_index: usize,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&controller, |_this, _, _evt, ctx| ctx.notify());
        Self {
            controller,
            card_index,
        }
    }
}

impl View for PersonaBlock {
    fn ui_name() -> &'static str {
        "PersonaBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let controller = self.controller.as_ref(app);
        let Some(card) = controller.state.cards.get(self.card_index) else {
            return Empty::new().finish();
        };
        block_chrome(persona_body(card, appearance), appearance)
    }
}

/// Verdict block. Renders a zero-height `Empty` until synthesis fires; once
/// `state.verdict` is `Some`, renders the synthesised take with its own
/// chrome and a small attribution footer.
pub struct VerdictBlock {
    controller: ModelHandle<CouncilController>,
}

impl Entity for VerdictBlock {
    type Event = ();
}

impl VerdictBlock {
    pub fn new(
        controller: ModelHandle<CouncilController>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&controller, |_this, _, _evt, ctx| ctx.notify());
        Self { controller }
    }
}

impl View for VerdictBlock {
    fn ui_name() -> &'static str {
        "VerdictBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let controller = self.controller.as_ref(app);
        let Some(card) = controller.state.verdict.as_ref() else {
            return Empty::new().finish();
        };
        block_chrome(verdict_body(card, appearance), appearance)
    }
}

/// Wrap `body` in the standard Warp block chrome: full-width container with
/// horizontal padding from `PADDING_LEFT`, vertical padding, and a 1px top
/// border in the theme outline color. No corner radius, no background fill —
/// blocks look like horizontal strips, the same way shell command blocks do.
fn block_chrome(body: Box<dyn Element>, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(body)
        .with_horizontal_padding(*PADDING_LEFT)
        .with_vertical_padding(VERTICAL_PADDING)
        .with_border(
            Border::new(1.)
                .with_sides(true, false, false, false)
                .with_border_fill(theme.outline()),
        )
        .finish()
}

/// Layout for a fan-out persona block. Header strip with the persona's name +
/// phase label, then thinking + output panes, then any tool-call chips, then
/// a failure reason if applicable.
fn persona_body(card: &PersonaCard, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg = theme.background();
    let main_color = theme.main_text_color(bg).into_solid();

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    // Header: "{badge} {name} — {phase}". When the persona's name equals the
    // binary basename (the default for auto-detected CLIs), don't print the
    // basename twice — it reads as "claude (claude)".
    let label = if card.name == card.binary_basename {
        format!("{} {} — {}", card.badge, card.name, phase_label(&card.phase))
    } else {
        format!(
            "{} {} ({}) — {}",
            card.badge,
            card.name,
            card.binary_basename,
            phase_label(&card.phase)
        )
    };
    col.add_child(
        Container::new(
            Text::new(label, appearance.ui_font_family(), appearance.ui_font_size())
                .with_style(Properties::default().weight(Weight::Bold))
                .finish(),
        )
        .with_margin_bottom(8.)
        .finish(),
    );

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
            .with_margin_bottom(2.)
            .finish(),
        );
        col.add_child(
            Container::new(render_markdown_or_plain(
                &card.thinking,
                appearance,
                main_color,
            ))
            .with_margin_bottom(6.)
            .finish(),
        );
    }

    // Failed cards show only the failure row — the empty placeholder there
    // is noise.
    let show_output = !matches!(card.phase, CardPhase::Failed(_));
    if show_output {
        if card.output.is_empty() {
            col.add_child(
                Container::new(
                    Text::new("…", appearance.ui_font_family(), appearance.ui_font_size())
                        .finish(),
                )
                .finish(),
            );
        } else {
            col.add_child(
                Container::new(render_markdown_or_plain(
                    &card.output,
                    appearance,
                    main_color,
                ))
                .finish(),
            );
        }
    }

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

    col.finish()
}

/// Layout for the synthesis verdict. Header is the literal "Verdict", body
/// is the lead's markdown output, footer is a small "synthesised by …"
/// attribution. No persona-card header — the verdict isn't another fan-out
/// take, it's the lead's call.
fn verdict_body(card: &PersonaCard, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg = theme.background();
    let main_color = theme.main_text_color(bg).into_solid();

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    col.add_child(
        Container::new(
            Text::new(
                "Verdict",
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .with_style(Properties::default().weight(Weight::Bold))
            .finish(),
        )
        .with_margin_bottom(8.)
        .finish(),
    );

    if !card.output.is_empty() {
        col.add_child(
            Container::new(render_markdown_or_plain(
                &card.output,
                appearance,
                main_color,
            ))
            .finish(),
        );
    } else if let CardPhase::Failed(reason) = &card.phase {
        col.add_child(
            Container::new(
                Text::new(
                    format!("Failed: {reason}"),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .finish(),
            )
            .finish(),
        );
    } else {
        col.add_child(
            Container::new(
                Text::new("…", appearance.ui_font_family(), appearance.ui_font_size())
                    .finish(),
            )
            .finish(),
        );
    }

    col.add_child(
        Container::new(
            Text::new(
                format!("— synthesised by {}", card.name),
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .finish(),
        )
        .with_margin_top(8.)
        .finish(),
    );

    col.finish()
}

/// Render `body` as parsed markdown via `FormattedTextElement`. Council
/// output comes from third-party CLIs and may not be valid markdown; on
/// parse error fall back to plain `Text` rather than panic.
fn render_markdown_or_plain(
    body: &str,
    appearance: &Appearance,
    text_color: ColorU,
) -> Box<dyn Element> {
    match parse_markdown(body) {
        Ok(parsed) => FormattedTextElement::new(
            parsed,
            appearance.ui_font_size(),
            appearance.ui_font_family(),
            appearance.ui_font_family(),
            text_color,
            Default::default(),
        )
        .with_inline_code_properties(Some(text_color), None)
        .finish(),
        Err(_) => Text::new(
            body.to_owned(),
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .finish(),
    }
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

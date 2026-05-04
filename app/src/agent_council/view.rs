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

use std::sync::Arc;

use markdown_parser::parse_markdown;
use parking_lot::RwLock;
use pathfinder_color::ColorU;
use yarpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, Element, Empty, Flex,
    FormattedTextElement, MainAxisSize, ParentElement, Radius, SelectableArea, SelectionHandle,
    Shrinkable, Text,
};
use yarpui::fonts::{Properties, Weight};
use yarpui::{AppContext, Entity, ModelHandle, SingletonEntity, View, ViewContext};

use crate::terminal::block_list_element::BlockListMenuSource;
use crate::terminal::view::TerminalAction;

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
    selection_handle: SelectionHandle,
    selected_text: Arc<RwLock<Option<String>>>,
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
            selection_handle: SelectionHandle::default(),
            selected_text: Arc::new(RwLock::new(None)),
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
        let chrome = block_chrome(
            persona_body(card, &controller.state.prompt, appearance),
            appearance,
        );
        wrap_selectable(chrome, &self.selection_handle, &self.selected_text)
    }
}

/// Verdict block. Renders a zero-height `Empty` until synthesis fires; once
/// `state.verdict` is `Some`, renders the synthesised take with its own
/// chrome and a small attribution footer.
pub struct VerdictBlock {
    controller: ModelHandle<CouncilController>,
    selection_handle: SelectionHandle,
    selected_text: Arc<RwLock<Option<String>>>,
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
        Self {
            controller,
            selection_handle: SelectionHandle::default(),
            selected_text: Arc::new(RwLock::new(None)),
        }
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
        let chrome = block_chrome(verdict_body(card, appearance), appearance);
        wrap_selectable(chrome, &self.selection_handle, &self.selected_text)
    }
}

/// Wrap an element in a `SelectableArea` so the user can drag-select text
/// inside the block (and copy with Cmd+C / right-click → Copy). Mirrors the
/// pattern AIBlock uses at `view_impl.rs:1197`.
fn wrap_selectable(
    body: Box<dyn Element>,
    selection_handle: &SelectionHandle,
    selected_text: &Arc<RwLock<Option<String>>>,
) -> Box<dyn Element> {
    let captured_selected_text = selected_text.clone();
    SelectableArea::new(
        selection_handle.clone(),
        move |selection_args, _, _| {
            *captured_selected_text.write() = selection_args.selection;
        },
        body,
    )
    .on_selection_right_click(move |ctx, position| {
        ctx.dispatch_typed_action(TerminalAction::BlockListContextMenu(
            BlockListMenuSource::OutsideBlockRightClick {
                position_in_terminal_view: position,
            },
        ));
    })
    .finish()
}

/// Wrap `body` in AIBlock-style block chrome: tinted background overlay
/// (`theme.ai_blocks_overlay()`) so council blocks visually distinguish from
/// regular shell command blocks the same way AIBlock does, with a 1px top
/// border in the outline color and the standard horizontal terminal
/// padding. This is the same chrome AIBlock uses (`view_impl.rs:1128-1190`):
///
/// - background: `theme.ai_blocks_overlay()`
/// - top border: 1px `theme.outline()`
/// - top padding: 16px (`CONTENT_VERTICAL_PADDING`)
/// - horizontal padding: `PADDING_LEFT`
fn block_chrome(body: Box<dyn Element>, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(body)
        .with_background(theme.ai_blocks_overlay())
        .with_horizontal_padding(*PADDING_LEFT)
        .with_padding_top(VERTICAL_PADDING)
        .with_padding_bottom(VERTICAL_PADDING)
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
fn persona_body(card: &PersonaCard, prompt: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg = theme.background();
    let main_color = theme.main_text_color(bg).into_solid();

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    // Header: a row that mirrors AIBlock's "avatar + query" layout. The
    // badge sits in a small chip on the left (theme.surface_3 background,
    // 4px corner radius, padded — same recipe as AIBlock's attached-blocks
    // chip, view_impl/header.rs:188-193). The persona name + phase label
    // go to the right in bold, like the user's query in AIBlock.
    col.add_child(
        Container::new(persona_header_row(card, appearance))
            .with_margin_bottom(6.)
            .finish(),
    );

    // Below the header: the prompt being asked of this persona, in
    // monospace with a "$ " prefix. Mirrors the way a shell command block
    // shows its command at the top — readers can see what each persona is
    // running without having to remember the original /agent prompt.
    let prompt_row = Container::new(
        Text::new(
            format!("$ {} {}", card.binary_basename, shell_quote(prompt)),
            appearance.monospace_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(theme.sub_text_color(theme.background()).into_solid())
        .finish(),
    )
    .with_margin_bottom(8.)
    .finish();
    col.add_child(prompt_row);

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

/// Header row: badge chip on the left, persona name + phase on the right.
/// Mirrors `view_impl/query.rs::render_query`'s avatar+query Flex::row
/// layout and `view_impl/header.rs::188`'s chip styling.
fn persona_header_row(card: &PersonaCard, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let chip_bg = theme.surface_3();
    let chip_text = theme.main_text_color(theme.surface_3()).into_solid();
    let main_color = theme.main_text_color(theme.background()).into_solid();
    let sub_color = theme.sub_text_color(theme.background()).into_solid();

    let chip = Container::new(
        Text::new(
            card.badge.clone(),
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(chip_text)
        .with_style(Properties::default().weight(Weight::Bold))
        .finish(),
    )
    .with_background(chip_bg)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
    .with_horizontal_padding(8.)
    .with_vertical_padding(4.)
    .with_margin_right(12.)
    .finish();

    // Skip the basename parens when name == basename (auto-detected CLIs).
    let name_text = if card.name == card.binary_basename {
        card.name.clone()
    } else {
        format!("{} ({})", card.name, card.binary_basename)
    };

    let name_el = Text::new(
        name_text,
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(main_color)
    .with_style(Properties::default().weight(Weight::Bold))
    .finish();

    let phase_el = Text::new(
        phase_label(&card.phase).to_owned(),
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(sub_color)
    .finish();

    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(chip)
        .with_child(Shrinkable::new(1., name_el).finish())
        .with_child(Container::new(phase_el).with_margin_left(8.).finish())
        .finish()
}

/// Wrap `s` in double-quotes if it contains whitespace or shell metacharacters.
/// Cheap escaping — good enough for the command-preview row, not for actual
/// execution (the controller doesn't use this).
fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    let needs_quoting = s
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | '$' | '`' | '\\' | ';' | '|' | '&' | '<' | '>' | '(' | ')'));
    if !needs_quoting {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '\\' | '$' | '`') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
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

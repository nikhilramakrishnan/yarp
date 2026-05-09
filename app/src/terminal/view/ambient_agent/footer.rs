use pathfinder_color::ColorU;
use yarp_core::ui::appearance::Appearance;
use yarpui::{
    elements::{
        Border, ConstrainedBox, Container, CrossAxisAlignment, Flex, MainAxisAlignment,
        MainAxisSize, ParentElement, Text,
    },
    fonts::{Properties, Weight},
    Element,
};

use crate::ui_components::blended_colors;

const CONTENT_SPACING: f32 = 4.;
const HORIZONTAL_PADDING: f32 = 12.;
const VERTICAL_PADDING: f32 = 8.;
const BORDER_WIDTH: f32 = 1.0;
const MAX_CONTENT_WIDTH: f32 = 400.;

/// Helper to build a centered two-line footer with common styling.
fn build_centered_footer(
    header_text: String,
    body_text: String,
    header_color: ColorU,
    body_color: ColorU,
    background: ColorU,
    border_color: ColorU,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let content = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_main_axis_alignment(MainAxisAlignment::Center)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(CONTENT_SPACING)
        .with_child(
            Text::new(
                header_text,
                appearance.ui_font_family(),
                appearance.ui_font_size() + 2.,
            )
            .with_style(Properties::default().weight(Weight::Bold))
            .with_color(header_color)
            .finish(),
        )
        .with_child(
            Text::new(
                body_text,
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .with_color(body_color)
            .finish(),
        )
        .finish();

    let content = ConstrainedBox::new(content)
        .with_max_width(MAX_CONTENT_WIDTH)
        .finish();

    // Use a row to horizontally center the content.
    let content = Flex::row()
        .with_child(content)
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(MainAxisAlignment::Center)
        .finish();

    Container::new(content)
        .with_background(background)
        .with_border(Border::top(BORDER_WIDTH).with_border_fill(border_color))
        .with_horizontal_padding(HORIZONTAL_PADDING)
        .with_vertical_padding(VERTICAL_PADDING)
        .finish()
}

/// Render a loading footer that replaces the terminal input while waiting to connect to an
// ambient agent session.
pub fn render_loading_footer(appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();

    let header_color = blended_colors::text_main(theme, theme.background());
    let body_color = blended_colors::text_disabled(theme, theme.background());
    let background = theme.surface_2().into();
    // The loading footer replaces the terminal input while we wait for an
    // ambient officer to report in — when the radio stripe is active on the
    // surrounding chrome, this seam needs to match so the operator's eye
    // sees a single contiguous edge: self → red, peer → yellow, post-ack
    // 5s → green, neutral_4 fallback.
    let border_color = if crate::radio::self_in_mayday() {
        theme.ansi_fg_red()
    } else if crate::radio::peer_in_mayday() {
        theme.ansi_fg_yellow()
    } else if crate::radio::time_since_self_stand_down()
        .map(|e| e.as_secs() < crate::radio::SELF_INBOX_ACK_BAR_SECS)
        .unwrap_or(false)
        || crate::radio::time_since_self_inbox_ack().is_some()
    {
        theme.ansi_fg_green()
    } else {
        blended_colors::neutral_4(theme)
    };

    build_centered_footer(
        "Ambient officer reporting in…".to_string(),
        "Stand by — you'll be on the radio in a tick".to_string(),
        header_color,
        body_color,
        background,
        border_color,
        appearance,
    )
}

/// Render an error footer that shows when the ambient agent failed to spawn.
pub fn render_error_footer(error_message: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();

    let header_color = theme.ui_error_color();
    let body_color = blended_colors::text_main(theme, theme.background());
    let background: ColorU = {
        let red: yarp_core::ui::theme::Fill = theme.ui_error_color().into();
        red.with_opacity(50).into()
    };
    let border_color = theme.ui_error_color();

    build_centered_footer(
        "PC down".to_string(),
        error_message.to_string(),
        header_color,
        body_color,
        background,
        border_color,
        appearance,
    )
}

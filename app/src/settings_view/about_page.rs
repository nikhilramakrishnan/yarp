use std::time::Duration;

use super::{
    settings_page::{
        MatchData, PageType, SettingsPageEvent, SettingsPageMeta, SettingsPageViewHandle,
        SettingsWidget,
    },
    SettingsSection,
};
use crate::{
    appearance::Appearance, channel::ChannelState, radio, themes::theme::ColorScheme,
    workspace::WorkspaceAction,
};
use yarpui::{
    assets::asset_cache::AssetSource,
    elements::{
        Align, CacheOption, ConstrainedBox, Container, CrossAxisAlignment, Element, Empty, Flex,
        Image, MainAxisAlignment, MouseStateHandle, ParentElement, Wrap,
    },
    fonts::Weight,
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    AppContext, Entity, View, ViewContext, ViewHandle,
};

pub struct AboutPageView {
    page: PageType<Self>,
}

impl AboutPageView {
    pub fn new(ctx: &mut ViewContext<AboutPageView>) -> Self {
        let view = AboutPageView {
            page: PageType::new_monolith(AboutPageWidget::default(), None, false),
        };
        Self::schedule_radio_poll(ctx);
        view
    }

    // Re-renders the page on a 5s heartbeat so dispatch ages tick and new
    // arrivals from other officers surface without re-navigating to About.
    // Skips the notify when no peers and no inbox — sole-officer instances
    // don't need to repaint.
    fn schedule_radio_poll(ctx: &mut ViewContext<AboutPageView>) {
        ctx.spawn(
            async move {
                yarpui::r#async::Timer::after(Duration::from_secs(5)).await;
            },
            |_me, _, ctx| {
                if !radio::peers().is_empty() || !radio::peek_inbox().is_empty() {
                    ctx.notify();
                }
                Self::schedule_radio_poll(ctx);
            },
        );
    }
}

impl Entity for AboutPageView {
    type Event = SettingsPageEvent;
}

impl View for AboutPageView {
    fn ui_name() -> &'static str {
        "AboutPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

#[derive(Default)]
struct AboutPageWidget {
    copy_version_button_mouse_state: MouseStateHandle,
    ack_dispatch_button_mouse_state: MouseStateHandle,
    mic_check_button_mouse_state: MouseStateHandle,
    ten_thirteen_button_mouse_state: MouseStateHandle,
}

impl SettingsWidget for AboutPageWidget {
    type View = AboutPageView;

    fn search_terms(&self) -> &str {
        "about yarp version"
    }

    fn render(
        &self,
        _view: &AboutPageView,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();

        let image_path = if theme.inferred_color_scheme() == ColorScheme::LightOnDark {
            "bundled/svg/yarp-logo-with-light-title.svg"
        } else {
            "bundled/svg/yarp-logo-with-dark-title.svg"
        };

        let version = ChannelState::app_version().unwrap_or("v#.##.###");

        let version_text = ui_builder
            .span(version.to_string())
            .with_soft_wrap()
            .build()
            .with_margin_top(16.)
            .finish();

        let copy_version_icon = appearance
            .ui_builder()
            .copy_button(16., self.copy_version_button_mouse_state.clone())
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::CopyVersion(version));
            })
            .finish();

        let version_row = Wrap::row()
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_children([
                version_text,
                Container::new(copy_version_icon)
                    .with_margin_top(16.)
                    .with_padding_left(6.)
                    .finish(),
            ]);

        Align::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    ConstrainedBox::new(
                        Image::new(
                            AssetSource::Bundled { path: image_path },
                            CacheOption::BySize,
                        )
                        .finish(),
                    )
                    .with_max_height(100.)
                    .with_max_width(350.)
                    .finish(),
                )
                .with_child(version_row.finish())
                .with_child(self.precinct_status_row(appearance))
                .with_child(
                    ui_builder
                        .span(format!("On the air as: {}", radio::self_call_sign()))
                        .with_soft_wrap()
                        .build()
                        .with_margin_top(4.)
                        .finish(),
                )
                .with_child(self.precinct_roster_row(appearance))
                .with_child(self.precinct_inbox_row(appearance))
                .with_child(self.precinct_latest_dispatch_row(appearance))
                .with_child(self.mic_check_row(appearance))
                .with_child(
                    ui_builder
                        .span("Copyright 2026 Yarp contributors. Sandford. Population: 1.")
                        .with_soft_wrap()
                        .build()
                        .with_margin_top(16.)
                        .finish(),
                )
                .finish(),
        )
        .finish()
    }
}

// Precinct-state banner. Code 4 = all clear; Code 3 = emergency response.
// Real-world police shorthand mapped onto the radio domain — the banner gives
// the whole precinct stack a unifying status pulse above the per-row detail,
// folding officer count into the same line so the stack stays tight.
fn precinct_status_line() -> (String, bool) {
    let emergency_count = radio::peek_inbox()
        .iter()
        .filter(|m| dispatch_is_emergency(&m.body))
        .count();
    let peer_count = radio::peers().len();
    let officer_phrase = match peer_count {
        0 => None,
        1 => Some("1 other officer on channel".to_string()),
        n => Some(format!("{n} other officers on channel")),
    };
    if emergency_count > 0 {
        let phrase = if emergency_count == 1 {
            "1 emergency pending".to_string()
        } else {
            format!("{emergency_count} emergencies pending")
        };
        let line = match officer_phrase {
            Some(p) => format!("Code 3 \u{00B7} {phrase} \u{00B7} {p}"),
            None => format!("Code 3 \u{00B7} {phrase}"),
        };
        return (line, true);
    }
    let line = match officer_phrase {
        Some(p) => format!("Code 4 \u{00B7} all clear \u{00B7} {p}"),
        None => "Code 4 \u{00B7} sole patrol".to_string(),
    };
    (line, false)
}

fn precinct_roster_line() -> Option<(String, bool)> {
    let peer_list = radio::peers();
    if peer_list.is_empty() {
        return None;
    }
    // Solidarity surface: tag any peer with a pending 10-13 in the inbox.
    // The (10-13) marker replaces their tab title — urgency dominates context —
    // and the bool tints the whole row red so the signal reads at a glance.
    let emergency_signs: std::collections::HashSet<String> = radio::peek_inbox()
        .iter()
        .filter(|m| dispatch_is_emergency(&m.body))
        .map(|m| m.from_call_sign.clone())
        .collect();
    let distress_count = peer_list
        .iter()
        .filter(|p| emergency_signs.contains(&p.call_sign))
        .count();
    let any_in_distress = distress_count > 0;
    let mut names: Vec<String> = peer_list
        .iter()
        .map(|p| {
            if emergency_signs.contains(&p.call_sign) {
                format!("{} (10-13)", p.call_sign)
            } else {
                match &p.tab_title {
                    Some(title) => format!("{} ({})", p.call_sign, title),
                    None => p.call_sign.clone(),
                }
            }
        })
        .collect();
    // Cap rendered names; trailing "+N more" if oversized.
    const MAX: usize = 5;
    let overflow = names.len().saturating_sub(MAX);
    names.truncate(MAX);
    // Multi-peer 10-13 gets a count tag in the label — at-a-glance distress
    // total above the per-peer (10-13) markers. Single-peer skips the tag;
    // the inline marker carries the signal alone.
    let label = if distress_count >= 2 {
        format!("Roster ({distress_count} in distress)")
    } else {
        "Roster".to_string()
    };
    let line = if overflow > 0 {
        format!("{label}: {} (+{overflow} more)", names.join(", "))
    } else {
        format!("{label}: {}", names.join(", "))
    };
    Some((line, any_in_distress))
}

fn precinct_inbox_line() -> Option<(String, bool)> {
    let inbox = radio::peek_inbox();
    if inbox.is_empty() {
        return None;
    }
    // Any pending 10-13 turns the whole inbox line red — emergency dominates routine.
    let emergency_count = inbox.iter().filter(|m| dispatch_is_emergency(&m.body)).count();
    let emergency = emergency_count > 0;
    // Count repeats per sender so a chatty officer doesn't crowd the line,
    // and preserve arrival order for predictable rendering.
    let mut order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for msg in &inbox {
        if seen.insert(msg.from_call_sign.as_str()) {
            order.push(msg.from_call_sign.clone());
        }
    }
    let distinct = order.len();
    const MAX: usize = 4;
    let overflow = order.len().saturating_sub(MAX);
    order.truncate(MAX);
    let roster = if overflow > 0 {
        format!("{} +{overflow} more", order.join(", "))
    } else {
        order.join(", ")
    };
    let total = inbox.len();
    let dispatches = if total == 1 {
        "1 pending dispatch".to_string()
    } else {
        format!("{total} pending dispatches")
    };
    // One officer (chatty or solo) reads cleaner without the redundant count.
    let body = if distinct == 1 {
        format!("{dispatches} from {roster}.")
    } else {
        format!("{dispatches} from {distinct} officers ({roster}).")
    };
    // Lead with the emergency count when any 10-13 is pending — urgency before volume.
    let line = if emergency_count > 0 {
        let prefix = if emergency_count == 1 {
            "1 emergency".to_string()
        } else {
            format!("{emergency_count} emergencies")
        };
        format!("{prefix} \u{00B7} {body}")
    } else {
        body
    };
    Some((line, emergency))
}

fn precinct_latest_dispatch_text() -> Option<(String, bool)> {
    let msg = radio::latest_dispatch()?;
    let emergency = dispatch_is_emergency(&msg.body);
    // Cap body length so a chatty officer can't blow out the layout.
    const MAX_BODY: usize = 80;
    let body = if msg.body.chars().count() > MAX_BODY {
        let truncated: String = msg.body.chars().take(MAX_BODY).collect();
        format!("{truncated}…")
    } else {
        msg.body.clone()
    };
    let age = radio::format_dispatch_age(msg.sent_at_unix);
    let line = format!(
        "Latest from {} ({age}): \u{201C}{body}\u{201D}",
        msg.from_call_sign
    );
    Some((line, emergency))
}

// 10-13 = officer needs assistance. Detected on the raw body so the latest-dispatch
// row can paint itself red and read as an emergency at a glance.
fn dispatch_is_emergency(body: &str) -> bool {
    body.trim_start().starts_with("10-13")
}

impl AboutPageWidget {
    fn mic_check_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        // No peers on the channel — nothing to broadcast at.
        if radio::peers().is_empty() {
            return Empty::new().finish();
        }
        let ui_builder = appearance.ui_builder();
        let radio_button_style = UiComponentStyles {
            font_size: Some(12.),
            font_weight: Some(Weight::Semibold),
            border_radius: Some(yarpui::elements::CornerRadius::with_all(
                yarpui::elements::Radius::Pixels(4.),
            )),
            padding: Some(Coords {
                top: 4.,
                bottom: 4.,
                left: 12.,
                right: 12.,
            }),
            ..Default::default()
        };

        let mic_check = ui_builder
            .button(
                ButtonVariant::Secondary,
                self.mic_check_button_mouse_state.clone(),
            )
            .with_style(radio_button_style.clone())
            .with_text_label("Mic check".to_owned())
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::MicCheckBroadcast);
            })
            .finish();

        // 10-13 = officer needs assistance. Red so it reads at a glance.
        let ten_thirteen = ui_builder
            .button(
                ButtonVariant::Error,
                self.ten_thirteen_button_mouse_state.clone(),
            )
            .with_style(radio_button_style)
            .with_text_label("10-13".to_owned())
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::TenThirteenBroadcast);
            })
            .finish();

        Container::new(
            Wrap::row()
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_children([
                    mic_check,
                    Container::new(ten_thirteen).with_padding_left(8.).finish(),
                ])
                .finish(),
        )
        .with_margin_top(8.)
        .finish()
    }

    fn precinct_status_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let (line, emergency) = precinct_status_line();
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();
        let mut span = ui_builder.span(line).with_soft_wrap();
        if emergency {
            span = span.with_style(UiComponentStyles {
                font_color: Some(theme.terminal_colors().normal.red.into()),
                font_weight: Some(Weight::Semibold),
                ..Default::default()
            });
        } else {
            span = span.with_style(UiComponentStyles {
                font_weight: Some(Weight::Semibold),
                ..Default::default()
            });
        }
        span.build().with_margin_top(16.).finish()
    }

    fn precinct_roster_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some((line, any_in_distress)) = precinct_roster_line() else {
            return Empty::new().finish();
        };
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();
        let mut span = ui_builder.span(line).with_soft_wrap();
        if any_in_distress {
            span = span.with_style(UiComponentStyles {
                font_color: Some(theme.terminal_colors().normal.red.into()),
                font_weight: Some(Weight::Semibold),
                ..Default::default()
            });
        }
        span.build().with_margin_top(4.).finish()
    }

    fn precinct_inbox_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some((line, emergency)) = precinct_inbox_line() else {
            return Empty::new().finish();
        };
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();
        let mut span = ui_builder.span(line).with_soft_wrap();
        if emergency {
            span = span.with_style(UiComponentStyles {
                font_color: Some(theme.terminal_colors().normal.red.into()),
                font_weight: Some(Weight::Semibold),
                ..Default::default()
            });
        }
        span.build().with_margin_top(4.).finish()
    }

    fn precinct_latest_dispatch_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some((line, emergency)) = precinct_latest_dispatch_text() else {
            return Empty::new().finish();
        };
        // read_inbox drains every queued message, not just the displayed one,
        // so the label tells officers when they're clearing more than the latest.
        let pending = radio::peek_inbox().len();
        let label = if pending > 1 {
            format!("10-4, all clear ({pending})")
        } else {
            "10-4, copy".to_string()
        };
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();

        let mut dispatch_builder = ui_builder.span(line).with_soft_wrap();
        if emergency {
            dispatch_builder = dispatch_builder.with_style(UiComponentStyles {
                font_color: Some(theme.terminal_colors().normal.red.into()),
                font_weight: Some(Weight::Semibold),
                ..Default::default()
            });
        }
        let dispatch_span = dispatch_builder.build().finish();

        let ack_button = ui_builder
            .button(
                ButtonVariant::Secondary,
                self.ack_dispatch_button_mouse_state.clone(),
            )
            .with_style(UiComponentStyles {
                font_size: Some(12.),
                font_weight: Some(Weight::Semibold),
                border_radius: Some(yarpui::elements::CornerRadius::with_all(
                    yarpui::elements::Radius::Pixels(4.),
                )),
                padding: Some(Coords {
                    top: 4.,
                    bottom: 4.,
                    left: 10.,
                    right: 10.,
                }),
                ..Default::default()
            })
            .with_text_label(label)
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::AckInboxDispatch);
            })
            .finish();

        Container::new(
            Wrap::row()
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_children([
                    dispatch_span,
                    Container::new(ack_button).with_padding_left(8.).finish(),
                ])
                .finish(),
        )
        .with_margin_top(4.)
        .finish()
    }
}

impl SettingsPageMeta for AboutPageView {
    fn section() -> SettingsSection {
        SettingsSection::About
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<AboutPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<AboutPageView>) -> Self {
        SettingsPageViewHandle::About(view_handle)
    }
}

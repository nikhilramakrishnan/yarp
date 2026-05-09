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
    stand_down_button_mouse_state: MouseStateHandle,
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
                // Identity row. Reads as a radio sign-on ("Officer X · on patrol")
                // rather than a label-value pair so it pairs with the dot-rhythm
                // of the precinct stack below. Sits above the status banner so the
                // four precinct rows (banner / roster / inbox / dispatch) render
                // as one contiguous urgent stack on emergency.
                .with_child({
                    let (line, emergency) = self_signon_line();
                    styled_precinct_text_row(appearance, line, emergency, false, 4.)
                })
                .with_child(self.precinct_status_row(appearance))
                .with_child(self.precinct_roster_row(appearance))
                .with_child(self.precinct_inbox_row(appearance))
                .with_child(self.precinct_latest_dispatch_row(appearance))
                .with_child(self.mic_check_row(appearance))
                .with_child(self.direct_dispatch_row(appearance))
                .with_child({
                    let (line, emergency) = sandford_population_line();
                    styled_precinct_text_row(appearance, line, emergency, false, 16.)
                })
                .finish(),
        )
        .finish()
    }
}

fn sandford_population_line() -> (String, bool) {
    let population = radio::peers().len() + 1;
    let mut down: std::collections::HashSet<String> = radio::peek_inbox()
        .iter()
        .filter(|m| dispatch_is_emergency(&m.body))
        .map(|m| m.from_call_sign.clone())
        .collect();
    // Self-mayday counts toward the down list — without this, an officer
    // calling their own 10-13 would see "Population: 1." with no urgency
    // marker even though their signon row is red.
    if radio::self_in_mayday() {
        down.insert(radio::self_call_sign());
    }
    let emergency = !down.is_empty();
    let line = if emergency {
        let suffix = if down.len() == 1 {
            "1 officer down".to_string()
        } else {
            format!("{} officers down", down.len())
        };
        format!(
            "Copyright 2026 Yarp contributors. Sandford. Population: {population} \u{00B7} {suffix}."
        )
    } else {
        format!("Copyright 2026 Yarp contributors. Sandford. Population: {population}.")
    };
    (line, emergency)
}

fn self_signon_line() -> (String, bool) {
    let sign = radio::self_call_sign();
    let self_calling = radio::self_in_mayday();
    let inbox_emergency = radio::peek_inbox()
        .iter()
        .any(|m| dispatch_is_emergency(&m.body));
    // Two distinct urgent states feed the signon:
    //   self-mayday   — *we* broadcast a 10-13, peers may not have responded yet
    //   inbox emergency — *they* broadcast a 10-13, we're the responder
    // Self-mayday outranks responder framing because your own emergency is
    // the more pressing duty state. Both flip the row to the red rhythm.
    let status = if self_calling {
        "calling 10-13"
    } else if inbox_emergency {
        "responding to 10-13"
    } else {
        "on patrol"
    };
    let prefix = if sign.starts_with("Officer-") {
        sign
    } else {
        format!("Officer {sign}")
    };
    let emergency = self_calling || inbox_emergency;
    (format!("{prefix} \u{00B7} {status}."), emergency)
}

fn styled_precinct_text_row(
    appearance: &Appearance,
    line: String,
    emergency: bool,
    semibold_when_routine: bool,
    margin_top: f32,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let ui_builder = appearance.ui_builder();
    let mut span = ui_builder.span(line).with_soft_wrap();
    if emergency {
        span = span.with_style(UiComponentStyles {
            font_color: Some(theme.terminal_colors().normal.red.into()),
            font_weight: Some(Weight::Semibold),
            ..Default::default()
        });
    } else if semibold_when_routine {
        span = span.with_style(UiComponentStyles {
            font_weight: Some(Weight::Semibold),
            ..Default::default()
        });
    }
    span.build().with_margin_top(margin_top).finish()
}

fn precinct_status_line() -> (String, bool) {
    let inbox_emergencies = radio::peek_inbox()
        .iter()
        .filter(|m| dispatch_is_emergency(&m.body))
        .count();
    // Self-mayday counts toward the banner's emergency tally — the
    // originator is in distress even though their own broadcast never
    // self-delivers, so the banner needs to read Code 3 the moment they
    // hit 10-13. Otherwise the operator's own signon row goes red while
    // the banner above it stays Code 4 — split signal at the top of stack.
    let self_mayday = if radio::self_in_mayday() { 1 } else { 0 };
    let emergency_count = inbox_emergencies + self_mayday;
    let peer_count = radio::peers().len();
    if emergency_count > 0 {
        // Code 3 fragments collapse to terse counts — banner context already
        // implies "pending" and the channel audience, so dropping the nouns
        // keeps the emergency line scannable instead of running long.
        let phrase = if emergency_count == 1 {
            "1 emergency".to_string()
        } else {
            format!("{emergency_count} emergencies")
        };
        let line = if peer_count > 0 {
            format!("Code 3 \u{00B7} {phrase} \u{00B7} {peer_count} on channel")
        } else {
            format!("Code 3 \u{00B7} {phrase}")
        };
        return (line, true);
    }
    // Code 4 keeps the longer noun phrase — without an emergency fragment to
    // anchor the line, "Code 4 · 4" alone reads cryptic; the full phrasing
    // earns its width.
    let line = match peer_count {
        0 => "Code 4 \u{00B7} sole patrol".to_string(),
        1 => "Code 4 \u{00B7} all clear \u{00B7} 1 other officer on channel".to_string(),
        n => format!("Code 4 \u{00B7} all clear \u{00B7} {n} other officers on channel"),
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
    // Promote distressed peers to the front so when the roster overflows the
    // MAX cap below, the (10-13) badge never gets truncated off the line.
    let mut sorted_peers = peer_list.clone();
    sorted_peers.sort_by_key(|p| !emergency_signs.contains(&p.call_sign));
    let mut names: Vec<String> = sorted_peers
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
    // Roster row picks up the same rhythm-shift the inbox/dispatch rows make
    // when emergency is active. Routine keeps the colon-led 'Roster: …'
    // shape; emergency leads with a middle-dot ('Roster · …') and lifts the
    // distress count out of a parenthetical into its own dot-segment so the
    // four precinct rows scan as one rhythm-pair (parenthetical/colon =
    // routine, dot-rhythm = urgent).
    let names_joined = names.join(", ");
    let body = if overflow > 0 {
        format!("{names_joined} (+{overflow} more)")
    } else {
        names_joined
    };
    let line = if any_in_distress {
        if distress_count >= 2 {
            format!("Roster \u{00B7} {distress_count} in distress \u{00B7} {body}")
        } else {
            format!("Roster \u{00B7} {body}")
        }
    } else {
        format!("Roster: {body}")
    };
    Some((line, any_in_distress))
}

fn precinct_inbox_line() -> Option<(String, bool)> {
    let inbox = radio::peek_inbox();
    if inbox.is_empty() {
        return None;
    }
    // Any pending 10-13 turns the whole inbox line red — emergency dominates routine.
    let emergency = inbox.iter().any(|m| dispatch_is_emergency(&m.body));
    // Track which senders have a 10-13 in queue so the roster can name the
    // officer(s) who need backup rather than just lighting the whole line red.
    let mut distressed: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Count repeats per sender so a chatty officer doesn't crowd the line,
    // and preserve arrival order for predictable rendering.
    let mut order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for msg in &inbox {
        if dispatch_is_emergency(&msg.body) {
            distressed.insert(msg.from_call_sign.clone());
        }
        if seen.insert(msg.from_call_sign.as_str()) {
            order.push(msg.from_call_sign.clone());
        }
    }
    // Promote distressed senders to the front so the eye lands on who needs
    // backup before scanning the rest of the roster — within each group we
    // keep arrival order so render stays stable.
    order.sort_by_key(|s| !distressed.contains(s));
    let distinct = order.len();
    const MAX: usize = 4;
    let overflow = order.len().saturating_sub(MAX);
    order.truncate(MAX);
    // Tag distressed senders inline with the 10-13 code so a multi-sender
    // inbox doesn't leave the operator guessing which officer triggered the
    // red — matches the latest-dispatch row's "10-13 · <officer>" lead.
    let render_name = |name: &str| -> String {
        if distressed.contains(name) {
            format!("10-13 {name}")
        } else {
            name.to_string()
        }
    };
    let rendered: Vec<String> = order.iter().map(|s| render_name(s)).collect();
    let roster = if overflow > 0 {
        format!("{} +{overflow} more", rendered.join(", "))
    } else {
        rendered.join(", ")
    };
    let total = inbox.len();
    // When this Yarp is mid-10-13 and every reply in the inbox is an en-route
    // ack, this isn't generic traffic — it's backup converging. Frame the
    // count as replies rolling in so the originator reads "help is moving"
    // instead of "you have N dispatches".
    let backup_converging =
        radio::self_in_mayday() && inbox.iter().all(|m| radio::is_en_route_body(&m.body));
    let dispatches = if backup_converging {
        if total == 1 {
            "1 unit en route".to_string()
        } else {
            format!("{total} units en route")
        }
    } else if total == 1 {
        "1 pending dispatch".to_string()
    } else {
        format!("{total} pending dispatches")
    };
    // Inbox row owns volume (count + senders); the dispatch row beneath it
    // owns urgency (10-13 lead) and the row's red tint already flags the
    // emergency, so a textual emergency count here would duplicate signals
    // already present at the banner above and the dispatch row below.
    // "Inbox:" prefix mirrors the "Roster:" label so the stack's section
    // labels read in parallel — both answer "what's in <X>?".
    //
    // On emergency the row picks up the same dot-rhythm shift the dispatch
    // row makes — routine stays parenthetical/conversational, emergency
    // collapses to terse middle-dot fragments. Same shape signal as the
    // dispatch line: different rhythm before different words.
    let line = if emergency {
        if distinct == 1 {
            format!("Inbox: {dispatches} \u{00B7} {roster}")
        } else {
            format!("Inbox: {dispatches} \u{00B7} {distinct} officers \u{00B7} {roster}")
        }
    } else if distinct == 1 {
        format!("Inbox: {dispatches} from {roster}.")
    } else {
        format!("Inbox: {dispatches} from {distinct} officers ({roster}).")
    };
    Some((line, emergency))
}

fn precinct_latest_dispatch_text() -> Option<(String, bool)> {
    // When this Yarp is the 10-13 originator, surface their own broadcast in
    // the dispatch row so the surface stack stays continuous: signon goes red,
    // population counts the operator, and the dispatch row leads with the
    // operator's own 10-13 instead of disappearing while peers haven't replied
    // yet. The synthetic only wins when the inbox is empty — once any peer
    // message lands (en-route reply or otherwise) the real latest_dispatch
    // takes over so the operator sees who's responding.
    if radio::self_in_mayday() && radio::peek_inbox().is_empty() {
        let started = radio::self_mayday_started_at_unix();
        let age = started
            .map(radio::format_dispatch_age)
            .unwrap_or_else(|| "now".to_string());
        let line = format!(
            "10-13 \u{00B7} {} \u{00B7} broadcasting \u{00B7} {age}",
            radio::self_call_sign()
        );
        return Some((line, true));
    }
    let msg = radio::latest_dispatch()?;
    let emergency = dispatch_is_emergency(&msg.body);
    // Hoist the 10-13 prefix out of the quoted body when present — burying
    // urgency inside quotes ("Latest from … : \"10-13 …\"") makes the eye
    // work twice. Lifted form ("10-13 from …") puts the code where it lands.
    let display_body = if emergency {
        let stripped = msg.body.trim_start().strip_prefix("10-13").unwrap_or(&msg.body);
        stripped.trim_start_matches([' ', '\t', ':', '-', '\u{00B7}', ',']).to_string()
    } else {
        msg.body.clone()
    };
    // Cap body length so a chatty officer can't blow out the layout.
    const MAX_BODY: usize = 80;
    let body = if display_body.chars().count() > MAX_BODY {
        let truncated: String = display_body.chars().take(MAX_BODY).collect();
        format!("{truncated}…")
    } else {
        display_body
    };
    let age = radio::format_dispatch_age(msg.sent_at_unix);
    // Emergency rewrites the line's *shape*, not just its lead token, so the
    // urgent dispatch scans with different rhythm than a routine one:
    //   routine:    Latest from Cooper (12s ago): "ten-four"
    //   emergency:  10-13 · Cooper · 12s ago — "need backup"
    // Middle-dot separators echo the Code 3 banner above and read as terse
    // hits; the em-dash before the quote replaces the routine's colon so the
    // eye picks up "different separator → different urgency" before parsing
    // any words. Routine keeps the parenthetical bracket for its slower,
    // conversational rhythm.
    let line = if emergency {
        if body.is_empty() {
            format!("10-13 \u{00B7} {} \u{00B7} {age}", msg.from_call_sign)
        } else {
            format!(
                "10-13 \u{00B7} {} \u{00B7} {age} \u{2014} \u{201C}{body}\u{201D}",
                msg.from_call_sign
            )
        }
    } else if body.is_empty() {
        format!("Latest from {} ({age})", msg.from_call_sign)
    } else {
        format!(
            "Latest from {} ({age}): \u{201C}{body}\u{201D}",
            msg.from_call_sign
        )
    };
    Some((line, emergency))
}

use radio::is_emergency_body as dispatch_is_emergency;

impl AboutPageWidget {
    fn mic_check_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        // No peers on the channel — nothing to broadcast at, with one
        // exception: a self-mayday flag we set with no peers around. Show a
        // standalone Stand-down button in that case so the operator can
        // clear their own urgency without needing peers to ack.
        if radio::peers().is_empty() && !radio::self_in_mayday() {
            return Empty::new().finish();
        }
        // Suppress the routine "Mic check" roll-call during an active 10-13 —
        // pinging the channel for presence while an officer needs backup is a
        // tone-break. The 10-13 button stays so the operator can join the
        // emergency wave without hunting for an unrelated CTA.
        let in_emergency = radio::peek_inbox()
            .iter()
            .any(|m| dispatch_is_emergency(&m.body));
        let self_calling = radio::self_in_mayday();
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

        // Tooltips spell out what each broadcast actually says, so a first-time
        // user knows the difference between a routine roll-call ping and the
        // emergency channel before they hit either button.
        let mic_check_tooltip_builder = ui_builder.clone();
        let ten_thirteen_tooltip_builder = ui_builder.clone();
        let stand_down_tooltip_builder = ui_builder.clone();

        let mic_check = ui_builder
            .button(
                ButtonVariant::Secondary,
                self.mic_check_button_mouse_state.clone(),
            )
            .with_style(radio_button_style.clone())
            .with_text_label("Mic check".to_owned())
            .with_tooltip(move || {
                mic_check_tooltip_builder
                    .tool_tip("Roll-call ping to every Yarp on the channel.".to_owned())
                    .build()
                    .finish()
            })
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
            .with_style(radio_button_style.clone())
            .with_text_label("10-13".to_owned())
            .with_tooltip(move || {
                ten_thirteen_tooltip_builder
                    .tool_tip("10-13 \u{2014} officer needs assistance. Reddens the channel.".to_owned())
                    .build()
                    .finish()
            })
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::TenThirteenBroadcast);
            })
            .finish();

        // Stand-down clears self-mayday and tells the channel the situation
        // is resolved. Outlined variant — it's a de-escalation, not a
        // broadcast at the same urgency tier as the red 10-13 button.
        let stand_down = ui_builder
            .button(
                ButtonVariant::Outlined,
                self.stand_down_button_mouse_state.clone(),
            )
            .with_style(radio_button_style)
            .with_text_label("Stand down".to_owned())
            .with_tooltip(move || {
                stand_down_tooltip_builder
                    .tool_tip(
                        "Clear your 10-13 and broadcast 'situation resolved' to the channel."
                            .to_owned(),
                    )
                    .build()
                    .finish()
            })
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::StandDownMayday);
            })
            .finish();

        let mut row = Wrap::row().with_main_axis_alignment(MainAxisAlignment::Center);
        if self_calling {
            // Originator's row: stand-down leads (highest-priority CTA for
            // them right now), 10-13 stays so they can re-broadcast if no
            // one's responding, mic-check is suppressed since the channel
            // is already lit on their behalf.
            row = row.with_child(stand_down);
            if !radio::peers().is_empty() {
                row = row.with_child(Container::new(ten_thirteen).with_padding_left(8.).finish());
            }
        } else if in_emergency {
            row = row.with_child(ten_thirteen);
        } else {
            row = row.with_child(mic_check);
            row = row.with_child(Container::new(ten_thirteen).with_padding_left(8.).finish());
        }
        Container::new(row.finish())
            .with_margin_top(8.)
            .finish()
    }

    // 1:1 direct-dispatch row — one "Hail Sandford" button per live peer.
    // Drops a "Hail — checking in." into that peer's inbox so an officer can
    // ping a specific unit without lighting up the whole channel. Each render
    // generates fresh MouseStateHandles per peer; the peer set is short-lived
    // (process-bound) so persistent hover state isn't worth the bookkeeping.
    fn direct_dispatch_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let peer_list = radio::peers();
        if peer_list.is_empty() {
            return Empty::new().finish();
        }
        // Peers with a pending 10-13 in our inbox flip to an urgent "Respond"
        // CTA — same surface, but the verb and color match the emergency
        // signal already lit on the roster line.
        let emergency_signs: std::collections::HashSet<String> = radio::peek_inbox()
            .iter()
            .filter(|m| dispatch_is_emergency(&m.body))
            .map(|m| m.from_call_sign.clone())
            .collect();
        let ui_builder = appearance.ui_builder();
        let hail_button_style = UiComponentStyles {
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
        };
        // Same distressed-first sort the roster line uses — keeps the urgent
        // "Respond" CTAs at the head of the wrap row so an operator scanning
        // left-to-right hits the in-distress peers before the routine hails,
        // and a wrapping row never buries a Respond CTA on a second line.
        let mut sorted_peers = peer_list.clone();
        sorted_peers.sort_by_key(|p| !emergency_signs.contains(&p.call_sign));
        let mut row = Wrap::row().with_main_axis_alignment(MainAxisAlignment::Center);
        for peer in sorted_peers {
            let in_distress = emergency_signs.contains(&peer.call_sign);
            let label_call_sign = peer.call_sign.clone();
            // When the peer's tab title is known, fold it into the tooltip so
            // the operator can tell which window they're hailing — buttons stay
            // terse but the hover spells out the case file.
            let tooltip_target = match peer.tab_title.as_deref() {
                Some(title) if !title.is_empty() => {
                    format!("{} ({})", peer.call_sign, title)
                }
                _ => peer.call_sign.clone(),
            };
            let tooltip_builder = ui_builder.clone();
            let to_pid = peer.pid;
            let (variant, label, tooltip) = if in_distress {
                (
                    ButtonVariant::Error,
                    format!("Respond to {label_call_sign}"),
                    format!("Send '10-4, en route' to {tooltip_target}."),
                )
            } else {
                (
                    ButtonVariant::Outlined,
                    format!("Hail {label_call_sign}"),
                    format!("Drop a 'checking in' ping into {tooltip_target}'s inbox."),
                )
            };
            let button = ui_builder
                .button(variant, MouseStateHandle::default())
                .with_style(hail_button_style.clone())
                .with_text_label(label)
                .with_tooltip(move || {
                    tooltip_builder.tool_tip(tooltip.clone()).build().finish()
                })
                .build()
                .on_click(move |ctx, _, _| {
                    if in_distress {
                        ctx.dispatch_typed_action(WorkspaceAction::RadioRespond { to_pid });
                    } else {
                        ctx.dispatch_typed_action(WorkspaceAction::RadioHail { to_pid });
                    }
                })
                .finish();
            row = row.with_child(Container::new(button).with_padding_left(6.).finish());
        }
        Container::new(row.finish()).with_margin_top(6.).finish()
    }

    fn precinct_status_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let (line, emergency) = precinct_status_line();
        styled_precinct_text_row(appearance, line, emergency, true, 16.)
    }

    fn precinct_roster_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some((line, any_in_distress)) = precinct_roster_line() else {
            return Empty::new().finish();
        };
        styled_precinct_text_row(appearance, line, any_in_distress, false, 4.)
    }

    fn precinct_inbox_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some((line, emergency)) = precinct_inbox_line() else {
            return Empty::new().finish();
        };
        styled_precinct_text_row(appearance, line, emergency, false, 4.)
    }

    fn precinct_latest_dispatch_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some((line, emergency)) = precinct_latest_dispatch_text() else {
            return Empty::new().finish();
        };
        // read_inbox drains every queued message, not just the displayed one,
        // so the label tells officers when they're clearing more than the latest.
        // On a 10-13 the ack reads as 'en route' instead of 'copy/all clear' —
        // acknowledging an emergency is responding to it, not just receiving it.
        let pending = radio::peek_inbox().len();
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

        // Synthetic self-broadcast (own 10-13 with no peer messages queued)
        // has nothing to ack — Stand down in mic_check_row owns self-cancel.
        // Render as text-only so the row stays informational instead of
        // surfacing a no-op button.
        if pending == 0 {
            return Container::new(
                Wrap::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_children([dispatch_span])
                    .finish(),
            )
            .with_margin_top(4.)
            .finish();
        }

        let label = match (emergency, pending) {
            (true, 0..=1) => "10-4, en route".to_string(),
            (true, n) => format!("10-4, en route ({n})"),
            (false, 0..=1) => "10-4, copy".to_string(),
            (false, n) => format!("10-4, all clear ({n})"),
        };

        // Mirror the 10-13 broadcast button's red tint when acknowledging an
        // emergency — the broadcast side already uses Error to flag "officer
        // needs assistance"; the response side should carry the same urgency
        // so the dispatch row reads red-on-red instead of red dispatch + grey
        // ack. Routine dispatches stay Secondary so a copy/all-clear ack
        // doesn't look as loud as a 10-13 response.
        let ack_variant = if emergency {
            ButtonVariant::Error
        } else {
            ButtonVariant::Secondary
        };
        let ack_button = ui_builder
            .button(ack_variant, self.ack_dispatch_button_mouse_state.clone())
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

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
                .with_child(self.precinct_earlier_dispatches_row(appearance))
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
    // During *any* active 10-13 (self or peer), count distinct call signs
    // that have acked en route so the population line reflects the backup
    // wave — "1 down · 2 responding" reads as the situation actively being
    // handled. Only counted while emergency is live; on routine traffic an
    // "en route" body is just a stray copy-ack and shouldn't pad the banner.
    let responding: std::collections::HashSet<String> = if emergency {
        radio::peek_inbox()
            .iter()
            .filter(|m| radio::is_en_route_body(&m.body))
            .map(|m| m.from_call_sign.clone())
            .collect()
    } else {
        std::collections::HashSet::new()
    };
    let line = if emergency {
        let down_frag = if down.len() == 1 {
            "1 officer down".to_string()
        } else {
            format!("{} officers down", down.len())
        };
        let suffix = if responding.is_empty() {
            down_frag
        } else if responding.len() == 1 {
            format!("{down_frag} \u{00B7} 1 responding")
        } else {
            format!("{down_frag} \u{00B7} {} responding", responding.len())
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
    // Distinct distressed-peer call signs in our inbox — used both for the
    // signon's emergency flag and to name the peer when only one is in
    // distress ("responding to Danny's 10-13") so the row tells the operator
    // which case file to flip to without scrolling to the dispatch row.
    let distressed_peers: std::collections::HashSet<String> = radio::peek_inbox()
        .iter()
        .filter(|m| dispatch_is_emergency(&m.body))
        .map(|m| m.from_call_sign.clone())
        .collect();
    let inbox_emergency = !distressed_peers.is_empty();
    // Two distinct urgent states feed the signon:
    //   self-mayday   — *we* broadcast a 10-13, peers may not have responded yet
    //   inbox emergency — *they* broadcast a 10-13, we're the responder
    // Self-mayday outranks responder framing because your own emergency is
    // the more pressing duty state. Both flip the row to the red rhythm.
    let responder_label = match distressed_peers.len() {
        1 => {
            // Borrow the single name for a possessive — "responding to Danny's
            // 10-13" beats "responding to 10-13" when there's only one call.
            let name = distressed_peers.iter().next().unwrap();
            format!("responding to {name}'s 10-13")
        }
        n @ (2 | 3) => {
            // Enumerate names for 2-3 distressed peers so the signon row tells
            // the operator which case files to flip to without scrolling — a
            // bare "responding to 10-13" loses the discrimination the inbox
            // already has. Above 3 the row gets crowded and the count phrasing
            // ("responding to 4 10-13s") earns its width back.
            let mut names: Vec<String> = distressed_peers.iter().cloned().collect();
            names.sort();
            let joined = if n == 2 {
                format!("{} & {}", names[0], names[1])
            } else {
                format!("{}, {} & {}", names[0], names[1], names[2])
            };
            format!("responding to {joined}'s 10-13s")
        }
        n => format!("responding to {n} 10-13s"),
    };
    let status: String = if self_calling {
        "calling 10-13".to_string()
    } else if inbox_emergency {
        responder_label
    } else {
        // Routine duty status picks up shift size so the operator gets a
        // glanceable peer count without scrolling to the roster row.
        // "solo patrol" mirrors the Code 4 banner's same fallback when
        // peer_count is 0 — keeps the two surfaces in lockstep on the
        // alone-on-channel framing.
        let peer_count = radio::peers().len();
        match peer_count {
            0 => "solo patrol".to_string(),
            1 => "on patrol \u{00B7} 1 on shift".to_string(),
            n => format!("on patrol \u{00B7} {n} on shift"),
        }
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
    // Pull distressed-peer names off the inbox dedup'd and sorted, then
    // prepend "you" when self-mayday is set so the banner names who's
    // actually in distress instead of collapsing to a bare count. Self
    // counts toward the tally — the originator is in distress even though
    // their broadcast never self-delivers, so the banner needs to read
    // Code 3 the moment they hit 10-13. >3 names crowds the row, fall back
    // to count phrasing.
    let mut distressed_names: Vec<String> = radio::peek_inbox()
        .iter()
        .filter(|m| dispatch_is_emergency(&m.body))
        .map(|m| m.from_call_sign.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    distressed_names.sort();
    if radio::self_in_mayday() {
        // "you" leads so the operator sees their own state first when they
        // are calling alongside peers — symmetric to how the signon row
        // treats self-mayday as the more pressing duty state.
        distressed_names.insert(0, "you".to_string());
    }
    let emergency_count = distressed_names.len();
    let peer_count = radio::peers().len();
    if emergency_count > 0 {
        // Name enumeration mirrors the signon's "responding to {names}'s
        // 10-13s" pattern: 1-3 emergencies surface call signs so the
        // operator can triage from the banner alone; >3 falls back to the
        // numeric phrasing because the row's character budget runs out.
        let phrase = match distressed_names.as_slice() {
            [a] => format!("{a} in 10-13"),
            [a, b] => format!("{a} & {b} in 10-13"),
            [a, b, c] => format!("{a}, {b} & {c} in 10-13"),
            _ => format!("{emergency_count} emergencies"),
        };
        let line = if peer_count > 0 {
            format!("Code 3 \u{00B7} {phrase} \u{00B7} {peer_count} on channel")
        } else {
            format!("Code 3 \u{00B7} {phrase}")
        };
        return (line, true);
    }
    // When the channel is quiet but peers have stand-down broadcasts queued,
    // swap the static "all clear" for "just stood down" so the banner echoes
    // the resolution moment instead of pretending nothing happened. Reads as
    // "the call just closed" rather than "nothing has ever been wrong" — the
    // dispatch row below is still announcing the stand-down and the banner
    // shouldn't contradict it. When two or more distinct senders' latest
    // body is a stand-down, lift the count into the phrase ("2 just stood
    // down") so the banner conveys the wave rather than reading like a
    // single closure. Mirrors the inbox-line stood_down badging: same
    // last-write-wins rule, same "distress wins" precedence by virtue of
    // running only inside the no-emergencies branch.
    let mut latest_body_per_sender: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for msg in radio::peek_inbox() {
        latest_body_per_sender.insert(msg.from_call_sign, msg.body);
    }
    let mut stood_down_names: Vec<String> = latest_body_per_sender
        .iter()
        .filter(|(_, body)| radio::is_stand_down_body(body))
        .map(|(name, _)| name.clone())
        .collect();
    stood_down_names.sort();
    // Name enumeration on the resolution side mirrors the Code 3 distress
    // enumeration: 1-3 stood-down senders surface their call signs so the
    // banner names whose case just closed; >3 collapses to the count phrase
    // because the row's character budget runs out. Singular form keeps
    // "just" — "Cooper just stood down" reads as the active resolution
    // moment — but plural forms drop "just" so "Cooper & Danny stood down"
    // doesn't trip on the awkward "Cooper & Danny just stood down".
    // Hail-aware fallback: when no stand-downs are queued but peers are
    // hailing in, "all clear" reads stale — the channel is alive. Mirror
    // the stood_down name-enumeration pattern (1-3 names, count fallback)
    // so the banner reads "Cooper checking in" rather than the mechanical
    // "1 checking in" — same rhythm as "Cooper just stood down". Stand-downs
    // still win the slot; this only kicks in when stand_down_names is empty.
    let mut hailing_names: Vec<String> = latest_body_per_sender
        .iter()
        .filter(|(_, body)| radio::is_hail_body(body))
        .map(|(name, _)| name.clone())
        .collect();
    hailing_names.sort();
    let calm_phrase: String = match stood_down_names.as_slice() {
        [] => match hailing_names.as_slice() {
            [] => "all clear".to_string(),
            [a] => format!("{a} checking in"),
            [a, b] => format!("{a} & {b} checking in"),
            [a, b, c] => format!("{a}, {b} & {c} checking in"),
            names => format!("{} checking in", names.len()),
        },
        [a] => format!("{a} just stood down"),
        [a, b] => format!("{a} & {b} stood down"),
        [a, b, c] => format!("{a}, {b} & {c} stood down"),
        names => format!("{} just stood down", names.len()),
    };
    // Code 4 keeps the longer noun phrase — without an emergency fragment to
    // anchor the line, "Code 4 · 4" alone reads cryptic; the full phrasing
    // earns its width.
    let line = match peer_count {
        0 => "Code 4 \u{00B7} sole patrol".to_string(),
        1 => format!("Code 4 \u{00B7} {calm_phrase} \u{00B7} 1 other officer on channel"),
        n => format!("Code 4 \u{00B7} {calm_phrase} \u{00B7} {n} other officers on channel"),
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
    // While *any* 10-13 is live (self or peer-originated), tag peers whose
    // en-route reply is queued so the roster reads "who's coming" not just
    // "who's around" — solidarity is strongest when the operator can scan
    // the responder list at a glance. Gating on any-active-emergency (rather
    // than just self-mayday) means a peer-side responder sees the wave of
    // backup converging on the originator's call too, not just their own.
    let any_emergency_active = radio::self_in_mayday() || !emergency_signs.is_empty();
    let en_route_signs: std::collections::HashSet<String> = if any_emergency_active {
        radio::peek_inbox()
            .iter()
            .filter(|m| radio::is_en_route_body(&m.body))
            .map(|m| m.from_call_sign.clone())
            .collect()
    } else {
        std::collections::HashSet::new()
    };
    // Hailing peers earn a "(hailing)" tag in the routine roster — symmetric
    // to the inbox roster's "(hailing)" suffix. Gated on no-emergency-active
    // so the red 10-13/en-route rhythm stays unpolluted; under Code 3 the
    // tab_title fallback is fine because the roster's job under urgency is
    // "who's coming", not "who's chatty". Last-write-wins per sender so a
    // peer who hailed and then sent something else doesn't get tagged.
    let hailing_signs: std::collections::HashSet<String> = if any_emergency_active {
        std::collections::HashSet::new()
    } else {
        let mut latest_body_per_sender: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for msg in radio::peek_inbox() {
            latest_body_per_sender.insert(msg.from_call_sign, msg.body);
        }
        latest_body_per_sender
            .into_iter()
            .filter(|(_, body)| radio::is_hail_body(body))
            .map(|(name, _)| name)
            .collect()
    };
    let distress_count = peer_list
        .iter()
        .filter(|p| emergency_signs.contains(&p.call_sign))
        .count();
    let any_in_distress = distress_count > 0;
    // Promote distressed peers to the front so when the roster overflows the
    // MAX cap below, the (10-13) badge never gets truncated off the line.
    // En-route responders rank just behind distressed peers — a unit actively
    // responding to our 10-13 is more load-bearing than a quiet idle one.
    let mut sorted_peers = peer_list.clone();
    sorted_peers.sort_by_key(|p| {
        (
            !emergency_signs.contains(&p.call_sign),
            !en_route_signs.contains(&p.call_sign),
        )
    });
    let mut names: Vec<String> = sorted_peers
        .iter()
        .map(|p| {
            if emergency_signs.contains(&p.call_sign) {
                format!("{} (10-13)", p.call_sign)
            } else if en_route_signs.contains(&p.call_sign) {
                format!("{} (en route)", p.call_sign)
            } else if hailing_signs.contains(&p.call_sign) {
                format!("{} (hailing)", p.call_sign)
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
    // Track which senders' *most recent* queued message is a stand-down
    // broadcast. resolve_superseded_emergencies drops their 10-13 once the
    // stand-down lands, so a sender showing up here means "their call closed
    // and the resolution is still in your inbox". Symmetric to distressed:
    // distressed badges senders who need backup, stood_down badges senders
    // whose case just resolved — both let the roster name names instead of
    // burying the state in body text.
    let mut stood_down: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Senders whose latest queued body is a routine hail. Mirrors stood_down
    // so the roster can read "Cooper (hailing)" without making the operator
    // pop the body open — keeps the inbox line glanceable when peers are
    // just checking in.
    let mut hailing: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut latest_body_per_sender: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    // Count repeats per sender so a chatty officer doesn't crowd the line,
    // and preserve arrival order for predictable rendering.
    let mut order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for msg in &inbox {
        if dispatch_is_emergency(&msg.body) {
            distressed.insert(msg.from_call_sign.clone());
        }
        latest_body_per_sender.insert(msg.from_call_sign.clone(), msg.body.clone());
        if seen.insert(msg.from_call_sign.as_str()) {
            order.push(msg.from_call_sign.clone());
        }
    }
    // Promote distress over resolution: if a peer is currently calling 10-13,
    // their stand-down badge from a *prior* case is irrelevant — backup is
    // the active state and the roster should read it.
    for (sender, body) in &latest_body_per_sender {
        if distressed.contains(sender) {
            continue;
        }
        if radio::is_stand_down_body(body) {
            stood_down.insert(sender.clone());
        } else if radio::is_hail_body(body) {
            hailing.insert(sender.clone());
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
    // When the entire inbox is one *kind* of routine traffic — all hails or
    // all stand-downs — promote that into the noun ("3 hails from 3 officers"
    // beats "3 pending dispatches from 3 officers (Cooper (hailing), …)") and
    // drop the now-redundant per-name suffix. Emergency rendering owns the
    // distress path, so this only kicks in when no 10-13 is queued.
    let all_hails = !emergency
        && !inbox.is_empty()
        && inbox.iter().all(|m| radio::is_hail_body(&m.body));
    let all_stand_downs = !emergency
        && !inbox.is_empty()
        && inbox.iter().all(|m| radio::is_stand_down_body(&m.body));
    // Tag distressed senders inline with the 10-13 code so a multi-sender
    // inbox doesn't leave the operator guessing which officer triggered the
    // red — matches the latest-dispatch row's "10-13 · <officer>" lead.
    // Stand-down senders pick up a "(stood down)" suffix so a quiet roster
    // line still tells the operator "this name is here because their call
    // just closed, not because they're chatty" — symmetric with the 10-13
    // prefix and matches the earlier-dispatches preview's "stood down" tag.
    // Suffixes drop when the noun already covers the kind (all-hails /
    // all-stand-downs) so the row doesn't read "3 hails from … (Cooper
    // (hailing), Danny (hailing))".
    let render_name = |name: &str| -> String {
        if distressed.contains(name) {
            format!("10-13 {name}")
        } else if stood_down.contains(name) && !all_stand_downs {
            format!("{name} (stood down)")
        } else if hailing.contains(name) && !all_hails {
            format!("{name} (hailing)")
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
    } else if all_stand_downs {
        if total == 1 {
            "1 stand-down".to_string()
        } else {
            format!("{total} stand-downs")
        }
    } else if all_hails {
        if total == 1 {
            "1 hail".to_string()
        } else {
            format!("{total} hails")
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
    // When a 10-13 is active (self or peer-originated) and the latest inbox
    // dispatch is an en-route ack, rewrite the row so the response
    // relationship reads at a glance — readers want "Cooper's coming" not
    // "Cooper said ten-four". When multiple peers are en route, list them
    // all in arrival order (latest first) so the wave reads as converging,
    // not just the most recent ack — "Cooper, Danny en route" beats "Cooper
    // en route" when Danny's also coming. Stays red because the call is
    // still active until stand-down. Gating on any-active-emergency (rather
    // than just self_in_mayday) means peer-side responders see the same
    // converging-wave picture the originator does.
    let emergency_active = radio::self_in_mayday()
        || radio::peek_inbox()
            .iter()
            .any(|m| dispatch_is_emergency(&m.body));
    if emergency_active && radio::is_en_route_body(&msg.body) {
        let mut owned: Vec<radio::Message> = radio::peek_inbox()
            .into_iter()
            .filter(|m| radio::is_en_route_body(&m.body))
            .collect();
        owned.sort_by_key(|m| std::cmp::Reverse(m.sent_at_unix));
        // Dedupe by call sign so a peer who re-acks doesn't get listed twice;
        // preserve latest-first order from the sorted vec.
        let mut seen: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let names: Vec<String> = owned
            .iter()
            .filter_map(|m| {
                if seen.insert(m.from_call_sign.clone()) {
                    Some(m.from_call_sign.clone())
                } else {
                    None
                }
            })
            .collect();
        let age = radio::format_dispatch_age(msg.sent_at_unix);
        let line = if names.len() <= 1 {
            format!(
                "10-4 \u{00B7} {} en route \u{00B7} {age}",
                msg.from_call_sign
            )
        } else {
            format!(
                "10-4 \u{00B7} {} en route \u{00B7} {age}",
                names.join(", ")
            )
        };
        return Some((line, true));
    }
    // Stand-down rendering: when the latest dispatch is a peer's "Stand down
    // — situation resolved." broadcast and no other 10-13 is active, the
    // routine fallthrough renders it as "Latest from Cooper: 'Stand down …'"
    // which buries the resolution semantic inside a quoted body. Pull it
    // forward as "10-4 all clear · Cooper stood down · 12s ago" so the row
    // reads as the call closing rather than yet another piece of routine
    // chatter. Stays not-emergency so the row drops out of red — that's the
    // whole point of the surface, signalling the call is over.
    if !emergency_active && radio::is_stand_down_body(&msg.body) {
        let case_tag: Option<String> = radio::peers()
            .into_iter()
            .find(|p| p.call_sign == msg.from_call_sign)
            .and_then(|p| p.tab_title)
            .filter(|t| !t.is_empty());
        let age = radio::format_dispatch_age(msg.sent_at_unix);
        let from_label = match &case_tag {
            Some(tag) => format!("{} ({tag})", msg.from_call_sign),
            None => msg.from_call_sign.clone(),
        };
        let line = format!(
            "10-4 all clear \u{00B7} {from_label} stood down \u{00B7} {age}"
        );
        return Some((line, false));
    }
    // Hail rendering: collapse the verbatim "Hail — checking in." body into
    // "Hail · Cooper · case-foo · 12s ago" so the dispatch row reads as a
    // calm roll-call rather than a quoted snippet competing with itself.
    // Mirrors the stand-down branch's terse 3-token rhythm. Skipped during
    // any active emergency so a routine ping never displaces the converging
    // wave or distress lead.
    if !emergency_active && radio::is_hail_body(&msg.body) {
        let case_tag: Option<String> = radio::peers()
            .into_iter()
            .find(|p| p.call_sign == msg.from_call_sign)
            .and_then(|p| p.tab_title)
            .filter(|t| !t.is_empty());
        let age = radio::format_dispatch_age(msg.sent_at_unix);
        let from_label = match &case_tag {
            Some(tag) => format!("{} \u{00B7} {tag}", msg.from_call_sign),
            None => msg.from_call_sign.clone(),
        };
        let line = format!("Hail \u{00B7} {from_label} \u{00B7} {age}");
        return Some((line, false));
    }
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
    // Look up the originator's tab title so the dispatch row carries case-file
    // context — "Latest from Cooper · case-foo" beats "Latest from Cooper"
    // when the responder is triaging which window to flip to. Skipped when
    // the peer's tab title is unset/empty so we don't render a dangling
    // separator. On emergency the tag rides as "(case-foo)" to mirror the
    // hail-button tooltip's bracket, and on routine it folds inside the
    // existing parenthetical alongside age.
    let case_tag: Option<String> = radio::peers()
        .into_iter()
        .find(|p| p.call_sign == msg.from_call_sign)
        .and_then(|p| p.tab_title)
        .filter(|t| !t.is_empty());
    // Emergency rewrites the line's *shape*, not just its lead token, so the
    // urgent dispatch scans with different rhythm than a routine one:
    //   routine:    Latest from Cooper · case-foo (12s ago): "ten-four"
    //   emergency:  10-13 · Cooper (case-foo) · 12s ago — "need backup"
    // Middle-dot separators echo the Code 3 banner above and read as terse
    // hits; the em-dash before the quote replaces the routine's colon so the
    // eye picks up "different separator → different urgency" before parsing
    // any words. Routine keeps the parenthetical bracket for its slower,
    // conversational rhythm.
    let from_label = match &case_tag {
        Some(tag) if emergency => format!("{} ({tag})", msg.from_call_sign),
        _ => msg.from_call_sign.clone(),
    };
    let routine_meta = match &case_tag {
        Some(tag) => format!("{tag} · {age}"),
        None => age.clone(),
    };
    let line = if emergency {
        if body.is_empty() {
            format!("10-13 \u{00B7} {from_label} \u{00B7} {age}")
        } else {
            format!(
                "10-13 \u{00B7} {from_label} \u{00B7} {age} \u{2014} \u{201C}{body}\u{201D}"
            )
        }
    } else if body.is_empty() {
        format!("Latest from {} ({routine_meta})", msg.from_call_sign)
    } else {
        format!(
            "Latest from {} ({routine_meta}): \u{201C}{body}\u{201D}",
            msg.from_call_sign
        )
    };
    Some((line, emergency))
}

use radio::is_emergency_body as dispatch_is_emergency;

// Compact preview of dispatches sitting *behind* the latest one. The
// latest-dispatch row already shows its body and sender; this surface fills
// the gap between "1 unread shown" and the inbox-row's volume tally by
// naming the next 1-2 senders and a short snippet so the operator can tell
// whether the queue is "Cooper said the same thing twice" vs "three
// different officers all chiming in" without acking and reading.
//
// Skipped when only the latest is queued (no "earlier" exists), or when
// every queued message is an en-route ack during a self-10-13 — in that
// frame the dispatch row already enumerates responders, so an "earlier"
// line would just repeat names.
fn precinct_earlier_dispatches_line() -> Option<String> {
    let inbox = radio::peek_inbox();
    if inbox.len() < 2 {
        return None;
    }
    // Skip the preview when *every* queued message is an en-route ack during
    // any active 10-13 (self or peer-originated) — the dispatch row above
    // already enumerates the responder wave, so an "Earlier:" line under it
    // would just repeat names. Broadening the gate beyond self-mayday means
    // peer-side responders don't see a redundant "Earlier: Danny (case-foo
    // · 12s) — '10-4 en route'" line under their dispatch row's
    // "Cooper, Danny en route".
    let any_emergency_active = radio::self_in_mayday()
        || inbox.iter().any(|m| dispatch_is_emergency(&m.body));
    if any_emergency_active && inbox.iter().all(|m| radio::is_en_route_body(&m.body)) {
        return None;
    }
    // Walk newest-first, skip the absolute newest (already shown above), and
    // grab up to two earlier ones, deduped by sender so a chatty officer
    // doesn't crowd the preview.
    let mut sorted: Vec<radio::Message> = inbox;
    sorted.sort_by_key(|m| std::cmp::Reverse(m.sent_at_unix));
    let mut sorted_iter = sorted.into_iter();
    let _newest = sorted_iter.next();
    // Promote stand-down fragments ahead of routine chatter so the preview
    // leads with "Cooper (case-foo · 12s) — stood down" before any quoted
    // chatter snippet. Symmetric to the inbox-line "(stood down)" badge:
    // resolutions deserve top billing in the secondary surface, and
    // recency-only ordering buries them when a chatty peer fires after the
    // close. Recency from the initial sort is preserved within each tier.
    // Three-tier ordering: stand-downs first (resolution headlines), then
    // substantive chatter (anything with body text the operator hasn't seen
    // collapsed away), then routine hails last. Hails carry the lowest
    // signal — "checking in" never beats a quoted snippet — so when the
    // 2-frag budget is tight, a hail should yield to substantive content.
    let (stand_downs, rest): (Vec<_>, Vec<_>) = sorted_iter
        .partition(|m| radio::is_stand_down_body(&m.body));
    let (hails, others): (Vec<_>, Vec<_>) = rest
        .into_iter()
        .partition(|m| radio::is_hail_body(&m.body));
    let iter = stand_downs.into_iter().chain(others).chain(hails);
    let case_tags: std::collections::HashMap<String, String> = radio::peers()
        .into_iter()
        .filter_map(|p| {
            p.tab_title
                .filter(|t| !t.is_empty())
                .map(|t| (p.call_sign, t))
        })
        .collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut frags: Vec<String> = Vec::new();
    for msg in iter {
        if !seen.insert(msg.from_call_sign.clone()) {
            continue;
        }
        let age = radio::format_dispatch_age(msg.sent_at_unix);
        let snippet: String = msg.body.chars().take(28).collect();
        let snippet = if msg.body.chars().count() > 28 {
            format!("{snippet}…")
        } else {
            snippet
        };
        let case_meta = match case_tags.get(&msg.from_call_sign) {
            Some(tag) => format!("{tag} \u{00B7} {age}"),
            None => age.clone(),
        };
        // Stand-down and hail fragments collapse to "{name} ({case_meta}) —
        // {tag}" rather than quoting the verbatim body. The reader cares
        // that the call closed (or that someone hailed); the literal text
        // ("Stand down — situation resolved." / "Hail — checking in.") is
        // decoration that pushes more useful fragments out of the row's
        // character budget.
        let frag = if radio::is_stand_down_body(&msg.body) {
            format!("{} ({case_meta}) \u{2014} stood down", msg.from_call_sign)
        } else if radio::is_hail_body(&msg.body) {
            format!("{} ({case_meta}) \u{2014} hail", msg.from_call_sign)
        } else if snippet.is_empty() {
            format!("{} ({case_meta})", msg.from_call_sign)
        } else {
            format!(
                "{} ({case_meta}) \u{2014} \u{201C}{snippet}\u{201D}",
                msg.from_call_sign
            )
        };
        frags.push(frag);
        if frags.len() >= 2 {
            break;
        }
    }
    if frags.is_empty() {
        return None;
    }
    Some(format!("Earlier: {}", frags.join("; ")))
}

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
        // Last-write-wins per sender so we know which peers are *currently*
        // hailing us. Drives the "Hail back" CTA flip below — closes the
        // loop on the inbox roster's "(hailing)" suffix so the response
        // verb reads as a reply rather than an unprompted ping.
        let hailing_signs: std::collections::HashSet<String> = {
            let mut latest_body_per_sender: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for msg in radio::peek_inbox() {
                latest_body_per_sender.insert(msg.from_call_sign, msg.body);
            }
            latest_body_per_sender
                .into_iter()
                .filter(|(name, body)| {
                    !emergency_signs.contains(name) && radio::is_hail_body(body)
                })
                .map(|(name, _)| name)
                .collect()
        };
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
            let is_hailing_us = hailing_signs.contains(&peer.call_sign);
            let (variant, label, tooltip) = if in_distress {
                (
                    ButtonVariant::Error,
                    format!("Respond to {label_call_sign}"),
                    format!("Send '10-4, en route' to {tooltip_target}."),
                )
            } else if is_hailing_us {
                (
                    ButtonVariant::Outlined,
                    format!("Hail back {label_call_sign}"),
                    format!("Reply to {tooltip_target}'s hail with one of your own."),
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

    fn precinct_earlier_dispatches_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some(line) = precinct_earlier_dispatches_line() else {
            return Empty::new().finish();
        };
        // Routine styling — earlier dispatches are *context*, not the
        // urgent surface; the latest-dispatch row above already owns red.
        // Tighter top margin so the preview reads as a continuation of the
        // dispatch row rather than a fresh section.
        styled_precinct_text_row(appearance, line, false, false, 2.)
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

        // When the latest dispatch is a stand-down broadcast, the ack-button
        // verb shifts from generic "copy" to "all clear" so it matches the
        // dispatch row's resolution framing — the row reads "10-4 all clear ·
        // Cooper stood down" up top and the ack reads "10-4, all clear" below.
        // Without this, single-pending stand-downs got "10-4, copy" which
        // softens the resolution vibe the row is trying to project.
        let stand_down_dispatch = radio::latest_dispatch()
            .map(|m| radio::is_stand_down_body(&m.body))
            .unwrap_or(false);
        let label = match (emergency, stand_down_dispatch, pending) {
            (true, _, 0..=1) => "10-4, en route".to_string(),
            (true, _, n) => format!("10-4, en route ({n})"),
            (false, true, 0..=1) => "10-4, all clear".to_string(),
            (false, _, 0..=1) => "10-4, copy".to_string(),
            (false, _, n) => format!("10-4, all clear ({n})"),
        };

        // Mirror the 10-13 broadcast button's red tint when acknowledging an
        // emergency — the broadcast side already uses Error to flag "officer
        // needs assistance"; the response side should carry the same urgency
        // so the dispatch row reads red-on-red instead of red dispatch + grey
        // ack. Stand-down acks drop to Outlined to match the de-escalation
        // tier of the stand-down broadcast button — the row already reads
        // "10-4 all clear · Cooper stood down" with a calmer label, so the
        // button shouldn't compete by sitting at full Secondary weight.
        // Routine dispatches stay Secondary so a copy ack doesn't disappear.
        let ack_variant = if emergency {
            ButtonVariant::Error
        } else if stand_down_dispatch {
            ButtonVariant::Outlined
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

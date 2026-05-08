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
    pub fn new(_ctx: &mut ViewContext<AboutPageView>) -> Self {
        AboutPageView {
            page: PageType::new_monolith(AboutPageWidget::default(), None, false),
        }
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
                .with_child(
                    ui_builder
                        .span(format!("On the air as: {}", radio::self_call_sign()))
                        .build()
                        .with_margin_top(16.)
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .span(precinct_population_line())
                        .build()
                        .with_margin_top(4.)
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .span(precinct_roster_line())
                        .build()
                        .with_margin_top(4.)
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .span(precinct_inbox_line())
                        .build()
                        .with_margin_top(4.)
                        .finish(),
                )
                .with_child(self.precinct_latest_dispatch_row(appearance))
                .with_child(self.mic_check_row(appearance))
                .with_child(
                    ui_builder
                        .span("Copyright 2026 Yarp contributors. Sandford. Population: 1.")
                        .build()
                        .with_margin_top(16.)
                        .finish(),
                )
                .finish(),
        )
        .finish()
    }
}

fn precinct_population_line() -> String {
    let peer_count = radio::peers().len();
    match peer_count {
        0 => "Sole officer on the channel.".to_string(),
        1 => "1 other officer on the channel.".to_string(),
        n => format!("{n} other officers on the channel."),
    }
}

fn precinct_roster_line() -> String {
    let peer_list = radio::peers();
    if peer_list.is_empty() {
        return String::new();
    }
    let mut names: Vec<String> = peer_list
        .iter()
        .map(|p| match &p.tab_title {
            Some(title) => format!("{} ({})", p.call_sign, title),
            None => p.call_sign.clone(),
        })
        .collect();
    // Cap rendered names; trailing "+N more" if oversized.
    const MAX: usize = 5;
    let overflow = names.len().saturating_sub(MAX);
    names.truncate(MAX);
    if overflow > 0 {
        format!("Roster: {} (+{overflow} more)", names.join(", "))
    } else {
        format!("Roster: {}", names.join(", "))
    }
}

fn precinct_inbox_line() -> String {
    let pending = radio::peek_inbox().len();
    match pending {
        0 => String::new(),
        1 => "1 pending dispatch in the inbox.".to_string(),
        n => format!("{n} pending dispatches in the inbox."),
    }
}

fn precinct_latest_dispatch_text() -> Option<String> {
    let msg = radio::latest_dispatch()?;
    // Cap body length so a chatty officer can't blow out the layout.
    const MAX_BODY: usize = 80;
    let body = if msg.body.chars().count() > MAX_BODY {
        let truncated: String = msg.body.chars().take(MAX_BODY).collect();
        format!("{truncated}…")
    } else {
        msg.body.clone()
    };
    Some(format!(
        "Latest from {}: \u{201C}{body}\u{201D}",
        msg.from_call_sign
    ))
}

impl AboutPageWidget {
    fn mic_check_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        // No peers on the channel — nothing to broadcast at.
        if radio::peers().is_empty() {
            return Empty::new().finish();
        }
        let ui_builder = appearance.ui_builder();
        let button = ui_builder
            .button(
                ButtonVariant::Secondary,
                self.mic_check_button_mouse_state.clone(),
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
                    left: 12.,
                    right: 12.,
                }),
                ..Default::default()
            })
            .with_text_label("Mic check".to_owned())
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::MicCheckBroadcast);
            })
            .finish();
        Container::new(button).with_margin_top(8.).finish()
    }

    fn precinct_latest_dispatch_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let Some(line) = precinct_latest_dispatch_text() else {
            return Empty::new().finish();
        };
        let ui_builder = appearance.ui_builder();

        let dispatch_span = ui_builder.span(line).with_soft_wrap().build().finish();

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
            .with_text_label("10-4, copy".to_owned())
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

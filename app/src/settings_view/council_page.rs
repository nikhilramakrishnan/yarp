use std::path::PathBuf;

use yarpui::{
    elements::{
        Align, Container, CornerRadius, CrossAxisAlignment, Element, Flex, Hoverable,
        MainAxisAlignment, MouseStateHandle, ParentElement, Radius,
    },
    platform::Cursor,
    ui_components::components::{UiComponent, UiComponentStyles},
    AppContext, Entity, View, ViewContext, ViewHandle,
};

use crate::{
    appearance::Appearance, personas::Roster, workspace::WorkspaceAction,
};

use super::{
    settings_page::{
        MatchData, PageType, SettingsPageEvent, SettingsPageMeta, SettingsPageViewHandle,
        SettingsWidget,
    },
    SettingsSection,
};

pub struct CouncilPageView {
    page: PageType<Self>,
}

impl CouncilPageView {
    pub fn new(_ctx: &mut ViewContext<CouncilPageView>) -> Self {
        CouncilPageView {
            page: PageType::new_monolith(CouncilPageWidget::default(), None, false),
        }
    }
}

impl Entity for CouncilPageView {
    type Event = SettingsPageEvent;
}

impl View for CouncilPageView {
    fn ui_name() -> &'static str {
        "CouncilPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

#[derive(Default)]
struct CouncilPageWidget {
    open_button_mouse: MouseStateHandle,
    reload_button_mouse: MouseStateHandle,
}

fn personas_path() -> Option<PathBuf> {
    Some(yarp_core::paths::yarp_home_config_dir()?.join("personas.json"))
}

fn pill_button(
    label: &'static str,
    mouse_state: MouseStateHandle,
    appearance: &Appearance,
    on_click: impl Fn(&mut yarpui::EventContext) + Clone + 'static,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg_idle = theme.surface_3();
    let bg_hover = theme.surface_2();
    let ui_builder = appearance.ui_builder();
    Hoverable::new(mouse_state, move |state| {
        let bg = if state.is_hovered() { bg_hover } else { bg_idle };
        Container::new(
            ui_builder
                .label(label)
                .build()
                .finish(),
        )
        .with_horizontal_padding(12.)
        .with_vertical_padding(6.)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_background(bg)
        .finish()
    })
    .with_cursor(Cursor::PointingHand)
    .on_click(move |ctx, _, _| on_click(ctx))
    .finish()
}

impl SettingsWidget for CouncilPageWidget {
    type View = CouncilPageView;

    fn search_terms(&self) -> &str {
        "council squad agents personas sandford nwa team"
    }

    fn render(
        &self,
        _view: &CouncilPageView,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();

        let header = ui_builder
            .span("Sandford NWA")
            .with_style(UiComponentStyles::default().set_font_size(20.))
            .build()
            .finish();

        let subhead = ui_builder
            .paragraph("Assemble the team that convenes when /agent is invoked. Edit personas.json to rewrite voices, add officers, or stand up a new squad.")
            .build()
            .with_margin_top(6.)
            .finish();

        let path_string = personas_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(personas.json path unavailable)".into());

        let path_label = ui_builder
            .span(path_string)
            .build()
            .with_margin_top(12.)
            .finish();

        let path_for_open = personas_path();
        let open_button = pill_button(
            "Open personas.json",
            self.open_button_mouse.clone(),
            appearance,
            move |ctx| {
                if let Some(path) = path_for_open.clone() {
                    ctx.dispatch_typed_action(WorkspaceAction::OpenFilePath { path });
                }
            },
        );
        let reload_button = pill_button(
            "Reload from disk",
            self.reload_button_mouse.clone(),
            appearance,
            |ctx| ctx.notify(),
        );

        let button_row = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_child(open_button)
            .with_child(
                Container::new(reload_button)
                    .with_margin_left(8.)
                    .finish(),
            )
            .finish();

        let roster_block: Box<dyn Element> = match Roster::load() {
            Some(roster) => {
                let mut col =
                    Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                let default_team_name = roster.default_team.clone();
                for team in &roster.teams {
                    let is_default = team.name == default_team_name;
                    let label = if is_default {
                        format!("{} — default", team.name)
                    } else {
                        team.name.clone()
                    };
                    col = col.with_child(
                        ui_builder
                            .span(label)
                            .with_style(UiComponentStyles::default().set_font_size(15.))
                            .build()
                            .with_margin_top(16.)
                            .finish(),
                    );
                    for p in &team.members {
                        let lead_marker = if p.lead { "★ " } else { "   " };
                        let cli_marker = if p.binary.is_some() {
                            "  [CLI]"
                        } else {
                            ""
                        };
                        let badge_name =
                            format!("{}{} {}{}", lead_marker, p.badge, p.name, cli_marker);
                        let role_voice = format!("{}  {}", p.role, p.voice);
                        let mut card_col = Flex::column()
                            .with_cross_axis_alignment(CrossAxisAlignment::Start)
                            .with_child(ui_builder.span(badge_name).build().finish())
                            .with_child(
                                ui_builder
                                    .paragraph(role_voice)
                                    .build()
                                    .with_margin_top(4.)
                                    .finish(),
                            );
                        if let Some(bin) = &p.binary {
                            card_col = card_col.with_child(
                                ui_builder
                                    .span(format!("→ {bin}"))
                                    .build()
                                    .with_margin_top(4.)
                                    .finish(),
                            );
                        }
                        let card = Container::new(card_col.finish())
                            .with_uniform_padding(10.)
                            .with_margin_top(8.)
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                            .with_background(theme.surface_3())
                            .finish();
                        col = col.with_child(card);
                    }
                }
                col.finish()
            }
            None => ui_builder
                .paragraph("No personas.json found. Click Reload after creating the file, or restart Yarp to drop the default Sandford roster.")
                .build()
                .with_margin_top(16.)
                .finish(),
        };

        let body = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(header)
            .with_child(subhead)
            .with_child(path_label)
            .with_child(
                Container::new(button_row)
                    .with_margin_top(10.)
                    .finish(),
            )
            .with_child(roster_block)
            .finish();

        Align::new(
            Container::new(body)
                .with_uniform_padding(16.)
                .finish(),
        )
        .finish()
    }
}

impl SettingsPageMeta for CouncilPageView {
    fn section() -> SettingsSection {
        SettingsSection::Council
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

impl From<ViewHandle<CouncilPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<CouncilPageView>) -> Self {
        SettingsPageViewHandle::Council(view_handle)
    }
}

use asset_macro::bundled_asset;
use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use yarp_core::ui::theme::YarpTheme;
use yarpui::assets::asset_cache::{AssetCache, AssetState};

use crate::ai::blocklist::inline_action::requested_action::RenderableAction;
use crate::appearance::Appearance;
use crate::terminal::shell::ShellType;
use crate::terminal::yarpify;
use crate::terminal::yarpify::render::SSH_DOCS_URL;
use crate::ui_components::icons::Icon as UiIcon;
use yarpui::elements::{HighlightedHyperlink, Hoverable, Icon, MouseStateHandle};
use yarpui::keymap::FixedBinding;
use yarpui::AppContext;
use yarpui::{
    elements::{Border, Container, CrossAxisAlignment, Flex, ParentElement},
    Element, Entity, SingletonEntity, TypedActionView, View, ViewContext,
};

#[derive(Debug, Clone)]
pub enum SshYarpifyBlockEvent {
    YarpifySession,
    Cancel,
    Interrupt,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SshYarpifyBlockAction {
    Interrupt,
    Focus,
}

pub struct SshYarpifyBlock {
    block_mouse_state: MouseStateHandle,
    ssh_command: String,
}

pub fn init(app: &mut AppContext) {
    use yarpui::keymap::macros::*;

    app.register_fixed_bindings([FixedBinding::new(
        "ctrl-c",
        SshYarpifyBlockAction::Interrupt,
        id!(SshYarpifyBlock::ui_name()),
    )]);
}

impl SshYarpifyBlock {
    #[allow(clippy::new_without_default)]
    pub fn new(ssh_command: String) -> Self {
        Self {
            block_mouse_state: Default::default(),
            ssh_command,
        }
    }

    pub fn focus(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
        ctx.notify();
    }
}

impl Entity for SshYarpifyBlock {
    type Event = SshYarpifyBlockEvent;
}

impl SshYarpifyBlock {
    fn render_title_ui(&self, theme: &YarpTheme, appearance: &Appearance) -> Box<dyn Element> {
        let icon = Icon::new(UiIcon::Yarp.into(), theme.active_ui_detail());
        yarpify::render::header_row("Yarpifying SSH Session...", icon, theme, appearance)
    }
}

pub fn yarpify_description(
    app: &AppContext,
    hyperlink_index: &HighlightedHyperlink,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    let description = FormattedText::new(vec![FormattedTextLine::Line(vec![
        FormattedTextFragment::plain_text(
            "Bring Yarp's features to your remote session. Blocks, full text editing, auto-complete, Fuzz, and more. "
        ),
        FormattedTextFragment::hyperlink("Pull the dossier", SSH_DOCS_URL),
    ])]);
    yarpify::render::build_description_row(description, theme, appearance, hyperlink_index.clone())
        .with_hyperlink_font_color(appearance.theme().accent().into_solid())
        .register_default_click_handlers(|url, _, ctx| {
            ctx.open_url(&url.url);
        })
        .finish()
}

impl View for SshYarpifyBlock {
    fn ui_name() -> &'static str {
        "SshYarpifyBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut content = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        content.add_child(self.render_title_ui(theme, appearance));

        content.add_child(
            Container::new(
                RenderableAction::new(&self.ssh_command, app)
                    .with_background_color(theme.background().into_solid())
                    .render(app)
                    .finish(),
            )
            .with_margin_top(16.)
            .finish(),
        );

        Hoverable::new(self.block_mouse_state.clone(), |_| {
            Container::new(content.finish())
                .with_padding_top(10.)
                .with_background(theme.foreground().with_opacity(10))
                .with_border(Border::top(1.).with_border_fill(theme.outline()))
                .finish()
        })
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(SshYarpifyBlockAction::Focus);
        })
        .finish()
    }
}

impl TypedActionView for SshYarpifyBlock {
    type Action = SshYarpifyBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            SshYarpifyBlockAction::Interrupt => {
                ctx.emit(SshYarpifyBlockEvent::Interrupt);
            }
            SshYarpifyBlockAction::Focus => {
                self.focus(ctx);
            }
        }
    }
}

/// Convert the begin_yarpify_ssh_session script into a string.
pub fn begin_yarpify_ssh_session_command(app: &AppContext) -> String {
    let asset = bundled_asset!("bootstrap/unknown_init_subshell.sh");

    match AssetCache::as_ref(app).load_asset::<String>(asset) {
        AssetState::Loaded { data } => data.to_string().replace("HOOK_NAME", "InitSsh"),
        _ => panic!("ssh begin yarpify script should be available as a string"),
    }
}

/// Convert the yarpify_ssh_session script into a string.
pub fn yarpify_ssh_session_command(
    uname: &str,
    shell_type: ShellType,
    app: &AppContext,
) -> Option<String> {
    let asset = match (uname, shell_type) {
        // Mac scripts must be less than 1020 characters due to macOS 15+ pty issue
        ("Darwin", ShellType::Zsh | ShellType::Bash) => {
            bundled_asset!("ssh/bash_zsh/yarpify_ssh_session_mac.sh")
        }
        // Mac scripts must be less than 1020 characters due to macOS 15+ pty issue
        ("Darwin", ShellType::Fish) => bundled_asset!("ssh/fish/yarpify_ssh_session_mac.sh"),
        (_, ShellType::Zsh | ShellType::Bash) => {
            bundled_asset!("ssh/bash_zsh/yarpify_ssh_session.sh")
        }
        (_, ShellType::Fish) => bundled_asset!("ssh/fish/yarpify_ssh_session.sh"),
        // PowerShell is not supported yet.
        (_, ShellType::PowerShell) => return None,
    };

    // Todo(Jack): look into avoiding an allocation here.
    match AssetCache::as_ref(app).load_asset::<String>(asset) {
        AssetState::Loaded { data } => Some(data.to_string()),
        _ => panic!("ssh yarpify script should be available as a string"),
    }
}
#[cfg(test)]
#[path = "yarpify_test.rs"]
mod tests;

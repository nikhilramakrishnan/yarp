use std::sync::Arc;

use parking_lot::FairMutex;
use yarpui::prelude::Empty;
use yarpui::{
    elements::{
        ChildView, Container, CrossAxisAlignment, Expanded, Flex, MainAxisSize, ParentElement,
    },
    AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::{
    terminal::view::{TerminalModel, PADDING_LEFT},
    ui_components::icons::Icon,
    view_components::action_button::{ActionButton, ButtonSize, KeystrokeSource, TooltipAlignment},
};

use super::{AgentFooterButtonTheme, USE_AGENT_KEYSTROKE};
use crate::terminal::view::block_banner::YarpificationMode;

/// Footer view rendered for detected subshell/SSH commands, offering both
/// "Yarpify" and "Use agent" buttons in a horizontal row.
pub(super) struct YarpifyFooterView {
    terminal_model: Arc<FairMutex<TerminalModel>>,
    yarpify_button: ViewHandle<ActionButton>,
    use_agent_button: ViewHandle<ActionButton>,
    dismiss_button: ViewHandle<ActionButton>,
    mode: Option<YarpificationMode>,
}

impl YarpifyFooterView {
    pub fn new(terminal_model: Arc<FairMutex<TerminalModel>>, ctx: &mut ViewContext<Self>) -> Self {
        let button_size = ButtonSize::XSmall;

        let yarpify_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Yarpify subshell", AgentFooterButtonTheme::new(None))
                .with_icon(Icon::Yarp)
                .with_size(button_size)
                .with_tooltip("Sign Yarp on for this beat")
                .with_tooltip_alignment(TooltipAlignment::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(YarpifyFooterViewAction::Yarpify);
                })
        });

        let use_agent_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new("Tag in PC", AgentFooterButtonTheme::new(None))
                .with_icon(Icon::Fuzz)
                .with_keybinding(KeystrokeSource::Fixed(USE_AGENT_KEYSTROKE.clone()), ctx)
                .with_size(button_size)
                .with_tooltip("Wave the Yarp PC in for backup")
                .with_tooltip_alignment(TooltipAlignment::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(YarpifyFooterViewAction::UseAgent);
                })
        });

        let dismiss_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Stand down", AgentFooterButtonTheme::new(None))
                .with_size(button_size)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(YarpifyFooterViewAction::Dismiss);
                })
        });

        Self {
            terminal_model,
            yarpify_button,
            use_agent_button,
            dismiss_button,
            mode: None,
        }
    }

    /// Updates the yarpify button label, keybinding, and stores the current yarpification mode.
    pub fn set_mode(&mut self, mode: YarpificationMode, ctx: &mut ViewContext<Self>) {
        let (label, binding_name) = match mode {
            YarpificationMode::Ssh { .. } => {
                ("Yarpify SSH session", "terminal:yarpify_ssh_session")
            }
            YarpificationMode::Subshell { .. } => ("Yarpify subshell", "terminal:yarpify_subshell"),
        };
        self.yarpify_button.update(ctx, |button, ctx| {
            button.set_label(label, ctx);
            button.set_keybinding(Some(KeystrokeSource::Binding(binding_name)), ctx);
        });
        self.mode = Some(mode);
        ctx.notify();
    }

    /// Returns the current yarpification mode, if set.
    pub fn mode(&self) -> Option<&YarpificationMode> {
        self.mode.as_ref()
    }

    /// Clears the yarpification mode.
    pub fn clear_mode(&mut self, ctx: &mut ViewContext<Self>) {
        self.mode = None;
        self.yarpify_button.update(ctx, |button, ctx| {
            button.set_keybinding(None, ctx);
        });
        ctx.notify();
    }
}

#[derive(Debug, Clone)]
pub enum YarpifyFooterViewAction {
    Yarpify,
    UseAgent,
    Dismiss,
}

pub enum YarpifyFooterViewEvent {
    Yarpify { mode: YarpificationMode },
    UseAgent,
    Dismiss,
}

impl Entity for YarpifyFooterView {
    type Event = YarpifyFooterViewEvent;
}

impl View for YarpifyFooterView {
    fn ui_name() -> &'static str {
        "YarpifyFooterView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let terminal_model = self.terminal_model.lock();

        let button_row = Flex::row()
            .with_spacing(4.)
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(ChildView::new(&self.yarpify_button).finish())
            .with_child(ChildView::new(&self.use_agent_button).finish())
            .with_child(Expanded::new(1., Empty::new().finish()).finish())
            .with_child(ChildView::new(&self.dismiss_button).finish());

        let mut container = Container::new(button_row.finish())
            .with_horizontal_padding(*PADDING_LEFT)
            .with_vertical_padding(4.);

        if terminal_model.is_alt_screen_active() {
            if let Some(bg_color) = terminal_model.alt_screen().inferred_bg_color() {
                container = container.with_background(bg_color);
            }
        }

        container.finish()
    }
}

impl TypedActionView for YarpifyFooterView {
    type Action = YarpifyFooterViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            YarpifyFooterViewAction::Yarpify => {
                if let Some(mode) = self.mode.clone() {
                    self.clear_mode(ctx);
                    ctx.emit(YarpifyFooterViewEvent::Yarpify { mode });
                }
            }
            YarpifyFooterViewAction::UseAgent => {
                self.clear_mode(ctx);
                ctx.emit(YarpifyFooterViewEvent::UseAgent);
            }
            YarpifyFooterViewAction::Dismiss => {
                self.clear_mode(ctx);
                ctx.emit(YarpifyFooterViewEvent::Dismiss);
            }
        }
    }
}

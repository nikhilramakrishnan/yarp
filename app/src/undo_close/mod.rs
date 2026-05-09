pub mod settings;
mod stack;

use yarpui::{keymap::EditableBinding, AppContext};

use crate::{util::bindings::CustomAction, workspace::WorkspaceAction};

pub use self::{settings::UndoCloseSettings, stack::UndoCloseStack, stack::UndoCloseStackEvent};

/// Register keybindings for undo close functionality.
pub fn init(ctx: &mut AppContext) {
    ctx.register_editable_bindings([EditableBinding::new(
        "app:reopen_closed_session",
        "Bring the closed shift back on duty",
        // Trigger ReopenClosedSession on the active workspace when
        // the action is taken from the command palette.
        WorkspaceAction::ReopenClosedSession,
    )
    .with_custom_action(CustomAction::ReopenClosedSession)]);
}

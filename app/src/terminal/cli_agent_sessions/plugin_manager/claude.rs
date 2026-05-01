// yarp: the upstream marketplace plugin lived at hotfuzz/claude-code-yarp,
// a GitHub org that doesn't exist. The auto-install/update flow would 404,
// so this stub disables it and falls back to the trait's default behavior:
// `can_auto_install() == false`, no chips/buttons in the footer, and the
// install/update modals are dropped from the UI.

use std::path::PathBuf;
use std::sync::LazyLock;

use super::{CliAgentPluginManager, PluginInstructions};
use crate::terminal::shell::ShellType;

pub(super) struct ClaudeCodePluginManager;

impl ClaudeCodePluginManager {
    pub(super) fn new(
        _shell_path: Option<PathBuf>,
        _shell_type: Option<ShellType>,
        _path_env_var: Option<String>,
    ) -> Self {
        Self
    }
}

impl CliAgentPluginManager for ClaudeCodePluginManager {
    fn minimum_plugin_version(&self) -> &'static str {
        ""
    }

    fn can_auto_install(&self) -> bool {
        false
    }

    fn install_instructions(&self) -> &'static PluginInstructions {
        &EMPTY_INSTRUCTIONS
    }

    fn update_instructions(&self) -> &'static PluginInstructions {
        &EMPTY_INSTRUCTIONS
    }
}

static EMPTY_INSTRUCTIONS: LazyLock<PluginInstructions> = LazyLock::new(|| PluginInstructions {
    title: "",
    subtitle: "",
    steps: &[],
    post_install_notes: &[],
});

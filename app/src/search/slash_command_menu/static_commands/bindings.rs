use yarpui::keymap::{BindingDescription, PerPlatformKeystroke};

use super::StaticCommand;

pub enum DefaultSlashCommandBinding {
    None,
    Single(&'static str),
    PerPlatform(PerPlatformKeystroke),
}

pub fn default_binding_for_command(name: &'static str) -> DefaultSlashCommandBinding {
    match name {
        "/detective" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "cmd-enter",
            linux_and_windows: "ctrl-shift-enter",
        }),
        "/cloud-detective" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "cmd-alt-enter",
            linux_and_windows: "ctrl-alt-enter",
        }),
        "/conversations" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "cmd-y",
            linux_and_windows: "ctrl-shift-Y",
        }),
        "/open-repo" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "alt-cmd-o",
            linux_and_windows: "ctrl-alt-o",
        }),
        _ => DefaultSlashCommandBinding::None,
    }
}

pub fn binding_description(command: &StaticCommand) -> BindingDescription {
    BindingDescription::new_preserve_case(format!("Slash command: {}", command.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detective_commands_own_primary_shortcuts() {
        match default_binding_for_command("/detective") {
            DefaultSlashCommandBinding::PerPlatform(binding) => {
                assert_eq!(binding.mac, "cmd-enter");
                assert_eq!(binding.linux_and_windows, "ctrl-shift-enter");
            }
            _ => panic!("expected /detective to own the primary Taskforce shortcut"),
        }

        match default_binding_for_command("/cloud-detective") {
            DefaultSlashCommandBinding::PerPlatform(binding) => {
                assert_eq!(binding.mac, "cmd-alt-enter");
                assert_eq!(binding.linux_and_windows, "ctrl-alt-enter");
            }
            _ => panic!("expected /cloud-detective to own the cloud Taskforce shortcut"),
        }
    }

    #[test]
    fn legacy_agent_aliases_do_not_claim_primary_shortcuts() {
        assert!(matches!(
            default_binding_for_command("/agent"),
            DefaultSlashCommandBinding::None
        ));
        assert!(matches!(
            default_binding_for_command("/cloud-agent"),
            DefaultSlashCommandBinding::None
        ));
    }
}

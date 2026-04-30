use serde::{Deserialize, Serialize};
use yarp_core::{features::FeatureFlag, settings::Setting};
use yarp_util::path::ShellFamily;

use crate::terminal::yarpify::settings::YarpifySettings;

/// The different possible outcomes of detecting an interactive SSH session.
/// Also the payload for the [`crate::server::telemetry::TelemetryEvent::SshInteractiveSessionDetected`] event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SshInteractiveSessionDetected {
    #[serde(rename = "feature_disabled")]
    FeatureDisabled,
    #[serde(rename = "host_denylisted")]
    HostDenylisted,
    #[serde(rename = "yarpify_prompt")]
    ShouldPromptYarpification {
        #[serde(skip)]
        command: String,
        #[serde(skip)]
        host: Option<String>,
    },
}

/// Determines whether a host could be yarpified.
pub fn evaluate_yarpify_ssh_host(
    command: &str,
    ssh_host: Option<&str>,
    shell_family: ShellFamily,
    yarpify_settings: &YarpifySettings,
) -> SshInteractiveSessionDetected {
    let should_prompt_ssh_tmux_wrapper = *yarpify_settings.enable_ssh_yarpification.value()
        && *yarpify_settings.use_ssh_tmux_wrapper.value();
    let matches_subshell = yarpify_settings.is_denylisted_subshell_command(command)
        || yarpify_settings.is_compatible_subshell_command(command, shell_family);
    if !should_prompt_ssh_tmux_wrapper
        || matches_subshell
        || !FeatureFlag::SSHTmuxWrapper.is_enabled()
    {
        return SshInteractiveSessionDetected::FeatureDisabled;
    }

    if let Some(ssh_host) = ssh_host {
        if yarpify_settings.is_ssh_host_denylisted(ssh_host) {
            return SshInteractiveSessionDetected::HostDenylisted;
        }
    }

    SshInteractiveSessionDetected::ShouldPromptYarpification {
        host: ssh_host.map(|host| host.to_owned()),
        command: command.to_string(),
    }
}

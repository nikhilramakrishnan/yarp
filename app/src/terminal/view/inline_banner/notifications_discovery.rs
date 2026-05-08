use serde::Serialize;
use yarpui::{elements::MouseStateHandle, notification::RequestPermissionsOutcome, Element};

use crate::{
    appearance::Appearance,
    terminal::{
        session_settings::NotificationsMode,
        view::{InlineBannerId, NotificationsTrigger, TerminalAction},
    },
};

use super::{
    render_inline_block_list_banner, InlineBannerButtonState, InlineBannerCloseButton,
    InlineBannerContent, InlineBannerStyle, InlineBannerTextButton, InlineBannerTextButtonVariant,
};

#[derive(Clone, Copy, Debug, Serialize)]
pub enum NotificationsDiscoveryBannerAction {
    LearnMore,
    Troubleshoot,
    TurnOn(NotificationsTrigger),
    Configure,
    Close,
}

#[derive(Default)]
pub struct NotificationsDiscoveryBannerMouseStates {
    pub learn_more: MouseStateHandle,
    pub troubleshoot: MouseStateHandle,
    pub turn_on: MouseStateHandle,
    pub configure: MouseStateHandle,
    pub close: MouseStateHandle,
}

/// State necessary to render the (singleton) notifications discovery banner.
pub struct NotificationsDiscoveryBannerState {
    pub banner_id: InlineBannerId,
    pub mouse_states: NotificationsDiscoveryBannerMouseStates,
}

pub fn render_inline_notifications_discovery_banner(
    trigger: NotificationsTrigger,
    request_outcome: Option<RequestPermissionsOutcome>,
    state: &NotificationsDiscoveryBannerState,
    notifications_mode: NotificationsMode,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let active_ui_text_color = appearance.theme().active_ui_text_color().into_solid();

    let learn_more_button = InlineBannerTextButton {
        text: "Pull the dossier".to_string(),
        text_color: active_ui_text_color,
        button_state: InlineBannerButtonState {
            on_click_event: TerminalAction::NotificationsDiscoveryBanner(
                NotificationsDiscoveryBannerAction::LearnMore,
            ),
            mouse_state_handle: state.mouse_states.learn_more.clone(),
        },
        font: Default::default(),
        position_id: None,
        variant: InlineBannerTextButtonVariant::Secondary,
    };
    let troubleshoot_button = InlineBannerTextButton {
        text: "Troubleshoot".to_string(),
        text_color: active_ui_text_color,
        button_state: InlineBannerButtonState {
            on_click_event: TerminalAction::NotificationsDiscoveryBanner(
                NotificationsDiscoveryBannerAction::Troubleshoot,
            ),
            mouse_state_handle: state.mouse_states.troubleshoot.clone(),
        },
        font: Default::default(),
        position_id: None,
        variant: InlineBannerTextButtonVariant::Secondary,
    };

    let (title, buttons) = match notifications_mode {
        NotificationsMode::Dismissed => (
            "We won't flag this again — Standing Orders has the radio chatter switch if you want it on.",
            vec![],
        ),
        NotificationsMode::Disabled => (
            "Radio chatter's off — Standing Orders has the switch if you want it back on.",
            vec![],
        ),
        NotificationsMode::Unset => (
            trigger.discovery_banner_copy(),
            vec![
                learn_more_button,
                InlineBannerTextButton {
                    text: "Enable".to_string(),
                    text_color: active_ui_text_color,
                    button_state: InlineBannerButtonState {
                        on_click_event: TerminalAction::NotificationsDiscoveryBanner(
                            NotificationsDiscoveryBannerAction::TurnOn(trigger),
                        ),
                        mouse_state_handle: state.mouse_states.turn_on.clone(),
                    },
                    font: Default::default(),
                    position_id: None,
                    variant: InlineBannerTextButtonVariant::Primary,
                },
            ],
        ),
        NotificationsMode::Enabled => {
            // Determine the messaging based on what the user's response was to the
            // permissions request (if any)
            let (title, docs_button) = match request_outcome {
                Some(request_outcome) => match request_outcome {
                    RequestPermissionsOutcome::Accepted => (
                        "Right then — radio chatter's wired in. You'll catch the dispatches now.",
                        learn_more_button,
                    ),
                    RequestPermissionsOutcome::PermissionsDenied => (
                        "Yarp's been waved off the radio — no clearance to chime in.",
                        troubleshoot_button,
                    ),
                    RequestPermissionsOutcome::OtherError { .. } => (
                        "Something's gone sideways while waving for radio permissions.",
                        troubleshoot_button,
                    ),
                },
                None => (
                    "Wave 'Allow' through on the request to wire up the radio chatter.",
                    learn_more_button,
                ),
            };

            (
                title,
                vec![
                    docs_button,
                    InlineBannerTextButton {
                        text: "Configure notifications".to_string(),
                        text_color: active_ui_text_color,
                        button_state: InlineBannerButtonState {
                            on_click_event: TerminalAction::NotificationsDiscoveryBanner(
                                NotificationsDiscoveryBannerAction::Configure,
                            ),
                            mouse_state_handle: state.mouse_states.configure.clone(),
                        },
                        font: Default::default(),
                        position_id: None,
                        variant: InlineBannerTextButtonVariant::Secondary,
                    },
                ],
            )
        }
    };

    let close_button = InlineBannerCloseButton(InlineBannerButtonState {
        on_click_event: TerminalAction::NotificationsDiscoveryBanner(
            NotificationsDiscoveryBannerAction::Close,
        ),
        mouse_state_handle: state.mouse_states.close.clone(),
    });

    render_inline_block_list_banner(
        InlineBannerStyle::CallToAction,
        appearance,
        InlineBannerContent {
            title: title.to_owned(),
            buttons,
            close_button: Some(close_button),
            ..Default::default()
        },
    )
}

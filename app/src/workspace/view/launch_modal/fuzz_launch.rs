use super::{CTAButton, CheckboxConfig, LaunchModalEvent, Slide};
use crate::ai::ambient_agents::telemetry::{CloudAgentTelemetryEvent, CloudModeEntryPoint};
use crate::terminal::view::OnboardingIntention;
use crate::ui_components::icons::Icon;
use crate::workspace::action::WorkspaceAction;
use crate::workspace::view::OnboardingTutorial;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::{AdminEnablementSetting, UgcCollectionEnablementSetting};
use asset_macro::bundled_or_fetched_asset;
use markdown_parser::{FormattedTextFragment, FormattedTextLine};
use yarp_core::send_telemetry_from_ctx;
use yarpui::assets::asset_cache::AssetSource;
use yarpui::{AppContext, SingletonEntity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OzLaunchSlide {
    CloudAgents,
    AgentAutomations,
    AgentManagement,
    LaunchCredits,
}

impl Slide for OzLaunchSlide {
    fn modal_title(&self) -> String {
        "Introducing Fuzz".to_string()
    }

    fn modal_subtext_paragraphs(&self) -> Vec<FormattedTextLine> {
        vec![FormattedTextLine::Line(vec![
            FormattedTextFragment::plain_text(
                "Infinitely scalable PC — dispatch from your desk or call up an ambient officer over the radio.",
            ),
        ])]
    }

    fn first() -> Self {
        OzLaunchSlide::CloudAgents
    }

    fn next(&self) -> Option<Self> {
        match self {
            OzLaunchSlide::CloudAgents => Some(OzLaunchSlide::AgentAutomations),
            OzLaunchSlide::AgentAutomations => Some(OzLaunchSlide::AgentManagement),
            OzLaunchSlide::AgentManagement => Some(OzLaunchSlide::LaunchCredits),
            OzLaunchSlide::LaunchCredits => None,
        }
    }

    fn prev(&self) -> Option<Self> {
        match self {
            OzLaunchSlide::CloudAgents => None,
            OzLaunchSlide::AgentAutomations => Some(OzLaunchSlide::CloudAgents),
            OzLaunchSlide::AgentManagement => Some(OzLaunchSlide::AgentAutomations),
            OzLaunchSlide::LaunchCredits => Some(OzLaunchSlide::AgentManagement),
        }
    }

    fn display_text(&self) -> Option<&'static str> {
        Some(match self {
            OzLaunchSlide::CloudAgents => "Ambient officers",
            OzLaunchSlide::AgentAutomations => "PC automations",
            OzLaunchSlide::AgentManagement => "Duty roster",
            OzLaunchSlide::LaunchCredits => "A little gift",
        })
    }

    fn short_label(&self) -> &'static str {
        match self {
            OzLaunchSlide::CloudAgents => "Ambient officers",
            OzLaunchSlide::AgentAutomations => "PC automations",
            OzLaunchSlide::AgentManagement => "Duty roster",
            OzLaunchSlide::LaunchCredits => "Launch rations",
        }
    }

    fn title(&self) -> &'static str {
        match self {
            OzLaunchSlide::CloudAgents => "Step away from the desk — ambient officers cover the patrol",
            OzLaunchSlide::AgentAutomations => {
                "Run the briefing room — turn Skills into standing orders"
            }
            OzLaunchSlide::AgentManagement => "Track every PC on the duty roster, desk and ambient",
            OzLaunchSlide::LaunchCredits => {
                "1,000 free ambient officer rations when you top up the kit to Yarp Build"
            }
        }
    }

    fn title_icon(&self) -> Option<Icon> {
        None
    }

    fn content(&self) -> &'static str {
        match self {
            OzLaunchSlide::CloudAgents => {
                "Dispatch ambient officers to work calls in parallel, keep them on patrol after you've checked off duty, or call them up over the radio. You can listen in on the comms over the web too."
            }
            OzLaunchSlide::AgentAutomations => {
                "Fuzz PCs are written up using the standard Skills format. The duty scheduler can put PCs on patrol at set intervals, or you can radio them in via the Fuzz SDK or API to start and manage Fuzz PCs from anywhere."
            }
            OzLaunchSlide::AgentManagement => {
                "Read the whole roster — desk PCs and ambient officers — from inside Yarp. Listen in on a live shift, take the call back to your desk, or steer a PC with one click."
            }
            OzLaunchSlide::LaunchCredits => {
                "Top up the kit to Build this month and you'll get 1,000 extra rations to put Fuzz on patrol. Rations only count toward Fuzz shifts in Yarp-hosted environments."
            }
        }
    }

    fn image(&self) -> AssetSource {
        // TODO: Replace with new images once provided.
        match self {
            OzLaunchSlide::CloudAgents => {
                bundled_or_fetched_asset!("png/fuzz_cloud_agents.png")
            }
            OzLaunchSlide::AgentAutomations => {
                bundled_or_fetched_asset!("png/fuzz_agent_automations.png")
            }
            OzLaunchSlide::AgentManagement => {
                bundled_or_fetched_asset!("png/fuzz_agent_management.png")
            }
            OzLaunchSlide::LaunchCredits => {
                bundled_or_fetched_asset!("png/fuzz_launch_credits.png")
            }
        }
    }

    fn all() -> Vec<Self> {
        vec![
            OzLaunchSlide::CloudAgents,
            OzLaunchSlide::AgentAutomations,
            OzLaunchSlide::AgentManagement,
            OzLaunchSlide::LaunchCredits,
        ]
    }

    fn cta_button(&self) -> CTAButton<Self> {
        match self {
            OzLaunchSlide::CloudAgents
            | OzLaunchSlide::AgentAutomations
            | OzLaunchSlide::AgentManagement => {
                let next = self.next().expect("Non-final slides should have a next");
                CTAButton::next_slide(next, format!("Next: {}", next.short_label()))
            }
            OzLaunchSlide::LaunchCredits => CTAButton::custom("Roll out", |ctx| {
                send_telemetry_from_ctx!(
                    CloudAgentTelemetryEvent::EnteredCloudMode {
                        entry_point: CloudModeEntryPoint::OzLaunchModal,
                    },
                    ctx
                );
                ctx.emit(LaunchModalEvent::Close);
                ctx.dispatch_typed_action(&WorkspaceAction::StartAgentOnboardingTutorial(
                    OnboardingTutorial::NoProject {
                        intention: OnboardingIntention::AgentDrivenDevelopment,
                    },
                ));
                ctx.dispatch_typed_action(&WorkspaceAction::AddAmbientAgentTab);
            }),
        }
    }

    fn secondary_cta_button(&self) -> Option<CTAButton<Self>> {
        match self {
            OzLaunchSlide::LaunchCredits => Some(CTAButton::close("Skip for now")),
            OzLaunchSlide::CloudAgents
            | OzLaunchSlide::AgentAutomations
            | OzLaunchSlide::AgentManagement => None,
        }
    }

    fn checkbox_config(&self) -> Option<CheckboxConfig> {
        Some(CheckboxConfig {
            label: "File case files in central records",
            description: "Case files filed in central records can be passed to anyone with one click, and let you pick the case back up from another desk or after checking off duty.",
        })
    }

    fn should_show_checkbox(&self, app: &AppContext) -> bool {
        let cloud_storage_setting =
            UserWorkspaces::as_ref(app).get_cloud_conversation_storage_enablement_setting();
        let ugc_setting = UserWorkspaces::as_ref(app).get_ugc_collection_enablement_setting();

        // Show checkbox only when user has control over cloud storage AND UGC is not force-enabled.
        matches!(
            cloud_storage_setting,
            AdminEnablementSetting::RespectUserSetting
        ) && !matches!(ugc_setting, UgcCollectionEnablementSetting::Enable)
    }

    fn on_close(&self, ctx: &mut yarpui::ViewContext<super::LaunchModal<Self>>) {
        ctx.dispatch_typed_action(&WorkspaceAction::StartAgentOnboardingTutorial(
            OnboardingTutorial::NoProject {
                intention: OnboardingIntention::AgentDrivenDevelopment,
            },
        ));
    }
}

pub fn init(app: &mut yarpui::AppContext) {
    super::init::<OzLaunchSlide>(app);
}

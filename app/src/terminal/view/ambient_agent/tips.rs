//! Tips for cloud mode loading screen.

use crate::ai::agent_tips::AITip;
use yarpui::keymap::Keystroke;
use yarpui::AppContext;

/// A cloud mode tip with text and optional link.
#[derive(Clone, Debug)]
pub struct CloudModeTip {
    text: String,
    link: Option<String>,
}

impl CloudModeTip {
    pub fn new(text: impl Into<String>, link: Option<&'static str>) -> Self {
        Self {
            text: text.into(),
            link: link.map(str::to_string),
        }
    }
}

impl AITip for CloudModeTip {
    fn keystroke(&self, _app: &AppContext) -> Option<Keystroke> {
        None
    }

    fn link(&self) -> Option<String> {
        self.link.clone()
    }

    fn description(&self) -> &str {
        &self.text
    }

    // Uses the default implementation which adds "Tip: " prefix and parses backticks as inline code
}

/// Returns a collection of tips for the cloud mode loading screen.
pub fn get_cloud_mode_tips() -> Vec<CloudModeTip> {
    vec![
        CloudModeTip::new(
            "Install the Taskforce Slack integration to dispatch ambient officers from any channel or DM.",
            None,
        ),
        CloudModeTip::new(
            "Build programmatic PCs using Taskforce's TypeScript and Python SDKs.",
            None,
        ),
        CloudModeTip::new(
            "Set team or personal secrets for ambient officers using the `fuzz secret` command.",
            None,
        ),
        CloudModeTip::new(
            "View all your PC patrols and their status in the Taskforce web app.",
            None,
        ),
        CloudModeTip::new(
            "Tune into any Taskforce officer's beat live by opening a radio channel.",
            None,
        ),
        CloudModeTip::new(
            "Set up recurring patrols on cron schedules so a PC walks the beat without you radioing in.",
            None,
        ),
        CloudModeTip::new(
            "Dispatch PCs that automatically work the case when issues are filed in Linear.",
            None,
        ),
        CloudModeTip::new(
            "Stand up PCs that respond to CI failures and have a go at fixing them.",
            None,
        ),
        CloudModeTip::new(
            "Send PCs out from GitHub Actions using the `fuzz-agent-action`.",
            Some("https://github.com/hotfuzz/fuzz-agent-action"),
        ),
        CloudModeTip::new(
            "Call the Taskforce REST API to dispatch PCs from any backend service or internal tool.",
            None,
        ),
        CloudModeTip::new(
            "Kit out reusable beats with Docker images so every PC walks into the same scene.",
            None,
        ),
        CloudModeTip::new(
            "Share PC session links with your team for collaborative case work.",
            None,
        ),
        CloudModeTip::new(
            "Use the `--share` flag with the Taskforce CLI to open the radio channel from anywhere.",
            None,
        ),
        CloudModeTip::new(
            "Fork a completed Taskforce ambient officer's case file into Yarp to keep working it from your desk.",
            None,
        ),
        CloudModeTip::new(
            "Stand up internal tools that send PCs to answer questions from your databases.",
            None,
        ),
        CloudModeTip::new(
            "Put a scheduled PC on the rota to bin stale feature flags every week.",
            None,
        ),
        CloudModeTip::new(
            "Tag @Taskforce in Linear issues to dispatch a PC to investigate and propose a fix.",
            None,
        ),
        CloudModeTip::new(
            "Send PCs out on remote dev boxes or CI runners using the Taskforce CLI.",
            None,
        ),
        CloudModeTip::new(
            "Wire up MCP servers so Taskforce ambient officers can radio GitHub, Linear, and Sentry.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz agent run` to dispatch a PC without ever opening the Yarp terminal.",
            None,
        ),
        CloudModeTip::new(
            "Tune in to your colleagues' PC patrols in the Taskforce web app for shared visibility.",
            None,
        ),
        CloudModeTip::new(
            "Stand up PCs that automatically triage and label incoming GitHub issues.",
            None,
        ),
        CloudModeTip::new(
            "Put a PC on the rota to file a daily report on newly opened issues.",
            None,
        ),
        CloudModeTip::new(
            "Dispatch a PC to review PRs on the spot and radio in suggested improvements.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz environment create` to lay out reproducible beats for your PCs.",
            None,
        ),
        CloudModeTip::new(
            "Trigger PCs from webhooks to respond to production incidents.",
            None,
        ),
        CloudModeTip::new(
            "Stand up a PC that restarts services or scales deployments the moment alerts fire.",
            None,
        ),
        CloudModeTip::new(
            "Use personal secrets for credentials that should only ride along with your own PCs.",
            None,
        ),
        CloudModeTip::new(
            "Use team secrets for shared infrastructure credentials across the whole duty roster.",
            None,
        ),
        CloudModeTip::new(
            "Put a PC on the night shift to check for dependency updates.",
            None,
        ),
        CloudModeTip::new(
            "Stand up a PC that formats and lints code on a fixed rota.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz schedule create` to put PCs on a cron-triggered rota.",
            None,
        ),
        CloudModeTip::new(
            "Stand a scheduled PC down and back up without binning them using `fuzz schedule pause`.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz mcp list` to see which MCP servers your PCs can radio.",
            None,
        ),
        CloudModeTip::new(
            "Stand up an internal Slack bot that hands off coding work to Taskforce PCs.",
            None,
        ),
        CloudModeTip::new(
            "Dispatch a PC that responds to @mentions in Slack threads with the full case file.",
            None,
        ),
        CloudModeTip::new(
            "Use the Taskforce TypeScript SDK to build custom dispatch pipelines.",
            None,
        ),
        CloudModeTip::new(
            "Use the Taskforce Python SDK to put PCs in your data pipelines.",
            None,
        ),
        CloudModeTip::new(
            "Watch PC success rates and patrol times using the Taskforce API.",
            None,
        ),
        CloudModeTip::new(
            "Stand up a dashboard that watches every PC on the duty roster across your team.",
            None,
        ),
    ]
}

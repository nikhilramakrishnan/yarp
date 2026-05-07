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
            "Install the Fuzz Slack integration to trigger agents from any channel or DM.",
            None,
        ),
        CloudModeTip::new(
            "Build programmatic agents using Fuzz's TypeScript and Python SDKs.",
            None,
        ),
        CloudModeTip::new(
            "Set team or personal secrets for agents using the `fuzz secret` command.",
            None,
        ),
        CloudModeTip::new(
            "View all your agent runs and their status in the Fuzz web app.",
            None,
        ),
        CloudModeTip::new(
            "Join any Fuzz cloud agent run in real-time using Agent Session Sharing.",
            None,
        ),
        CloudModeTip::new(
            "Set up recurring agents that run on cron schedules for automated maintenance.",
            None,
        ),
        CloudModeTip::new(
            "Create agents that automatically fix bugs when issues are filed in Linear.",
            None,
        ),
        CloudModeTip::new(
            "Build agents that respond to CI failures and attempt automatic fixes.",
            None,
        ),
        CloudModeTip::new(
            "Run agents from GitHub Actions using the `fuzz-agent-action`.",
            Some("https://github.com/hotfuzz/fuzz-agent-action"),
        ),
        CloudModeTip::new(
            "Call the Fuzz REST API to trigger agents from any backend service or internal tool.",
            None,
        ),
        CloudModeTip::new(
            "Create reusable environments with Docker images for consistent agent execution.",
            None,
        ),
        CloudModeTip::new(
            "Share agent session links with your team for collaborative debugging.",
            None,
        ),
        CloudModeTip::new(
            "Use the `--share` flag with the Fuzz CLI to enable session sharing from anywhere.",
            None,
        ),
        CloudModeTip::new(
            "Fork a completed Fuzz cloud agent session into Yarp to continue the work locally.",
            None,
        ),
        CloudModeTip::new(
            "Build internal tools that use agents to answer questions from your databases.",
            None,
        ),
        CloudModeTip::new(
            "Create a scheduled agent to clean up stale feature flags every week.",
            None,
        ),
        CloudModeTip::new(
            "Tag @Fuzz in Linear issues to automatically investigate and propose fixes.",
            None,
        ),
        CloudModeTip::new(
            "Run agents on remote dev boxes or CI runners using the Fuzz CLI.",
            None,
        ),
        CloudModeTip::new(
            "Wire up MCP servers so Fuzz ambient officers can radio GitHub, Linear, and Sentry.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz agent run` to kick off tasks without opening the Yarp terminal.",
            None,
        ),
        CloudModeTip::new(
            "View your teammates' agent runs in the Fuzz web app for shared visibility.",
            None,
        ),
        CloudModeTip::new(
            "Build agents that automatically triage and label incoming GitHub issues.",
            None,
        ),
        CloudModeTip::new(
            "Set up an agent to generate daily summaries of newly opened issues.",
            None,
        ),
        CloudModeTip::new(
            "Create an agent that automatically reviews PRs and suggests improvements.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz environment create` to define reproducible execution contexts.",
            None,
        ),
        CloudModeTip::new(
            "Trigger agents from webhooks to respond to production incidents.",
            None,
        ),
        CloudModeTip::new(
            "Build an agent that restarts services or scales deployments when alerts fire.",
            None,
        ),
        CloudModeTip::new(
            "Use personal secrets for credentials that should only be used by your agents.",
            None,
        ),
        CloudModeTip::new(
            "Use team secrets for shared infrastructure credentials across all agents.",
            None,
        ),
        CloudModeTip::new(
            "Create an agent that runs nightly to check for dependency updates.",
            None,
        ),
        CloudModeTip::new(
            "Build an agent that automatically formats and lints code on a schedule.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz schedule create` to set up cron-triggered agents.",
            None,
        ),
        CloudModeTip::new(
            "Pause and resume scheduled agents without deleting them using `fuzz schedule pause`.",
            None,
        ),
        CloudModeTip::new(
            "Use `fuzz mcp list` to see which MCP servers are available to your agents.",
            None,
        ),
        CloudModeTip::new(
            "Build an internal Slack bot that delegates coding tasks to Fuzz agents.",
            None,
        ),
        CloudModeTip::new(
            "Create an agent that responds to @mentions in Slack threads with full context.",
            None,
        ),
        CloudModeTip::new(
            "Use the Fuzz TypeScript SDK to build custom automation pipelines.",
            None,
        ),
        CloudModeTip::new(
            "Use the Fuzz Python SDK to integrate agents into your data pipelines.",
            None,
        ),
        CloudModeTip::new(
            "Monitor agent success rates and runtimes using the Fuzz API.",
            None,
        ),
        CloudModeTip::new(
            "Build a dashboard that tracks all agent activity across your team.",
            None,
        ),
    ]
}

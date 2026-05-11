use std::{collections::HashMap, sync::LazyLock};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use yarp_core::features::FeatureFlag;

use crate::search::slash_command_menu::{static_commands::Argument, StaticCommand};

use super::Availability;

pub static AGENT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/agent",
    description: "Brief a detective (legacy alias)",
    icon_path: "bundled/svg/fuzz.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub static DETECTIVE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/detective",
    description: "Brief a detective",
    icon_path: "bundled/svg/fuzz.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub static CLOUD_AGENT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/cloud-agent",
    description: "Brief a cloud detective (legacy alias)",
    icon_path: "bundled/svg/fuzz-cloud.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub static CLOUD_DETECTIVE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/cloud-detective",
    description: "Brief a detective in the cloud",
    icon_path: "bundled/svg/fuzz-cloud.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub const ADD_MCP: StaticCommand = StaticCommand {
    name: "/add-mcp",
    description: "Recruit a new MCP informant",
    icon_path: "bundled/svg/dataflow.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const PR_COMMENTS: StaticCommand = StaticCommand {
    name: "/pr-comments",
    description: "Review the case file comments",
    icon_path: "bundled/svg/github.svg",
    availability: Availability::REPOSITORY.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
};

pub static CREATE_ENVIRONMENT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/create-environment",
    description: "Set up a precinct (Docker image + repos) via guided briefing",
    icon_path: "bundled/svg/dataflow.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(
        Argument::optional()
            .with_hint_text("<optional repo paths or GitHub URLs>")
            .with_execute_on_selection(),
    ),
});

pub const CREATE_DOCKER_SANDBOX: StaticCommand = StaticCommand {
    name: "/docker-sandbox",
    description: "Open a sandbox precinct",
    icon_path: "bundled/svg/docker.svg",
    availability: Availability::LOCAL.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
};

pub static CREATE_NEW_PROJECT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/create-new-project",
    description: "Open a fresh investigation",
    icon_path: "bundled/svg/plus.svg",
    availability: Availability::LOCAL | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::required().with_hint_text("<describe what you want to build>")),
});

pub static EDIT_SKILL: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-skill",
    description: "Pull a technique file from the casebook",
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static INVOKE_SKILL: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/skills",
    description: "Deploy a technique",
    icon_path: "bundled/svg/stars-01.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub static ADD_PROMPT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/add-prompt",
    description: "File a new briefing",
    icon_path: if FeatureFlag::AgentView.is_enabled() {
        "bundled/svg/prompt.svg"
    } else {
        "bundled/svg/agentmode.svg"
    },
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub const ADD_RULE: StaticCommand = StaticCommand {
    name: "/add-rule",
    description: "Add a new procedure to the standing orders",
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub static EDIT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/open-file",
    description: "Pull evidence into the editor",
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: Some(
        Argument::optional().with_hint_text("<path/to/file[:line[:col]]> or \"@\" to search"),
    ),
});

pub static RENAME_TAB: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/rename-tab",
    description: "Re-label this desk",
    icon_path: "bundled/svg/pencil-line.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: Some(Argument::required().with_hint_text("<tab name>")),
});

pub static FORK: LazyLock<StaticCommand> = LazyLock::new(|| {
    let hint_text = "<optional prompt to send in forked conversation>";
    StaticCommand {
        name: "/fork",
        description: "Branch the investigation in a new pane or tab",
        icon_path: "bundled/svg/arrow-split.svg",
        availability: Availability::AGENT_VIEW
            | Availability::ACTIVE_CONVERSATION
            | Availability::NO_LRC_CONTROL
            | Availability::AI_ENABLED,
        auto_enter_ai_mode: true,
        argument: Some(Argument::optional().with_hint_text(hint_text)),
    }
});

pub const OPEN_CODE_REVIEW: StaticCommand = StaticCommand {
    name: "/open-code-review",
    description: "Review the evidence",
    icon_path: "bundled/svg/diff.svg",
    availability: Availability::REPOSITORY,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const INDEX: StaticCommand = StaticCommand {
    name: "/index",
    description: "Catalogue the codebase",
    icon_path: "bundled/svg/find-all.svg",
    availability: Availability::REPOSITORY
        .union(Availability::CODEBASE_CONTEXT)
        .union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
};

pub const INIT: StaticCommand = StaticCommand {
    name: "/init",
    description: "Catalogue the codebase and file an AGENTS.md briefing",
    icon_path: "bundled/svg/yarp-2.svg",
    availability: Availability::REPOSITORY
        .union(Availability::AGENT_VIEW)
        .union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
};

pub const OPEN_PROJECT_RULES: StaticCommand = StaticCommand {
    name: "/open-project-rules",
    description: "Open the project standing orders (AGENTS.md)",
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::REPOSITORY.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
};

pub const OPEN_MCP_SERVERS: StaticCommand = StaticCommand {
    name: "/open-mcp-servers",
    description: "Open the informant registry",
    icon_path: "bundled/svg/dataflow.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const OPEN_SETTINGS_FILE: StaticCommand = StaticCommand {
    name: "/open-settings-file",
    description: "Open the precinct config (TOML)",
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const CHANGELOG: StaticCommand = StaticCommand {
    name: "/changelog",
    description: "Read the latest dispatch",
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: None,
};

// Accepts an optional argument so that buffers like `/feedback some text` still parse to
// this command (the trailing text is ignored on execution). Without this, typing any
// argument after `/feedback` would fall through and be treated as plain input.
pub static FEEDBACK: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/feedback",
    description: "File a complaint",
    icon_path: "bundled/svg/feedback.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub const OPEN_REPO: StaticCommand = StaticCommand {
    name: "/open-repo",
    description: "Switch to another case file",
    icon_path: "bundled/svg/folder.svg",
    availability: Availability::LOCAL.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
};

pub const OPEN_RULES: StaticCommand = StaticCommand {
    name: "/open-rules",
    description: "Review standing orders",
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub static NEW: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/new",
    description: "Brief a detective (alias for /detective)",
    icon_path: "bundled/svg/new-conversation.svg",
    availability: Availability::NO_LRC_CONTROL | Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: Some(Argument::optional().with_execute_on_selection()),
});

pub static MODEL: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/model",
    description: "Reassign the detective",
    icon_path: "bundled/svg/fuzz.svg",
    availability: Availability::AGENT_VIEW | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: None,
});

pub static PROFILE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/profile",
    description: "Switch the active duty profile",
    icon_path: "bundled/svg/psychology.svg",
    availability: Availability::AGENT_VIEW | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: None,
});

pub const PLAN_NAME: &str = "/plan";

pub static PLAN: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: PLAN_NAME,
    description: "Brief the detective to investigate and sketch a plan",
    icon_path: "bundled/svg/file-06.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text("<describe your task>")),
});

pub const ORCHESTRATE_NAME: &str = "/orchestrate";

pub static ORCHESTRATE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: ORCHESTRATE_NAME,
    description: "Coordinate the unit on a multi-pronged operation",
    icon_path: "bundled/svg/fuzz.svg",
    availability: Availability::LOCAL | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text("<describe your task>")),
});

/// If `query` starts with the given command `name` followed by a space,
/// returns the remainder of the query. Otherwise returns `None`.
pub fn strip_command_prefix(query: &str, name: &str) -> Option<String> {
    query
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix(' '))
        .map(|rest| rest.to_string())
}

pub static COMPACT: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/compact",
    description: "Tidy the briefing — summarise prior conversation",
    icon_path: "bundled/svg/collapse_content.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(
        Argument::optional().with_hint_text("<optional custom summarization instructions>"),
    ),
});

pub static COMPACT_AND: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/compact-and",
    description: "Tidy the briefing and send a follow-up",
    icon_path: "bundled/svg/collapse_content.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text("<prompt to send after compaction>")),
});

pub static QUEUE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/queue",
    description: "Stack a follow-up briefing in the queue",
    icon_path: "bundled/svg/clock-plus.svg",
    availability: Availability::AGENT_VIEW
        | Availability::ACTIVE_CONVERSATION
        | Availability::NO_LRC_CONTROL
        | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(
        Argument::required().with_hint_text("<prompt to send when the officer is done>"),
    ),
});

pub static FORK_AND_COMPACT: LazyLock<StaticCommand> = LazyLock::new(|| {
    let hint_text = "<optional prompt to send after compaction>";
    StaticCommand {
        name: "/fork-and-compact",
        description: "Branch the investigation and tidy the new copy",
        icon_path: "bundled/svg/fork_and_compact.svg",
        availability: Availability::AGENT_VIEW
            | Availability::ACTIVE_CONVERSATION
            | Availability::NO_LRC_CONTROL
            | Availability::AI_ENABLED,
        auto_enter_ai_mode: true,
        argument: Some(Argument::optional().with_hint_text(hint_text)),
    }
});

pub const FORK_FROM: StaticCommand = StaticCommand {
    name: "/fork-from",
    description: "Branch the investigation from a specific point",
    icon_path: "bundled/svg/arrow-split.svg",
    availability: Availability::AGENT_VIEW
        .union(Availability::NO_LRC_CONTROL)
        .union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
};

pub const USAGE: StaticCommand = StaticCommand {
    name: "/usage",
    description: "Open the cost ledger",
    icon_path: "bundled/svg/bar-chart-04.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const REMOTE_CONTROL: StaticCommand = StaticCommand {
    name: "/remote-control",
    description: "Take remote command of this shift",
    icon_path: "bundled/svg/phone-01.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const COST: StaticCommand = StaticCommand {
    name: "/cost",
    description: "Toggle the cost ledger",
    icon_path: "bundled/svg/bar-chart-04.svg",
    availability: Availability::AGENT_VIEW.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: false,
    argument: None,
};

pub const CONVERSATIONS: StaticCommand = StaticCommand {
    name: "/conversations",
    description: "Open the case history",
    icon_path: "bundled/svg/conversation.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
};

pub static PROMPTS: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/prompts",
    description: "Sweep filed briefings",
    icon_path: "bundled/svg/prompt.svg",
    availability: Availability::AI_ENABLED,
    auto_enter_ai_mode: false,
    argument: None,
});

pub const REWIND: StaticCommand = StaticCommand {
    name: "/rewind",
    description: "Roll back the tape",
    icon_path: "bundled/svg/clock-rewind.svg",
    availability: Availability::AGENT_VIEW.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
};

pub const EXPORT_TO_CLIPBOARD: StaticCommand = StaticCommand {
    name: "/export-to-clipboard",
    description: "Bag the briefing as markdown",
    icon_path: "bundled/svg/copy.svg",
    availability: Availability::AGENT_VIEW.union(Availability::AI_ENABLED),
    auto_enter_ai_mode: true,
    argument: None,
};

pub static EXPORT_TO_FILE: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/export-to-file",
    description: "File the briefing as a markdown report",
    icon_path: "bundled/svg/download-01.svg",
    availability: Availability::AGENT_VIEW | Availability::AI_ENABLED,
    auto_enter_ai_mode: true,
    argument: Some(Argument::optional().with_hint_text("<optional filename>")),
});

pub const RADIO: StaticCommand = StaticCommand {
    name: "/radio",
    description: "Dispatch a transmission to other units",
    icon_path: "bundled/svg/message-chat-square.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: Some(Argument {
        hint_text: Some("<message>"),
        is_optional: false,
        should_execute_on_selection: false,
    }),
};

pub const INBOX: StaticCommand = StaticCommand {
    name: "/inbox",
    description: "Receive transmissions from other units",
    icon_path: "bundled/svg/inbox-01.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const RESPOND: StaticCommand = StaticCommand {
    name: "/respond",
    description: "Reply to the most recent inbound transmission",
    icon_path: "bundled/svg/corner-down-left.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: Some(Argument {
        hint_text: Some("<message>"),
        is_optional: false,
        should_execute_on_selection: false,
    }),
};

pub const ROSTER: StaticCommand = StaticCommand {
    name: "/roster",
    description: "Show the precinct roster",
    icon_path: "bundled/svg/users-02.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const SITREP: StaticCommand = StaticCommand {
    name: "/sitrep",
    description: "Situational report from the station",
    icon_path: "bundled/svg/info.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const CASE: StaticCommand = StaticCommand {
    name: "/case",
    description: "Open a case — rename tab and notify other units",
    icon_path: "bundled/svg/folder.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: Some(Argument {
        hint_text: Some("<case name>"),
        is_optional: false,
        should_execute_on_selection: false,
    }),
};

pub const CLOCK_OUT: StaticCommand = StaticCommand {
    name: "/clock-out",
    description: "End of shift — clear pending radio traffic",
    icon_path: "bundled/svg/log-out-01.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const DUTY: StaticCommand = StaticCommand {
    name: "/duty",
    description: "Claim a callsign so direct radio reaches this tab",
    icon_path: "bundled/svg/stars-01.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: Some(Argument {
        hint_text: Some("<callsign>"),
        is_optional: true,
        should_execute_on_selection: false,
    }),
};

pub static COMMAND_REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// A unique identifier for a static slash command.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SlashCommandId(Uuid);

impl SlashCommandId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SlashCommandId {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Registry {
    commands: HashMap<SlashCommandId, StaticCommand>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let mut commands = HashMap::new();
        for command in all_commands().into_iter() {
            debug_assert!(
                !command
                    .availability
                    .contains(Availability::TERMINAL_VIEW | Availability::AGENT_VIEW),
                "command `{}` sets both TERMINAL_VIEW and AGENT_VIEW, which is unsatisfiable",
                command.name,
            );
            commands.insert(SlashCommandId::new(), command);
        }
        Self { commands }
    }

    pub fn all_commands_by_id(&self) -> impl Iterator<Item = (SlashCommandId, &StaticCommand)> {
        self.commands.iter().map(|(id, cmd)| (*id, cmd))
    }

    pub fn all_commands(&self) -> impl Iterator<Item = &StaticCommand> {
        self.commands.values()
    }

    pub fn get_command(&self, id: &SlashCommandId) -> Option<&StaticCommand> {
        self.commands.get(id)
    }

    pub fn get_command_with_name(&self, name: &str) -> Option<&StaticCommand> {
        self.commands.values().find(|command| command.name == name)
    }

    #[cfg(test)]
    pub fn get_command_id_with_name(&self, name: &str) -> Option<&SlashCommandId> {
        self.commands
            .iter()
            .find(|(_, command)| command.name == name)
            .map(|(id, _)| id)
    }
}

fn all_commands() -> Vec<StaticCommand> {
    let mut commands = vec![
        ADD_MCP,
        ADD_PROMPT.clone(),
        ADD_RULE,
        COST,
        FEEDBACK.clone(),
        INDEX,
        INIT,
        OPEN_PROJECT_RULES,
        OPEN_MCP_SERVERS,
        OPEN_RULES,
        DETECTIVE.clone(),
        AGENT.clone(),
        NEW.clone(),
        PLAN.clone(),
        RENAME_TAB.clone(),
        USAGE,
        CONVERSATIONS,
        EXPORT_TO_CLIPBOARD,
        MODEL.clone(),
        RADIO,
        INBOX,
        RESPOND,
        ROSTER,
        SITREP,
        CASE,
        CLOCK_OUT,
        DUTY,
    ];

    if FeatureFlag::LocalDockerSandbox.is_enabled() {
        commands.push(CREATE_DOCKER_SANDBOX);
    }

    if FeatureFlag::CreatingSharedSessions.is_enabled()
        && FeatureFlag::HOARemoteControl.is_enabled()
    {
        commands.push(REMOTE_CONTROL);
    }

    if FeatureFlag::Changelog.is_enabled() {
        commands.push(CHANGELOG);
    }

    if FeatureFlag::AgentView.is_enabled() {
        commands.push(PROMPTS.clone());
    }

    commands.push(OPEN_CODE_REVIEW);

    if FeatureFlag::CreateEnvironmentSlashCommand.is_enabled() {
        commands.push(CREATE_ENVIRONMENT.clone());
    }

    if FeatureFlag::CreateProjectFlow.is_enabled() {
        commands.push(CREATE_NEW_PROJECT.clone());
    }

    if FeatureFlag::SummarizationConversationCommand.is_enabled() {
        commands.push(COMPACT.clone());
        commands.push(COMPACT_AND.clone());
    }

    if FeatureFlag::QueueSlashCommand.is_enabled() {
        commands.push(QUEUE.clone());
    }

    if !cfg!(target_family = "wasm") {
        commands.extend([FORK.clone(), FORK_AND_COMPACT.clone()]);

        if FeatureFlag::ForkFromCommand.is_enabled() {
            commands.push(FORK_FROM);
        }
    }

    if !cfg!(target_family = "wasm") {
        commands.extend([EDIT.clone(), EXPORT_TO_FILE.clone()]);
    }

    if FeatureFlag::ListSkills.is_enabled() && !cfg!(target_family = "wasm") {
        commands.push(EDIT_SKILL.clone());
        commands.push(INVOKE_SKILL.clone());
    }

    if FeatureFlag::PRCommentsSlashCommand.is_enabled()
        && !FeatureFlag::PRCommentsSkill.is_enabled()
    {
        commands.push(PR_COMMENTS);
    }

    if FeatureFlag::CloudMode.is_enabled() && FeatureFlag::CloudModeFromLocalSession.is_enabled() {
        commands.push(CLOUD_DETECTIVE.clone());
        commands.push(CLOUD_AGENT.clone());
    }

    if FeatureFlag::InlineProfileSelector.is_enabled() {
        commands.push(PROFILE.clone());
    }

    if FeatureFlag::RevertToCheckpoints.is_enabled() && FeatureFlag::RewindSlashCommand.is_enabled()
    {
        commands.push(REWIND);
    }

    if FeatureFlag::InlineRepoMenu.is_enabled() && !cfg!(target_family = "wasm") {
        commands.push(OPEN_REPO);
    }

    if FeatureFlag::Orchestration.is_enabled() {
        commands.push(ORCHESTRATE.clone());
    }

    if FeatureFlag::SettingsFile.is_enabled() && cfg!(feature = "local_fs") {
        commands.push(OPEN_SETTINGS_FILE);
    }

    commands
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn command_names_are_unique() {
        let names = COMMAND_REGISTRY.all_commands().map(|command| command.name);
        let mut seen = HashSet::new();
        for name in names {
            assert!(seen.insert(name), "duplicate slash command name: {name}");
        }
    }

    #[test]
    fn detective_aliases_are_registered() {
        assert!(
            COMMAND_REGISTRY
                .get_command_with_name(DETECTIVE.name)
                .is_some(),
            "expected /detective to be registered"
        );

        if FeatureFlag::CloudMode.is_enabled()
            && FeatureFlag::CloudModeFromLocalSession.is_enabled()
        {
            assert!(
                COMMAND_REGISTRY
                    .get_command_with_name(CLOUD_DETECTIVE.name)
                    .is_some(),
                "expected /cloud-detective to be registered"
            );
        }
    }

    #[test]
    fn rename_tab_command_requires_argument() {
        let command = COMMAND_REGISTRY
            .get_command_with_name(RENAME_TAB.name)
            .expect("expected /rename-tab to be registered");
        let argument = command
            .argument
            .as_ref()
            .expect("expected /rename-tab to require an argument");

        assert!(!argument.is_optional);
        assert!(!argument.should_execute_on_selection);
        assert_eq!(argument.hint_text, Some("<tab name>"));
    }

    #[test]
    fn strip_command_prefix_matches_orchestrate() {
        let result = strip_command_prefix("/orchestrate deploy services", "/orchestrate");
        assert_eq!(result, Some("deploy services".to_string()));
    }

    #[test]
    fn strip_command_prefix_no_match() {
        let result = strip_command_prefix("just a normal query", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_empty() {
        let result = strip_command_prefix("", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_no_trailing_space() {
        // "/plan" alone (no trailing space) should NOT be stripped
        let result = strip_command_prefix("/plan", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_trailing_space_only() {
        // "/plan " with nothing after should strip to empty string
        let result = strip_command_prefix("/plan ", "/plan");
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn strip_command_prefix_substring_not_matched() {
        // "/planning" should not match "/plan"
        let result = strip_command_prefix("/planning something", "/plan");
        assert_eq!(result, None);
    }
}

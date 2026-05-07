mod data_source;
mod search_item;
mod view;

pub use data_source::*;
pub use view::*;

use ai::skills::SkillReference;
use yarp_core::features::FeatureFlag;
use yarp_core::send_telemetry_from_ctx;
use yarp_core::ui::appearance::Appearance;
use yarpui::clipboard::ClipboardContent;
use yarpui::{SingletonEntity, ViewContext};

use crate::ai::blocklist::agent_view::{
    AgentViewEntryOrigin, DismissalStrategy, EphemeralMessage, ENTER_OR_EXIT_CONFIRMATION_WINDOW,
};
use crate::ai::blocklist::{BlocklistAIHistoryModel, SlashCommandRequest};
use crate::cloud_object::model::persistence::CloudModel;
use crate::code_review::telemetry_event::CodeReviewPaneEntrypoint;
use crate::search::slash_command_menu::static_commands::commands::{self, COMMAND_REGISTRY};
use crate::search::slash_command_menu::static_commands::Availability;
use crate::search::slash_command_menu::{SlashCommandId, StaticCommand};
use crate::server::ids::SyncId;
use crate::server::telemetry::SlashCommandAcceptedDetails;
use crate::settings::AISettings;
use crate::terminal::input::decorations::InputBackgroundJobOptions;
use crate::terminal::input::inline_menu::{InlineMenuAction, InlineMenuType};
use crate::terminal::input::message_bar::Message;
use crate::terminal::input::slash_command_model::{
    SlashCommandEntryState, UpdatedSlashCommandModel,
};
use crate::terminal::input::{
    CompletionsTrigger, Event, Input, InputSuggestionsMode, UserQueryMenuAction,
};
use crate::terminal::view::TerminalAction;
use crate::view_components::DismissibleToast;
use crate::workflows::{WorkflowSelectionSource, WorkflowSource, WorkflowType};
use crate::workspace::{ForkedConversationDestination, ToastStack, WorkspaceAction};
use crate::TelemetryEvent;

#[derive(Debug, Clone)]
pub enum AcceptSlashCommandOrSavedPrompt {
    SlashCommand {
        id: SlashCommandId,
    },
    SavedPrompt {
        id: SyncId,
    },
    /// A skill selected from browse or search. Contains name (for display/insertion) and path/bundled_skill_id (for execution).
    Skill {
        reference: SkillReference,
        name: String,
    },
}
impl InlineMenuAction for AcceptSlashCommandOrSavedPrompt {
    const MENU_TYPE: InlineMenuType = InlineMenuType::SlashCommands;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SlashCommandTrigger {
    Input { cmd_or_ctrl_enter: bool },
    Keybinding,
}

impl SlashCommandTrigger {
    fn cmd_or_ctrl_enter() -> Self {
        Self::Input {
            cmd_or_ctrl_enter: true,
        }
    }

    pub fn input() -> Self {
        Self::Input {
            cmd_or_ctrl_enter: false,
        }
    }

    pub(super) fn keybinding() -> Self {
        Self::Keybinding
    }

    pub fn is_keybinding(&self) -> bool {
        matches!(self, Self::Keybinding)
    }

    fn is_cmd_or_ctrl_enter(&self) -> bool {
        matches!(
            self,
            Self::Input {
                cmd_or_ctrl_enter: true
            }
        )
    }
}

impl Input {
    pub(super) fn select_slash_command(
        &mut self,
        command: &StaticCommand,
        trigger: SlashCommandTrigger,
        ctx: &mut ViewContext<Self>,
    ) {
        if command.argument.as_ref().is_none() {
            self.execute_slash_command(
                command, None, trigger, /*is_queued_prompt*/ false, ctx,
            );
        } else if command
            .argument
            .as_ref()
            .is_some_and(|arg| arg.should_execute_on_selection)
        {
            // TODO (zachbai): this is a hack for Fuzz launch. Caller
            // should probably be invoking `execute_slash_command` in this case.
            let argument = if !self.suggestions_mode_model.as_ref(ctx).is_slash_commands() {
                let trimmed = self.buffer_text(ctx).trim().to_owned();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            };
            self.execute_slash_command(
                command,
                argument.as_ref(),
                trigger,
                /*is_queued_prompt*/ false,
                ctx,
            );
        } else {
            self.editor.update(ctx, |editor, ctx| {
                editor.set_buffer_text(&format!("{} ", command.name), ctx);
            });
        }
    }

    pub(super) fn close_slash_commands_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.suggestions_mode_model.update(ctx, |model, ctx| {
            model.set_mode(InputSuggestionsMode::Closed, ctx);
        });
        ctx.notify();
    }

    pub(super) fn handle_slash_command_model_event(
        &mut self,
        event: &UpdatedSlashCommandModel,
        ctx: &mut ViewContext<Self>,
    ) {
        // Refresh decorations if the slash command detection state changed, since
        // detected commands affect syntax highlighting.
        let new_state = self.slash_command_model.as_ref(ctx).state();
        if event.old_state.is_detected_command() != new_state.is_detected_command() {
            let _ = self
                .debounce_input_background_tx
                .try_send(InputBackgroundJobOptions::default().with_command_decoration());
        }

        match self.slash_command_model.as_ref(ctx).state().clone() {
            SlashCommandEntryState::None | SlashCommandEntryState::DisabledUntilEmptyBuffer => {
                if self.suggestions_mode_model.as_ref(ctx).is_slash_commands() {
                    self.close_slash_commands_menu(ctx);
                }
            }
            SlashCommandEntryState::Composing { .. } => {
                if self.suggestions_mode_model.as_ref(ctx).is_closed() {
                    self.open_slash_commands_menu(ctx);
                } else if !self.suggestions_mode_model.as_ref(ctx).is_slash_commands() {
                    self.slash_command_model.update(ctx, |model, ctx| {
                        model.disable(ctx);
                    });
                }
            }
            SlashCommandEntryState::SlashCommand(detected_command) => {
                // If there is only one result (or zero, but that should be impossible if there is
                // a valid command in the input) OR if the user has started typing arguments, hide
                // the menu.
                if self.suggestions_mode_model.as_ref(ctx).is_slash_commands()
                    && (self
                        .inline_slash_commands_view
                        .as_ref(ctx)
                        .result_count(ctx)
                        < 2
                        || detected_command.argument.is_some())
                {
                    self.close_slash_commands_menu(ctx);
                }

                if detected_command.command.auto_enter_ai_mode
                    || !FeatureFlag::AgentView.is_enabled()
                {
                    self.enter_ai_mode(ctx);
                }

                if detected_command.command.name == commands::EDIT.name
                    && detected_command
                        .argument
                        .as_ref()
                        .is_some_and(|argument| argument.is_empty())
                    && self.suggestions_mode_model.as_ref(ctx).is_closed()
                {
                    self.open_completion_suggestions(CompletionsTrigger::Keybinding, ctx);
                }
            }
            SlashCommandEntryState::SkillCommand(detected_skill) => {
                // Hide the menu once the user has started typing the prompt
                if self.suggestions_mode_model.as_ref(ctx).is_slash_commands()
                    && (self
                        .inline_slash_commands_view
                        .as_ref(ctx)
                        .result_count(ctx)
                        < 2
                        || detected_skill.argument.is_some())
                {
                    self.close_slash_commands_menu(ctx);
                }

                // Skill commands always require AI mode
                self.enter_ai_mode(ctx);
            }
        }
    }

    pub(crate) fn handle_slash_commands_menu_event(
        &mut self,
        event: &SlashCommandsEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SlashCommandsEvent::Close(reason) => {
                if reason.is_manual_dismissal() {
                    self.slash_command_model.update(ctx, |model, ctx| {
                        model.disable(ctx);
                    });
                }

                self.suggestions_mode_model.update(ctx, |model, ctx| {
                    model.set_mode(InputSuggestionsMode::Closed, ctx);
                });
                ctx.notify();
            }
            SlashCommandsEvent::SelectedSavedPrompt { id } => {
                let Some(workflow) = CloudModel::as_ref(ctx).get_workflow(id).cloned() else {
                    log::warn!("Tried to execute workflow for id {id:?} but it does not exist");
                    return;
                };
                let is_in_agent_view = FeatureFlag::AgentView.is_enabled()
                    && self.agent_view_controller.as_ref(ctx).is_fullscreen();
                send_telemetry_from_ctx!(
                    TelemetryEvent::SlashCommandAccepted {
                        command_details: SlashCommandAcceptedDetails::SavedPrompt,
                        is_in_agent_view,
                    },
                    ctx
                );

                self.show_workflows_info_box_on_workflow_selection(
                    WorkflowType::Cloud(Box::new(workflow)),
                    WorkflowSource::YarpAI,
                    WorkflowSelectionSource::SlashMenu,
                    None,
                    ctx,
                );
            }
            SlashCommandsEvent::SelectedStaticCommand {
                id,
                cmd_or_ctrl_enter,
            } => {
                let Some(command) = COMMAND_REGISTRY.get_command(id) else {
                    return;
                };
                self.select_slash_command(
                    command,
                    SlashCommandTrigger::Input {
                        cmd_or_ctrl_enter: *cmd_or_ctrl_enter,
                    },
                    ctx,
                );
            }
            SlashCommandsEvent::SelectedSkill { name, reference: _ } => {
                // Insert /{skill-name} into the buffer
                self.editor.update(ctx, |editor, ctx| {
                    editor.set_buffer_text(format!("/{name} ").as_str(), ctx);
                });
                self.close_slash_commands_menu(ctx);
            }
        }
    }

    /// Executes the given `command` with `argument`, if any.
    ///
    /// When `is_queued_prompt` is true, this is the first send of a previously queued prompt:
    /// the input buffer is left alone so the user doesn't lose anything they've typed while
    /// the agent was busy.
    ///
    /// Returns `true` if execution was 'handled' (whether or not it resulted in success or failure).
    pub(super) fn execute_slash_command(
        &mut self,
        command: &StaticCommand,
        argument: Option<&String>,
        trigger: SlashCommandTrigger,
        is_queued_prompt: bool,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        fn show_error_toast(message: String, ctx: &mut ViewContext<Input>) {
            let window_id = ctx.window_id();
            ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                toast_stack.add_ephemeral_toast(DismissibleToast::error(message), window_id, ctx);
            });
        }

        // Safety net: commands whose availability requires AI should not execute when AI is
        // globally disabled. They're normally filtered out of the slash command menu, but this
        // protects keybinding-triggered execution where a bound key may still address the command.
        if command.availability.contains(Availability::AI_ENABLED)
            && !AISettings::as_ref(ctx).is_any_ai_enabled(ctx)
        {
            show_error_toast(format!("{} requires AI to be enabled", command.name), ctx);
            return true;
        }

        // Handle the slash command action based on its kind
        match command.name {
            add_mcp if command.name == commands::ADD_MCP.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenAddMCPPane);
            }
            add_prompt if command.name == commands::ADD_PROMPT.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenAddPromptPane);
            }
            add_rule if command.name == commands::ADD_RULE.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenAddRulePane);
            }
            agent_or_new
                if command.name == commands::NEW.name || command.name == commands::AGENT.name =>
            {
                if !self
                    .ai_context_model
                    .as_ref(ctx)
                    .can_start_new_conversation()
                {
                    self.ephemeral_message_model.update(ctx, |model, ctx| {
                        let appearance = Appearance::handle(ctx).as_ref(ctx);
                        let message = Message::from_text(
                            "cannot start new conversation while terminal command is running",
                        )
                        .with_text_color(appearance.theme().ansi_fg_red());
                        model.show_ephemeral_message(
                            EphemeralMessage::new(
                                message,
                                DismissalStrategy::Timer(ENTER_OR_EXIT_CONFIRMATION_WINDOW),
                            ),
                            ctx,
                        );
                    });
                    return true;
                }
                // Keybindings can be triggered reflexively while users are already in an active
                // conversation, so we gate only this path behind a second-press confirmation.
                // Typed `/agent`/`/new` and slash-menu execution stay single-step by design.
                if trigger.is_keybinding() && self.agent_view_controller.as_ref(ctx).is_active() {
                    let should_start_new_conversation =
                        self.agent_view_controller.update(ctx, |controller, ctx| {
                            controller
                                .should_start_new_conversation_for_keybinding(command.name, ctx)
                        });
                    if !should_start_new_conversation {
                        // Keep the current input/conversation untouched on first press; only the
                        // ephemeral confirmation prompt should change.
                        return true;
                    }
                }

                let prompt = argument.and_then(|argument| {
                    let trimmed = argument.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                });

                // /agent convenes the Sandford NWA council. If the team has
                // any CLI-backed personas, emit EnterAgentCouncil so the
                // native council view takes over. Otherwise fall through to
                // the standard agent view.
                if command.name == commands::AGENT.name {
                    if let Some(prompt) = prompt.as_ref() {
                        if let Some(roster) = crate::personas::Roster::load() {
                            if let Some(team) = roster.default_team() {
                                if !crate::personas::cli_invocations(team).is_empty() {
                                    ctx.emit(Event::EnterAgentCouncil {
                                        prompt: prompt.clone(),
                                    });
                                    return true;
                                }
                            }
                        }
                    }
                }

                ctx.emit(Event::EnterAgentView {
                    initial_prompt: prompt,
                    conversation_id: None,
                    origin: AgentViewEntryOrigin::SlashCommand { trigger },
                });
            }
            cloud_agent if command.name == commands::CLOUD_AGENT.name => {
                let prompt = argument.and_then(|argument| {
                    let trimmed = argument.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                });

                ctx.emit(Event::EnterCloudAgentView {
                    initial_prompt: prompt,
                });
            }
            create_docker_sandbox if command.name == commands::CREATE_DOCKER_SANDBOX.name => {
                ctx.emit(Event::CreateDockerSandbox);
            }
            conversations if command.name == commands::CONVERSATIONS.name => {
                if FeatureFlag::AgentView.is_enabled() {
                    self.open_conversation_menu(ctx);
                } else {
                    ctx.dispatch_typed_action(&TerminalAction::OpenConversationsPalette);
                }
            }
            rename_tab if command.name == commands::RENAME_TAB.name => {
                let Some(name) = argument
                    .map(|name| name.trim())
                    .filter(|name| !name.is_empty())
                else {
                    show_error_toast(
                        "Please provide a tab name after /rename-tab".to_owned(),
                        ctx,
                    );
                    return true;
                };

                ctx.dispatch_typed_action(&WorkspaceAction::SetActiveTabName(name.to_owned()));
            }
            radio if command.name == commands::RADIO.name => {
                let Some(raw) = argument
                    .map(|a| a.trim())
                    .filter(|a| !a.is_empty())
                else {
                    show_error_toast(
                        "Please provide a message: /radio [@unit] <message>".to_owned(),
                        ctx,
                    );
                    return true;
                };
                let (target, message) = match raw.strip_prefix('@') {
                    Some(rest) => match rest.split_once(char::is_whitespace) {
                        Some((t, body)) => (t.trim(), body.trim()),
                        None => (rest.trim(), ""),
                    },
                    None => ("", raw),
                };
                if message.is_empty() {
                    show_error_toast(
                        "Please provide a message body: /radio @unit <message>".to_owned(),
                        ctx,
                    );
                    return true;
                }
                let cmd = format!(
                    "mkdir -p /tmp/yarp-radio; \
                     ts=$(date +%s%N 2>/dev/null | cut -c1-13); \
                     [ -z \"$ts\" ] && ts=$(date +%s)000; \
                     file=/tmp/yarp-radio/${{ts}}-$$-${{RANDOM}}${{RANDOM}}.msg; \
                     sender=\"${{USER:-unknown}}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                     target={target}; \
                     {{ \
                       printf 'from: %s\\n' \"$sender\"; \
                       [ -n \"$target\" ] && printf 'to: %s\\n' \"$target\"; \
                       printf '\\n'; \
                       printf '%s' {body}; \
                     }} > \"$file\"; \
                     if [ -n \"$target\" ]; then \
                       printf '\\033[1;38;5;220m📻 DISPATCH\\033[0m \\033[3;38;5;244m%s → @%s\\033[0m\\n  \\033[38;5;178m\"%s\"\\033[0m\\n' \"$sender\" \"$target\" {body}; \
                     else \
                       printf '\\033[1;38;5;220m📻 RADIO\\033[0m \\033[3;38;5;244m%s → all units\\033[0m\\n  \\033[38;5;178m\"%s\"\\033[0m\\n' \"$sender\" {body}; \
                     fi",
                    target = crate::personas::shell_quote_one(target),
                    body = crate::personas::shell_quote_one(message),
                );
                self.try_execute_command(&cmd, ctx);
            }
            roster if command.name == commands::ROSTER.name => {
                let roster = crate::personas::Roster::load()
                    .unwrap_or_else(crate::personas::Roster::default_sandford);
                let team = match roster.default_team() {
                    Some(team) => team,
                    None => {
                        show_error_toast("No team configured in personas.json".to_owned(), ctx);
                        return true;
                    }
                };
                let count = team.members.len();
                let header = format!(
                    "printf '\\033[1;38;5;220m👮 ROSTER\\033[0m \\033[3;38;5;244m%s — %d \
                     personas\\033[0m\\n\\n' {team} {count}; ",
                    team = crate::personas::shell_quote_one(&team.name),
                    count = count,
                );
                let mut script = String::from(header);
                for p in &team.members {
                    let role = p
                        .role
                        .split('.')
                        .next()
                        .unwrap_or(&p.role)
                        .trim();
                    let (badge_color, name_color) = if p.lead {
                        ("38;5;220", "1;38;5;220")
                    } else if p.binary.is_some() {
                        ("38;5;39", "1;38;5;39")
                    } else {
                        ("38;5;178", "1;38;5;178")
                    };
                    let lead_tag = if p.lead {
                        " \\033[3;38;5;220m⭐ lead\\033[0m"
                    } else {
                        ""
                    };
                    let line = format!(
                        "printf '  \\033[{badge_color}m%-4s\\033[0m \\033[{name_color}m%-18s\\033[0m \
                         \\033[3;38;5;244m%s\\033[0m{lead_tag}\\n' {badge} {name} {role}; ",
                        badge_color = badge_color,
                        name_color = name_color,
                        lead_tag = lead_tag,
                        badge = crate::personas::shell_quote_one(&p.badge),
                        name = crate::personas::shell_quote_one(&p.name),
                        role = crate::personas::shell_quote_one(role),
                    );
                    script.push_str(&line);
                    if let Some(bin) = p.binary.as_deref() {
                        let bin_line = format!(
                            "printf '       \\033[2;38;5;244m↳ %s\\033[0m\\n' {bin}; ",
                            bin = crate::personas::shell_quote_one(bin),
                        );
                        script.push_str(&bin_line);
                    }
                }
                self.try_execute_command(&script, ctx);
            }
            case if command.name == commands::CASE.name => {
                let Some(name) = argument
                    .map(|n| n.trim())
                    .filter(|n| !n.is_empty())
                else {
                    show_error_toast(
                        "Please name the case: /case <name>".to_owned(),
                        ctx,
                    );
                    return true;
                };
                let tab_name = format!("Case: {}", name);
                ctx.dispatch_typed_action(&WorkspaceAction::SetActiveTabName(tab_name.clone()));
                let cmd = format!(
                    "mkdir -p /tmp/yarp-radio; \
                     ts=$(date +%s%N 2>/dev/null | cut -c1-13); \
                     [ -z \"$ts\" ] && ts=$(date +%s)000; \
                     file=/tmp/yarp-radio/${{ts}}-$$-${{RANDOM}}${{RANDOM}}.msg; \
                     sender=\"${{USER:-unknown}}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                     {{ \
                       printf 'from: %s\\n' \"$sender\"; \
                       printf '\\n'; \
                       printf 'case opened: %s' {name}; \
                     }} > \"$file\"; \
                     printf '\\033[1;38;5;220m📁 CASE OPENED\\033[0m \\033[3;38;5;244m%s\\033[0m\\n  \\033[38;5;178mtab → \"%s\"\\033[0m\\n  \\033[3;38;5;244mall units notified\\033[0m\\n' {name} {tab}",
                    name = crate::personas::shell_quote_one(name),
                    tab = crate::personas::shell_quote_one(&tab_name),
                );
                self.try_execute_command(&cmd, ctx);
            }
            _duty if command.name == commands::DUTY.name => {
                let callsign = argument
                    .map(|a| a.trim())
                    .filter(|a| !a.is_empty());
                let cmd = if let Some(cs) = callsign {
                    if cs.eq_ignore_ascii_case("off") || cs.eq_ignore_ascii_case("clear") {
                        "unset YARP_CALLSIGN; printf '\\033[3;38;5;244m🎖  off duty — callsign cleared\\033[0m\\n'".to_owned()
                    } else {
                        format!(
                            "export YARP_CALLSIGN={cs}; \
                             printf '\\033[1;38;5;220m🎖  ON DUTY\\033[0m \\033[3;38;5;244mcallsign claimed\\033[0m\\n  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m@%s\\033[0m\\n  \\033[3;38;5;244m/inbox will now flag traffic addressed to @%s as DIRECT\\033[0m\\n' 'unit' {cs} {cs}",
                            cs = crate::personas::shell_quote_one(cs),
                        )
                    }
                } else {
                    "if [ -n \"$YARP_CALLSIGN\" ]; then \
                       printf '\\033[1;38;5;220m🎖  ON DUTY\\033[0m \\033[3;38;5;244mcurrent callsign\\033[0m\\n  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m@%s\\033[0m\\n  \\033[3;38;5;244m/duty <name> to change · /duty off to clear\\033[0m\\n' 'unit' \"$YARP_CALLSIGN\"; \
                     else \
                       printf '\\033[3;38;5;179m🎖  no callsign claimed\\033[0m\\n  \\033[3;38;5;244m/duty <name> to claim one — direct radio routes by callsign\\033[0m\\n'; \
                     fi"
                        .to_owned()
                };
                self.try_execute_command(&cmd, ctx);
            }
            _clock_out if command.name == commands::CLOCK_OUT.name => {
                const SIGN_OFFS: &[&str] = &[
                    "Pub? — Danny",
                    "By the power of Greyskull. — Danny",
                    "Forget it, Nicholas, it's Sandford. — Frank",
                    "Yarp. — Michael",
                    "The greater good. — The NWA",
                    "Have a nice evening. — Angel",
                ];
                let quote = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let nanos = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0);
                    SIGN_OFFS[nanos % SIGN_OFFS.len()]
                };
                let cmd = format!(
                    "user=\"${{USER:-unknown}}\"; \
                     host=\"$(hostname -s 2>/dev/null || echo localhost)\"; \
                     when=\"$(date '+%a %H:%M' 2>/dev/null || date)\"; \
                     prev_call=\"${{YARP_CALLSIGN:-}}\"; \
                     unset YARP_CALLSIGN; \
                     shopt -s nullglob; \
                     queue=(/tmp/yarp-radio/*.msg); \
                     n=${{#queue[@]}}; \
                     if [ \"$n\" -gt 0 ]; then rm -f /tmp/yarp-radio/*.msg; fi; \
                     printf '\\033[1;38;5;220m🌙 OFF DUTY\\033[0m \\033[3;38;5;244m%s\\033[0m\\n' \"$when\"; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m%s@%s\\033[0m\\n' 'officer' \"$user\" \"$host\"; \
                     if [ -n \"$prev_call\" ]; then \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m@%s\\033[0m \\033[3;38;5;244mcleared\\033[0m\\n' 'callsign' \"$prev_call\"; \
                     fi; \
                     if [ \"$n\" -gt 0 ]; then \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m%d transmission(s) cleared\\033[0m\\n' 'radio' \"$n\"; \
                     else \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[3;38;5;244mqueue already empty\\033[0m\\n' 'radio'; \
                     fi; \
                     printf '\\n  \\033[3;38;5;244m“%s”\\033[0m\\n' {quote}",
                    quote = crate::personas::shell_quote_one(quote),
                );
                self.try_execute_command(&cmd, ctx);
            }
            sitrep if command.name == commands::SITREP.name => {
                let roster = crate::personas::Roster::load()
                    .unwrap_or_else(crate::personas::Roster::default_sandford);
                let team_name = roster
                    .default_team()
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "—".into());
                let team_size = roster.default_team().map(|t| t.members.len()).unwrap_or(0);
                let cli_count = roster
                    .default_team()
                    .map(|t| crate::personas::cli_invocations(t).len())
                    .unwrap_or(0);
                const QUOTES: &[&str] = &[
                    "By the power of Greyskull. — Danny",
                    "It's not Sunday, the gun shop's shut. — Angel",
                    "The greater good. — The NWA",
                    "Have you ever fired two guns whilst jumping through the air? — Danny",
                    "Yarp. — Michael",
                    "Murder, murder, murder. — Angel",
                    "Forget it, Nicholas, it's Sandford. — Frank",
                    "Pub? — Danny",
                ];
                let quote = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let nanos = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0);
                    QUOTES[nanos % QUOTES.len()]
                };
                let cmd = format!(
                    "user=\"${{USER:-unknown}}\"; \
                     host=\"$(hostname -s 2>/dev/null || echo localhost)\"; \
                     cwd=\"$(pwd)\"; \
                     shopt -s nullglob; \
                     queue=(/tmp/yarp-radio/*.msg); \
                     printf '\\033[1;38;5;220m🚓 SITREP\\033[0m \\033[3;38;5;244m%s station\\033[0m\\n\\n' {team}; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m%s@%s\\033[0m\\n' 'officer' \"$user\" \"$host\"; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m%s\\033[0m\\n' 'beat' \"$cwd\"; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;178m%s\\033[0m \\033[3;38;5;244m(%s CLI-backed)\\033[0m\\n' 'roster' {size} {clis}; \
                     n=${{#queue[@]}}; \
                     if [ \"$n\" -gt 0 ]; then \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[1;38;5;220m%d transmission(s) pending\\033[0m \\033[3;38;5;244m— /inbox to read\\033[0m\\n' 'radio' \"$n\"; \
                     else \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[3;38;5;244mall quiet on the air\\033[0m\\n' 'radio'; \
                     fi; \
                     printf '\\n  \\033[3;38;5;244m“%s”\\033[0m\\n' {quote}",
                    team = crate::personas::shell_quote_one(&team_name),
                    size = team_size,
                    clis = cli_count,
                    quote = crate::personas::shell_quote_one(quote),
                );
                self.try_execute_command(&cmd, ctx);
            }
            inbox if command.name == commands::INBOX.name => {
                let cmd = "shopt -s nullglob; \
                    mkdir -p /tmp/yarp-radio; \
                    pruned=$(find /tmp/yarp-radio -maxdepth 1 -name '*.msg' -mmin +60 -print -delete 2>/dev/null | wc -l | tr -d ' '); \
                    files=(/tmp/yarp-radio/*.msg); \
                    me_user=\"${USER:-unknown}\"; \
                    me_full=\"${me_user}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                    me_call=\"${YARP_CALLSIGN:-}\"; \
                    if [ ${#files[@]} -eq 0 ]; then \
                      if [ \"${pruned:-0}\" -gt 0 ]; then \
                        printf '\\033[3;38;5;244m📻 INBOX  no traffic \\033[0m\\033[2;38;5;240m· expired %s stale\\033[0m\\n' \"$pruned\"; \
                      else \
                        printf '\\033[3;38;5;244m📻 INBOX  no traffic\\033[0m\\n'; \
                      fi; \
                    else \
                      if [ \"${pruned:-0}\" -gt 0 ]; then \
                        printf '\\033[1;38;5;220m📻 INBOX\\033[0m \\033[3;38;5;244m%d transmission(s)\\033[0m \\033[2;38;5;240m· expired %s stale\\033[0m\\n\\n' \"${#files[@]}\" \"$pruned\"; \
                      else \
                        printf '\\033[1;38;5;220m📻 INBOX\\033[0m \\033[3;38;5;244m%d transmission(s)\\033[0m\\n\\n' \"${#files[@]}\"; \
                      fi; \
                      for f in \"${files[@]}\"; do \
                        base=$(basename \"$f\" .msg); \
                        ts=${base%%-*}; \
                        rest=${base#*-}; \
                        pid=${rest%%-*}; \
                        ts_s=$((ts / 1000)); \
                        human=$(date -r \"$ts_s\" '+%H:%M:%S' 2>/dev/null || echo \"$ts\"); \
                        sender=\"unknown\"; target=\"\"; body_start=1; \
                        while IFS= read -r line; do \
                          case \"$line\" in \
                            from:*) sender=\"${line#from: }\"; body_start=$((body_start+1));; \
                            to:*)   target=\"${line#to: }\";   body_start=$((body_start+1));; \
                            \"\")    body_start=$((body_start+1)); break;; \
                            *)      break;; \
                          esac; \
                        done < \"$f\"; \
                        consume=1; \
                        if [ -n \"$target\" ]; then \
                          if [ \"$target\" = \"$me_user\" ] || [ \"$target\" = \"$me_full\" ] || { [ -n \"$me_call\" ] && [ \"$target\" = \"$me_call\" ]; }; then \
                            badge='\\033[1;38;5;35m▸ DIRECT\\033[0m '; \
                            sender_color='\\033[1;38;5;35m'; \
                            body_color='\\033[38;5;179m'; \
                          else \
                            badge='\\033[2;38;5;240m▸ relay  \\033[0m '; \
                            sender_color='\\033[2;38;5;240m'; \
                            body_color='\\033[2;38;5;240m'; \
                            consume=0; \
                          fi; \
                          tag=\" → @${target}\"; \
                        else \
                          badge='\\033[1;38;5;220m📻 ALL   \\033[0m '; \
                          sender_color='\\033[1;38;5;220m'; \
                          body_color='\\033[38;5;178m'; \
                          tag=''; \
                        fi; \
                        printf '  %b\\033[2;38;5;244m[%s pid=%s]\\033[0m %b%s\\033[0m\\033[3;38;5;244m%s\\033[0m\\n' \"$badge\" \"$human\" \"$pid\" \"$sender_color\" \"$sender\" \"$tag\"; \
                        tail -n +$body_start \"$f\" 2>/dev/null | while IFS= read -r line; do printf '    %b%s\\033[0m\\n' \"$body_color\" \"$line\"; done; \
                        if [ \"$consume\" = 1 ]; then rm -f \"$f\"; fi; \
                        echo; \
                      done; \
                    fi";
                self.try_execute_command(cmd, ctx);
            }
            create_env if command.name == commands::CREATE_ENVIRONMENT.name => {
                // If the user included args after the slash command, treat them as repo paths/URLs.
                let repos = argument
                    .map(|arg| {
                        arg.split_whitespace()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();

                ctx.emit(Event::TriggerEnvironmentSetup { repos });
            }
            create_project if command.name == commands::CREATE_NEW_PROJECT.name => {
                if argument.is_none_or(|args| args.is_empty()) {
                    show_error_toast(
                        "Please describe the project you want to create after /create-new-project"
                            .to_owned(),
                        ctx,
                    );
                    return true;
                }

                let args = argument.expect("args are Some()");
                self.initiate_create_new_project(args.to_owned(), ctx);
            }
            edit if command.name == commands::EDIT.name => {
                #[cfg(feature = "local_fs")]
                match argument {
                    Some(args) if !args.is_empty() => {
                        use shellexpand::tilde;
                        use yarp_util::path::CleanPathResult;

                        let Some(session_id) = self.active_block_session_id() else {
                            return false;
                        };

                        let Some(session) = self.sessions.as_ref(ctx).get(session_id) else {
                            return false;
                        };

                        if !session.is_local() {
                            let window_id = ctx.window_id();
                            ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                                toast_stack.add_ephemeral_toast(
                                    DismissibleToast::error(
                                        "The /open-file command is only available for local sessions"
                                            .to_owned(),
                                    ),
                                    window_id,
                                    ctx,
                                );
                            });
                            return false;
                        }

                        let current_dir = self
                            .active_block_metadata
                            .as_ref()
                            .and_then(|metadata| metadata.current_working_directory())
                            .map(std::path::PathBuf::from);

                        let Some(current_dir) = current_dir else {
                            return false;
                        };

                        let parsed_path = CleanPathResult::with_line_and_column_number(args.trim());
                        // The argument may contain shell-escaped characters (e.g. `\ ` for
                        // spaces) from auto-suggest. Unescape them so the path matches the
                        // actual filesystem entry.
                        let unescaped_path = session.shell_family().unescape(&parsed_path.path);
                        // Expand `~` to the user's home directory.
                        let expanded_path = tilde(&unescaped_path);
                        let file_path = current_dir.join(&*expanded_path);

                        match std::fs::metadata(&file_path) {
                            Ok(metadata) if metadata.is_file() => {
                                use crate::util::file::external_editor;

                                ctx.dispatch_typed_action(&TerminalAction::OpenCodeInYarp {
                                    path: file_path,
                                    layout: external_editor::settings::EditorLayout::SplitPane,
                                    line_col: parsed_path.line_and_column_num,
                                });
                            }
                            Ok(_) => {
                                show_error_toast(
                                    "The /open-file command only works for files, not directories"
                                        .to_owned(),
                                    ctx,
                                );
                                return true;
                            }
                            Err(_) => {
                                show_error_toast(
                                    format!("File not found: {}", file_path.display()),
                                    ctx,
                                );
                                return true;
                            }
                        }
                    }
                    _ => {
                        use crate::server::telemetry::PaletteSource;

                        ctx.emit(Event::OpenFilesPalette {
                            source: PaletteSource::Keybinding,
                        });
                    }
                }
                #[cfg(not(feature = "local_fs"))]
                {
                    show_error_toast(
                        "The /open-file command is not supported in this build".to_owned(),
                        ctx,
                    );
                    return true;
                }
            }
            export_to_clipboard if command.name == commands::EXPORT_TO_CLIPBOARD.name => {
                let history = BlocklistAIHistoryModel::handle(ctx);
                let Some(conversation) = history
                    .as_ref(ctx)
                    .active_conversation(self.terminal_view_id)
                else {
                    show_error_toast("No active conversation to export".to_owned(), ctx);
                    return true;
                };

                let action_model = self.ai_action_model.as_ref(ctx);
                let conversation_text = conversation.export_to_markdown(Some(action_model));

                ctx.clipboard()
                    .write(ClipboardContent::plain_text(conversation_text));

                // Show a toast to confirm the export
                let window_id = ctx.window_id();
                ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                    let toast = DismissibleToast::default(String::from(
                        "Conversation exported to clipboard",
                    ));
                    toast_stack.add_ephemeral_toast(toast, window_id, ctx);
                });
            }
            export_to_file if command.name == commands::EXPORT_TO_FILE.name => {
                #[cfg(not(target_family = "wasm"))]
                {
                    self.export_conversation_to_file(
                        argument.map(|filename| filename.to_owned()),
                        ctx,
                    );
                }
                #[cfg(target_family = "wasm")]
                {
                    show_error_toast(
                        "Export conversation to file unsupported in web".to_owned(),
                        ctx,
                    );
                    return true;
                }
            }
            index if command.name == commands::INDEX.name => {
                ctx.dispatch_typed_action(&TerminalAction::IndexProjectSpeedbump);
            }
            init if command.name == commands::INIT.name => {
                ctx.dispatch_typed_action(&TerminalAction::InitProject);
            }
            changelog if command.name == commands::CHANGELOG.name => {
                if !FeatureFlag::Changelog.is_enabled() {
                    return false;
                }
                ctx.dispatch_typed_action(&WorkspaceAction::ViewLatestChangelog);
            }
            feedback if command.name == commands::FEEDBACK.name => {
                ctx.dispatch_typed_action(&WorkspaceAction::SendFeedback);
            }
            open_code_review if command.name == commands::OPEN_CODE_REVIEW.name => {
                ctx.dispatch_typed_action(&TerminalAction::ToggleCodeReviewPane {
                    entrypoint: CodeReviewPaneEntrypoint::SlashCommand,
                });
            }
            open_mcp_servers if command.name == commands::OPEN_MCP_SERVERS.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenViewMCPPane);
            }
            open_settings_file if command.name == commands::OPEN_SETTINGS_FILE.name => {
                if !FeatureFlag::SettingsFile.is_enabled() || !cfg!(feature = "local_fs") {
                    return false;
                }
                ctx.dispatch_typed_action(&WorkspaceAction::OpenSettingsFile);
            }
            open_project_rules if command.name == commands::OPEN_PROJECT_RULES.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenProjectRulesPane);
            }
            open_rules if command.name == commands::OPEN_RULES.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenRulesPane);
            }
            edit_skill if command.name == commands::EDIT_SKILL.name => {
                if !FeatureFlag::ListSkills.is_enabled() {
                    return false;
                }
                // Open the skill selector menu - user will select a skill from the inline menu
                self.open_skill_selector(ctx);
            }
            invoke_skill if command.name == commands::INVOKE_SKILL.name => {
                if !FeatureFlag::ListSkills.is_enabled() {
                    return false;
                }
                // Open the skill selector menu for invocation - skill command will be inserted into buffer
                self.open_invoke_skill_selector(ctx);
            }
            models if command.name == commands::MODEL.name => {
                self.open_model_selector(ctx);
            }
            profiles if command.name == commands::PROFILE.name => {
                if !FeatureFlag::InlineProfileSelector.is_enabled() {
                    return false;
                }

                self.open_profile_selector(ctx);
            }
            prompts if command.name == commands::PROMPTS.name => {
                if FeatureFlag::AgentView.is_enabled() {
                    self.open_prompts_menu(ctx);
                } else {
                    return false;
                }
            }
            rewind if command.name == commands::REWIND.name => {
                self.open_rewind_menu(ctx);
            }
            pr_comments if command.name == commands::PR_COMMENTS.name => {
                if !FeatureFlag::PRCommentsSlashCommand.is_enabled() {
                    return false;
                }

                let Some(repo_path) = self
                    .active_session_path_if_local(ctx)
                    .map(|path| path.to_path_buf())
                    .map(|path| path.to_string_lossy().to_string())
                else {
                    log::error!("Expected a valid working directory since /pr-comments is only available from the terminal");
                    return false;
                };

                self.ai_controller.update(ctx, move |controller, ctx| {
                    controller.send_slash_command_request(
                        SlashCommandRequest::FetchReviewComments { repo_path },
                        ctx,
                    )
                });
            }
            usage if command.name == commands::USAGE.name => {
                ctx.dispatch_typed_action(&TerminalAction::OpenBillingAndUsagePane);
            }
            remote_control if command.name == commands::REMOTE_CONTROL.name => {
                if !FeatureFlag::CreatingSharedSessions.is_enabled()
                    || !FeatureFlag::HOARemoteControl.is_enabled()
                {
                    return false;
                }
                if self
                    .model
                    .lock()
                    .shared_session_status()
                    .is_sharer_or_viewer()
                {
                    show_error_toast("Session is already being shared".to_owned(), ctx);
                    return true;
                }
                ctx.emit(Event::StartRemoteControl);
            }
            cost if command.name == commands::COST.name => {
                let history = BlocklistAIHistoryModel::handle(ctx);
                let conversation = history
                    .as_ref(ctx)
                    .active_conversation(self.terminal_view_id);
                if conversation.is_none() {
                    show_error_toast(
                        "Cannot show conversation cost: no active conversation".to_owned(),
                        ctx,
                    );
                } else if conversation.is_some_and(|c| c.is_empty()) {
                    show_error_toast(
                        "Cannot show conversation cost: conversation is empty".to_owned(),
                        ctx,
                    );
                } else if conversation.is_some_and(|c| !c.status().is_done()) {
                    show_error_toast(
                        "Cannot show conversation cost: conversation is in progress".to_owned(),
                        ctx,
                    );
                } else {
                    ctx.dispatch_typed_action(&TerminalAction::ToggleUsageFooter);
                }
            }
            fork if command.name == commands::FORK.name => {
                let Some(conversation_id) = self
                    .ai_context_model
                    .as_ref(ctx)
                    .selected_conversation_id(ctx)
                else {
                    show_error_toast("/fork requires an active conversation".to_owned(), ctx);
                    return true;
                };

                let destination = if trigger.is_cmd_or_ctrl_enter() {
                    ForkedConversationDestination::NewTab
                } else {
                    ForkedConversationDestination::SplitPane
                };

                ctx.dispatch_typed_action(&WorkspaceAction::ForkAIConversation {
                    conversation_id,
                    fork_from_exchange: None,
                    summarize_after_fork: false,
                    summarization_prompt: None,
                    initial_prompt: argument.cloned(),
                    destination,
                });
            }
            fork_from if command.name == commands::FORK_FROM.name => {
                self.open_user_query_menu(UserQueryMenuAction::ForkFrom, ctx);
                return true;
            }
            fork_and_compact if command.name == commands::FORK_AND_COMPACT.name => {
                let Some(conversation_id) = self
                    .ai_context_model
                    .as_ref(ctx)
                    .selected_conversation_id(ctx)
                else {
                    show_error_toast(
                        "/fork-and-compact requires an active conversation".to_owned(),
                        ctx,
                    );
                    return true;
                };

                let destination = if trigger.is_cmd_or_ctrl_enter() {
                    ForkedConversationDestination::SplitPane
                } else {
                    ForkedConversationDestination::CurrentPane
                };

                ctx.dispatch_typed_action(&WorkspaceAction::ForkAIConversation {
                    conversation_id,
                    fork_from_exchange: None,
                    summarize_after_fork: true,
                    summarization_prompt: None,
                    initial_prompt: argument.cloned(),
                    destination,
                });
            }
            compact_and if command.name == commands::COMPACT_AND.name => {
                if self
                    .ai_context_model
                    .as_ref(ctx)
                    .selected_conversation_id(ctx)
                    .is_none()
                {
                    show_error_toast(
                        "/compact-and requires an active conversation".to_owned(),
                        ctx,
                    );
                    return true;
                };

                ctx.dispatch_typed_action(&WorkspaceAction::SummarizeAIConversation {
                    prompt: None,
                    initial_prompt: argument.cloned(),
                });
            }
            queue if command.name == commands::QUEUE.name => {
                let Some(conversation_id) = self
                    .ai_context_model
                    .as_ref(ctx)
                    .selected_conversation_id(ctx)
                else {
                    show_error_toast("/queue requires an active conversation".to_owned(), ctx);
                    return true;
                };

                let Some(prompt) = argument.filter(|a| !a.is_empty()).cloned() else {
                    show_error_toast("/queue requires a prompt argument".to_owned(), ctx);
                    return true;
                };

                let history = BlocklistAIHistoryModel::handle(ctx);
                let is_in_progress = history
                    .as_ref(ctx)
                    .conversation(&conversation_id)
                    .is_some_and(|c| c.status().is_in_progress() || c.status().is_blocked());

                if is_in_progress {
                    ctx.dispatch_typed_action(&WorkspaceAction::QueuePromptForConversation {
                        prompt,
                    });
                } else {
                    self.submit_queued_prompt(prompt, ctx);
                }
            }
            open_repo if command.name == commands::OPEN_REPO.name => {
                if !FeatureFlag::InlineRepoMenu.is_enabled() {
                    return false;
                }
                self.open_repos_menu(ctx);
            }
            command_that_just_sends_ai_request_with_prefix
                if command.name == commands::COMPACT.name
                    || command.name == commands::PLAN.name
                    || command.name == commands::ORCHESTRATE.name =>
            {
                // These slash commands just send AI requests with the slash command text as a
                // prefix, and special handling is done downstream as an implementation detail
                // of handling user queries with specific slash command prefixes.
                return false;
            }
            _ => {
                debug_assert!(
                    false,
                    "Attempted to execute slash command with no handler: {}",
                    command.name
                );
                return false;
            }
        }

        // Leave the buffer alone when re-sending a queued prompt (the user may have typed
        // new input while the agent was busy).
        if !is_queued_prompt {
            self.editor.update(ctx, |editor, ctx| {
                editor.clear_buffer(ctx);
            });
        }

        // If the command must be executed in AI mode, and we're not already in an agent view,
        // enter the agent view.
        if FeatureFlag::AgentView.is_enabled()
            && command.auto_enter_ai_mode
            && !self.agent_view_controller.as_ref(ctx).is_active()
        {
            self.agent_view_controller.update(ctx, |controller, ctx| {
                let _ = controller.try_enter_agent_view(
                    None,
                    AgentViewEntryOrigin::SlashCommand {
                        trigger: SlashCommandTrigger::input(),
                    },
                    ctx,
                );
            });
        }

        let is_in_agent_view = FeatureFlag::AgentView.is_enabled()
            && self.agent_view_controller.as_ref(ctx).is_active();
        send_telemetry_from_ctx!(
            TelemetryEvent::SlashCommandAccepted {
                command_details: SlashCommandAcceptedDetails::StaticCommand {
                    command_name: command.name.to_owned(),
                },
                is_in_agent_view,
            },
            ctx
        );
        true
    }

    /// Handles cmd+enter (Mac) / ctrl+enter (Linux/Windows) for slash commands.
    ///
    /// Returns `true` if the keypress was handled.
    pub(super) fn maybe_handle_cmd_or_ctrl_shift_enter_for_slash_command(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        // If slash command menu is open, accept the selected item with cmd_or_ctrl_enter=true.
        if matches!(
            self.suggestions_mode_model.as_ref(ctx).mode(),
            InputSuggestionsMode::SlashCommands
        ) {
            self.inline_slash_commands_view.update(ctx, |view, ctx| {
                view.accept_selected_item(true, ctx);
            });
            return true;
        }

        // If no menu but slash command detected in buffer, execute with cmd_or_ctrl_enter=true
        match self.slash_command_model.as_ref(ctx).state() {
            SlashCommandEntryState::SlashCommand(detected_command) => {
                let command = detected_command.command.clone();
                let argument = detected_command.argument.clone();
                self.execute_slash_command(
                    &command,
                    argument.as_ref(),
                    SlashCommandTrigger::cmd_or_ctrl_enter(),
                    /*is_queued_prompt*/ false,
                    ctx,
                )
            }
            SlashCommandEntryState::SkillCommand(detected_skill) => {
                let reference = detected_skill.reference.clone();
                let user_query = detected_skill.argument.clone();
                self.execute_skill_command(
                    reference, user_query, /*is_queued_prompt*/ false, ctx,
                )
            }
            SlashCommandEntryState::None
            | SlashCommandEntryState::Composing { .. }
            | SlashCommandEntryState::DisabledUntilEmptyBuffer => false,
        }
    }

    /// Executes a slash command on `enter` keypress.
    ///
    /// If the slash command menu is open, then "accepts" the slash command:
    ///   * If the slash command does not take arguments, executes it
    ///   * If the slash command does take arguments, inserts it into the input.
    ///
    /// If the slash command menu is not open, then "executes" the slash command in the input, if
    /// there is one.
    ///
    /// Returns `true` if the enter keypress was 'handled', else upstream enter keypress handling
    /// logic should continue.
    pub(super) fn maybe_handle_enter_for_slash_command(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        if matches!(
            self.suggestions_mode_model.as_ref(ctx).mode(),
            InputSuggestionsMode::SlashCommands
        ) {
            self.inline_slash_commands_view.update(ctx, |view, ctx| {
                view.accept_selected_item(false, ctx);
            });
            return true;
        }

        match self.slash_command_model.as_ref(ctx).state() {
            SlashCommandEntryState::SlashCommand(detected_command) => {
                let command = detected_command.command.clone();
                let argument = detected_command.argument.clone();
                self.execute_slash_command(
                    &command,
                    argument.as_ref(),
                    SlashCommandTrigger::input(),
                    /*is_queued_prompt*/ false,
                    ctx,
                )
            }
            SlashCommandEntryState::SkillCommand(detected_skill) => {
                let reference = detected_skill.reference.clone();
                let user_query = detected_skill.argument.clone();
                self.execute_skill_command(
                    reference, user_query, /*is_queued_prompt*/ false, ctx,
                )
            }
            SlashCommandEntryState::None
            | SlashCommandEntryState::Composing { .. }
            | SlashCommandEntryState::DisabledUntilEmptyBuffer => false,
        }
    }
}

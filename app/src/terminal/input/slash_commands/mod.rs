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
                let (target_raw, message) = match raw.strip_prefix('@') {
                    Some(rest) => match rest.split_once(char::is_whitespace) {
                        Some((t, body)) => (t.trim(), body.trim()),
                        None => (rest.trim(), ""),
                    },
                    None => ("", raw),
                };
                let target_lc = target_raw.to_ascii_lowercase();
                let target = target_lc.as_str();
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
                     if [ -n \"${{YARP_CALLSIGN:-}}\" ]; then \
                       sender=\"@${{YARP_CALLSIGN}}\"; \
                     else \
                       sender=\"${{USER:-unknown}}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                     fi; \
                     target={target}; \
                     {{ \
                       printf 'from: %s\\n' \"$sender\"; \
                       [ -n \"$target\" ] && printf 'to: %s\\n' \"$target\"; \
                       printf '\\n'; \
                       printf '%s' {body}; \
                     }} > \"$file\"; \
                     sender_lc=$(printf '%s' \"$sender\" | tr '[:upper:]' '[:lower:]' | sed 's/^@//' | sed 's/@.*$//'); \
                     av=''; \
                     case \"$sender_lc\" in \
                       nicholas|angel) av='🎯' ;; \
                       frank|butterman.snr) av='🦔' ;; \
                       danny|butterman) av='🍦' ;; \
                       andy|wainwright|cartwright) av='🤡' ;; \
                       doris|thatcher) av='🚓' ;; \
                       tony) av='📻' ;; \
                     esac; \
                     if [ -n \"$av\" ]; then sender_disp=\"$av $sender\"; else sender_disp=\"$sender\"; fi; \
                     target_lc=$(printf '%s' \"$target\" | tr '[:upper:]' '[:lower:]'); \
                     tav=''; \
                     case \"$target_lc\" in \
                       nicholas|angel) tav='🎯' ;; \
                       frank|butterman.snr) tav='🦔' ;; \
                       danny|butterman) tav='🍦' ;; \
                       andy|wainwright|cartwright) tav='🤡' ;; \
                       doris|thatcher) tav='🚓' ;; \
                       tony) tav='📻' ;; \
                     esac; \
                     if [ -n \"$tav\" ]; then target_disp=\"$tav @$target\"; else target_disp=\"@$target\"; fi; \
                     if [ -n \"$target\" ]; then \
                       printf '\\033[1;38;5;220m📻 DISPATCH\\033[0m \\033[3;38;5;244m%s → %s\\033[0m\\n  \\033[38;5;178m\"%s\"\\033[0m\\n' \"$sender_disp\" \"$target_disp\" {body}; \
                     else \
                       printf '\\033[1;38;5;220m📻 RADIO\\033[0m \\033[3;38;5;244m%s → all units\\033[0m\\n  \\033[38;5;178m\"%s\"\\033[0m\\n' \"$sender_disp\" {body}; \
                     fi; \
                     cs_lc=$(printf '%s' \"${{YARP_CALLSIGN:-}}\" | tr '[:upper:]' '[:lower:]'); \
                     case \"$cs_lc\" in \
                       nicholas|angel) signoff='Have a nice evening.' ;; \
                       danny|butterman) signoff='Yarp.' ;; \
                       doris|thatcher) signoff='Out.' ;; \
                       frank|butterman.snr) signoff='The greater good.' ;; \
                       andy|wainwright|cartwright) signoff='Crusty Jugglers.' ;; \
                       tony) signoff='Yarp.' ;; \
                       *) signoff='10-4.' ;; \
                     esac; \
                     printf '  \\033[3;38;5;240m“%s”\\033[0m\\n' \"$signoff\"",
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
                    "shopt -s nullglob; \
                     yarp_msgs=(/tmp/yarp-radio/*.msg); \
                     printf '\\033[1;38;5;220m👮 ROSTER\\033[0m \\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244m— %d \
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
                    let name_lc = p.name.to_ascii_lowercase();
                    let line = format!(
                        "mail=0; \
                         if [ ${{#yarp_msgs[@]}} -gt 0 ]; then \
                           for f in \"${{yarp_msgs[@]}}\"; do \
                             t=$(awk '/^to: /{{sub(/^to: /,\"\"); print; exit}}' \"$f\" 2>/dev/null); \
                             [ -z \"$t\" ] && continue; \
                             t_lc=$(printf '%s' \"$t\" | tr '[:upper:]' '[:lower:]'); \
                             [ \"$t_lc\" = {name_lc} ] && mail=$((mail+1)); \
                           done; \
                         fi; \
                         mail_tag=''; \
                         [ \"$mail\" -gt 0 ] && mail_tag=$(printf ' \\033[1;38;5;35m📬 %d\\033[0m' \"$mail\"); \
                         you=''; if [ \"${{YARP_CALLSIGN:-}}\" = {name_lc} ]; then you=' \\033[1;38;5;35m⭐ ← you\\033[0m'; fi; \
                         printf '  \\033[{badge_color}m%-4s\\033[0m \\033[{name_color}m%-18s\\033[0m \
                         \\033[3;38;5;244m%s\\033[0m{lead_tag}%s%s\\n' {badge} {name} {role} \"$mail_tag\" \"$you\"; ",
                        badge_color = badge_color,
                        name_color = name_color,
                        lead_tag = lead_tag,
                        name_lc = crate::personas::shell_quote_one(&name_lc),
                        badge = crate::personas::shell_quote_one(&p.badge),
                        name = crate::personas::shell_quote_one(&p.name),
                        role = crate::personas::shell_quote_one(role),
                    );
                    script.push_str(&line);
                    if let Some(bin) = p.binary.as_deref() {
                        let bin_line = format!(
                            "printf '       \\033[2;38;5;244m↳ 🤖 %s\\033[0m\\n' {bin}; ",
                            bin = crate::personas::shell_quote_one(bin),
                        );
                        script.push_str(&bin_line);
                    }
                }
                script.push_str(
                    "printf '  \\033[3;38;5;244m/duty <name> to claim · /radio <name>: <msg> to send\\033[0m\\n'; ",
                );
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
                const CASE_QUOTES: &[(&str, &str)] = &[
                    ("angel", "Murder, murder, murder. — Angel"),
                    ("angel", "I dare say there's a perfectly innocent explanation. — Angel"),
                    ("frank", "It's all there in black and white. — Frank"),
                    ("frank", "Forget it, Nicholas, it's Sandford. — Frank"),
                    ("danny", "By the power of Greyskull. — Danny"),
                    ("danny", "Skip to the end. — Danny"),
                    ("andy", "Crusty Jugglers. — Andy"),
                    ("andy", "He's not Judge Judy and Executioner. — Andy"),
                    ("doris", "Crispy Christ. — Doris"),
                    ("doris", "She's about to receive a great whopping kiss. — Doris"),
                    ("tony", "Yarp. — Tony"),
                    ("tony", "Narp. — Tony"),
                ];
                let nanos = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0)
                };
                let pick_for = |key: &str| -> &'static str {
                    let pool: Vec<&'static str> = CASE_QUOTES.iter()
                        .filter(|(k, _)| *k == key)
                        .map(|(_, q)| *q)
                        .collect();
                    if pool.is_empty() {
                        CASE_QUOTES[nanos % CASE_QUOTES.len()].1
                    } else {
                        pool[nanos % pool.len()]
                    }
                };
                let avatar_for = |key: &str| -> &'static str {
                    match key {
                        "angel" => "🎯",
                        "frank" => "🦔",
                        "danny" => "🍦",
                        "andy" => "🤡",
                        "doris" => "🚓",
                        "tony" => "📻",
                        _ => "",
                    }
                };
                let (q_random_key, q_random) = CASE_QUOTES[nanos % CASE_QUOTES.len()];
                let q_random_av = avatar_for(q_random_key);
                let q_angel = pick_for("angel");
                let q_frank = pick_for("frank");
                let q_danny = pick_for("danny");
                let q_andy = pick_for("andy");
                let q_doris = pick_for("doris");
                let q_tony = pick_for("tony");
                let cmd = format!(
                    "mkdir -p /tmp/yarp-radio; \
                     ts=$(date +%s%N 2>/dev/null | cut -c1-13); \
                     [ -z \"$ts\" ] && ts=$(date +%s)000; \
                     file=/tmp/yarp-radio/${{ts}}-$$-${{RANDOM}}${{RANDOM}}.msg; \
                     if [ -n \"${{YARP_CALLSIGN:-}}\" ]; then \
                       sender=\"@${{YARP_CALLSIGN}}\"; \
                     else \
                       sender=\"${{USER:-unknown}}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                     fi; \
                     {{ \
                       printf 'from: %s\\n' \"$sender\"; \
                       printf '\\n'; \
                       printf 'case opened: %s' {name}; \
                     }} > \"$file\"; \
                     n=$(ls /tmp/yarp-radio/*.msg 2>/dev/null | wc -l | tr -d ' '); \
                     [ -z \"$n\" ] && n=0; \
                     cs_lc=$(printf '%s' \"${{YARP_CALLSIGN:-}}\" | tr '[:upper:]' '[:lower:]'); \
                     case \"$cs_lc\" in \
                       nicholas|angel) quote={q_angel}; cs_avatar='🎯'; quote_av='🎯' ;; \
                       frank|butterman.snr) quote={q_frank}; cs_avatar='🦔'; quote_av='🦔' ;; \
                       danny|butterman) quote={q_danny}; cs_avatar='🍦'; quote_av='🍦' ;; \
                       andy|wainwright|cartwright) quote={q_andy}; cs_avatar='🤡'; quote_av='🤡' ;; \
                       doris|thatcher) quote={q_doris}; cs_avatar='🚓'; quote_av='🚓' ;; \
                       tony) quote={q_tony}; cs_avatar='📻'; quote_av='📻' ;; \
                       *) quote={q_random}; cs_avatar=''; quote_av={q_random_av} ;; \
                     esac; \
                     if [ -n \"${{YARP_CALLSIGN:-}}\" ] && [ -n \"$cs_avatar\" ]; then filed_by=\"✍️  filed by $cs_avatar @${{YARP_CALLSIGN}} — \"; \
                     elif [ -n \"${{YARP_CALLSIGN:-}}\" ]; then filed_by=\"✍️  filed by @${{YARP_CALLSIGN}} — \"; \
                     else filed_by=''; fi; \
                     if [ -n \"$quote_av\" ]; then quote_prefix=\"$quote_av  \"; else quote_prefix=''; fi; \
                     printf '\\033[1;38;5;220m📁 CASE OPENED\\033[0m \\033[1;38;5;179m%s\\033[0m\\n  📂 \\033[38;5;178mtab → \"%s\"\\033[0m\\n  📣 \\033[3;38;5;244m%sall units notified\\033[0m\\n  📥 \\033[1;38;5;220m%s\\033[0m \\033[3;38;5;244min queue\\033[0m\\n  %s\\033[3;38;5;240m“%s”\\033[0m\\n' {name} {tab} \"$filed_by\" \"$n\" \"$quote_prefix\" \"$quote\"",
                    name = crate::personas::shell_quote_one(name),
                    tab = crate::personas::shell_quote_one(&tab_name),
                    q_random = crate::personas::shell_quote_one(q_random),
                    q_random_av = crate::personas::shell_quote_one(q_random_av),
                    q_angel = crate::personas::shell_quote_one(q_angel),
                    q_frank = crate::personas::shell_quote_one(q_frank),
                    q_danny = crate::personas::shell_quote_one(q_danny),
                    q_andy = crate::personas::shell_quote_one(q_andy),
                    q_doris = crate::personas::shell_quote_one(q_doris),
                    q_tony = crate::personas::shell_quote_one(q_tony),
                );
                self.try_execute_command(&cmd, ctx);
            }
            _duty if command.name == commands::DUTY.name => {
                let callsign = argument
                    .map(|a| a.trim())
                    .filter(|a| !a.is_empty());
                let cmd = if let Some(cs) = callsign {
                    if cs.eq_ignore_ascii_case("off") || cs.eq_ignore_ascii_case("clear") {
                        "prev_call=\"${YARP_CALLSIGN:-}\"; \
                         unset YARP_CALLSIGN; \
                         duty_marker=\"/tmp/yarp-radio/.duty-${USER:-unknown}\"; \
                         shift_str=''; \
                         ended_at=$(date '+%H:%M'); \
                         if [ -f \"$duty_marker\" ]; then \
                           start=$(cat \"$duty_marker\" 2>/dev/null); \
                           if [ -n \"$start\" ] && [ \"$start\" -gt 0 ] 2>/dev/null; then \
                             now=$(date +%s); \
                             elapsed=$((now - start)); \
                             h=$((elapsed / 3600)); m=$(((elapsed % 3600) / 60)); s=$((elapsed % 60)); \
                             if [ $h -gt 0 ]; then shift_str=$(printf '%dh %02dm' $h $m); \
                             elif [ $m -gt 0 ]; then shift_str=$(printf '%dm %02ds' $m $s); \
                             else shift_str=$(printf '%ds' $s); fi; \
                           fi; \
                           rm -f \"$duty_marker\"; \
                         fi; \
                         printf '\\033[1;38;5;220m🎖  OFF DUTY\\033[0m\\n'; \
                         prev_lc=$(printf '%s' \"$prev_call\" | tr '[:upper:]' '[:lower:]'); \
                         prev_av=''; \
                         case \"$prev_lc\" in \
                           nicholas|angel) prev_av='🎯' ;; \
                           frank|butterman.snr) prev_av='🦔' ;; \
                           danny|butterman) prev_av='🍦' ;; \
                           andy|wainwright|cartwright) prev_av='🤡' ;; \
                           doris|thatcher) prev_av='🚓' ;; \
                           tony) prev_av='📻' ;; \
                         esac; \
                         if [ -n \"$prev_call\" ]; then \
                           if [ -n \"$prev_av\" ]; then prev_marker=\"$prev_av \"; else prev_marker=\"📛 \"; fi; \
                           printf '  \\033[2;38;5;244m%-10s\\033[0m %s\\033[1;38;5;220m@%s\\033[0m \\033[3;38;5;244mcleared\\033[0m\\n' 'callsign' \"$prev_marker\" \"$prev_call\"; \
                         else \
                           printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[3;38;5;244mnone active\\033[0m\\n' 'callsign'; \
                         fi; \
                         if [ -n \"$shift_str\" ]; then \
                           printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[1;38;5;220m%s\\033[0m \\033[3;38;5;244mon the beat\\033[0m\\n' 'shift' \"$shift_str\"; \
                         fi; \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m ⏱  \\033[1;38;5;220m%s\\033[0m\\n' 'ended' \"$ended_at\"".to_owned()
                    } else {
                        let cs_lower = cs.to_ascii_lowercase();
                        let cs = cs_lower.as_str();
                        let roster = crate::personas::Roster::load()
                            .unwrap_or_else(crate::personas::Roster::default_sandford);
                        let persona = roster
                            .default_team()
                            .and_then(|t| t.members.iter()
                                .find(|p| p.name.eq_ignore_ascii_case(cs)));
                        let persona_lines = if let Some(p) = persona {
                            let role = p.role.split('.').next().unwrap_or(&p.role).trim();
                            let lead_tag = if p.lead {
                                " \\033[3;38;5;220m⭐ lead\\033[0m"
                            } else {
                                ""
                            };
                            format!(
                                "; printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;220m%s\\033[0m \\033[1;38;5;178m%s\\033[0m{lead_tag}\\n  \\033[2;38;5;244m%-10s\\033[0m \\033[3;38;5;244m%s\\033[0m\\n' 'persona' {badge} {name} 'role' {role}",
                                badge = crate::personas::shell_quote_one(&p.badge),
                                name = crate::personas::shell_quote_one(&p.name),
                                role = crate::personas::shell_quote_one(role),
                                lead_tag = lead_tag,
                            )
                        } else {
                            String::new()
                        };
                        let greeting = crate::personas::shell_quote_one(
                            crate::personas::persona_on_duty_quote(cs),
                        );
                        let badge_prefix = crate::personas::shell_quote_one(
                            &persona
                                .map(|p| format!("{} ", p.badge))
                                .unwrap_or_else(|| "📛 ".to_string()),
                        );
                        format!(
                            "export YARP_CALLSIGN={cs}; \
                             mkdir -p /tmp/yarp-radio; \
                             date +%s > \"/tmp/yarp-radio/.duty-${{USER:-unknown}}\" 2>/dev/null; \
                             started_at=$(date '+%H:%M'); \
                             printf '\\033[1;38;5;220m🎖  ON DUTY\\033[0m \\033[1;38;5;179m@%s\\033[0m \\033[3;38;5;244mcallsign claimed\\033[0m\\n  \\033[2;38;5;244m%-10s\\033[0m %s\\033[1;38;5;220m@%s\\033[0m\\n' {cs} 'unit' {badge_prefix} {cs}{persona_lines}; \
                             printf '  \\033[2;38;5;244m%-10s\\033[0m ⏱  \\033[1;38;5;220m%s\\033[0m\\n' 'started' \"$started_at\"; \
                             printf '\\n  \\033[3;38;5;240m“%s”\\033[0m\\n' {greeting}; \
                             printf '  \\033[3;38;5;244m/inbox will flag traffic addressed to @%s as DIRECT\\033[0m\\n' {cs}",
                            cs = crate::personas::shell_quote_one(cs),
                            persona_lines = persona_lines,
                            greeting = greeting,
                            badge_prefix = badge_prefix,
                        )
                    }
                } else {
                    "if [ -n \"$YARP_CALLSIGN\" ]; then \
                       cs_lc=$(printf '%s' \"$YARP_CALLSIGN\" | tr '[:upper:]' '[:lower:]'); \
                       av=''; \
                       case \"$cs_lc\" in \
                         nicholas|angel) av='🎯' ;; \
                         frank|butterman.snr) av='🦔' ;; \
                         danny|butterman) av='🍦' ;; \
                         andy|wainwright|cartwright) av='🤡' ;; \
                         doris|thatcher) av='🚓' ;; \
                         tony) av='📻' ;; \
                       esac; \
                       if [ -n \"$av\" ]; then unit_disp=\"$av @$YARP_CALLSIGN\"; else unit_disp=\"📛 @$YARP_CALLSIGN\"; fi; \
                       since_row=''; \
                       shopt -s nullglob; \
                       qfiles=(/tmp/yarp-radio/*.msg); \
                       qn=${#qfiles[@]}; \
                       duty_marker=\"/tmp/yarp-radio/.duty-${USER:-unknown}\"; \
                       tier_icon='⏱'; \
                       if [ -f \"$duty_marker\" ]; then \
                         start=$(cat \"$duty_marker\" 2>/dev/null); \
                         if [ -n \"$start\" ] && [ \"$start\" -gt 0 ] 2>/dev/null; then \
                           now=$(date +%s); \
                           elapsed=$((now - start)); \
                           hh=$((elapsed / 3600)); mm=$(((elapsed % 3600) / 60)); \
                           when=$(date -r \"$start\" '+%H:%M' 2>/dev/null || echo '—'); \
                           if [ $hh -ge 8 ]; then tier_icon='🌙'; \
                           elif [ $hh -ge 4 ]; then tier_icon='🔴'; \
                           elif [ $hh -ge 1 ]; then tier_icon='🟡'; \
                           else tier_icon='🟢'; \
                           fi; \
                           if [ $hh -gt 0 ]; then \
                             since_row=$(printf '%s \\033[2;38;5;240m(%dh %02dm)\\033[0m' \"$when\" $hh $mm); \
                           elif [ $mm -gt 0 ]; then \
                             since_row=$(printf '%s \\033[2;38;5;240m(%dm)\\033[0m' \"$when\" $mm); \
                           else \
                             since_row=$(printf '%s \\033[2;38;5;240m(just now)\\033[0m' \"$when\"); \
                           fi; \
                         fi; \
                       fi; \
                       printf '\\033[1;38;5;220m🎖  ON DUTY\\033[0m \\033[3;38;5;244mcurrent callsign\\033[0m\\n  \\033[2;38;5;244m%-10s\\033[0m \\033[1;38;5;220m%s\\033[0m\\n' 'unit' \"$unit_disp\"; \
                       if [ -n \"$since_row\" ]; then \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m %s  \\033[1;38;5;220m%b\\033[0m\\n' 'since' \"$tier_icon\" \"$since_row\"; \
                       fi; \
                       if [ \"$qn\" -gt 0 ]; then \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📥 \\033[1;38;5;220m%d\\033[0m \\033[3;38;5;244mpending — /inbox to read\\033[0m\\n' 'queue' \"$qn\"; \
                       else \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📥 \\033[3;38;5;244minbox clear\\033[0m\\n' 'queue'; \
                       fi; \
                       printf '  \\033[3;38;5;244m/duty <name> to change · /duty off to clear\\033[0m\\n'; \
                     else \
                       shopt -s nullglob; \
                       qfiles=(/tmp/yarp-radio/*.msg); \
                       qn=${#qfiles[@]}; \
                       printf '\\033[3;38;5;179m🎖  no callsign claimed\\033[0m\\n'; \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[38;5;240m🎯 angel · 🦔 frank · 🍦 danny · 🤡 andy · 🚓 doris · 📻 tony\\033[0m\\n' 'examples'; \
                       if [ \"$qn\" -gt 0 ]; then \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📥 \\033[1;38;5;220m%d\\033[0m \\033[3;38;5;244mpending — claim a callsign first to read directs\\033[0m\\n' 'queue' \"$qn\"; \
                       fi; \
                       printf '  \\033[3;38;5;244m/duty <name> to claim one — direct radio routes by callsign\\033[0m\\n'; \
                     fi"
                        .to_owned()
                };
                self.try_execute_command(&cmd, ctx);
            }
            _clock_out if command.name == commands::CLOCK_OUT.name => {
                const SIGN_OFFS: &[(&str, &str)] = &[
                    ("angel", "Have a nice evening. — Angel"),
                    ("angel", "I'll be off home, then. — Angel"),
                    ("danny", "Pub? — Danny"),
                    ("danny", "By the power of Greyskull. — Danny"),
                    ("frank", "Forget it, Nicholas, it's Sandford. — Frank"),
                    ("frank", "Right then, paperwork. — Frank"),
                    ("andy", "Crusty Jugglers. — Andy"),
                    ("andy", "Cool. — Andy"),
                    ("doris", "Off home, then. — Doris"),
                    ("doris", "Mind how you go. — Doris"),
                    ("tony", "Yarp. — Tony"),
                    ("tony", "Narp. — Tony"),
                    ("any", "The greater good. — The NWA"),
                    ("any", "Punish the bad. Promote the good. — The NWA"),
                ];
                let nanos = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0)
                };
                let pick_for = |key: &str| -> &'static str {
                    let pool: Vec<&'static str> = SIGN_OFFS.iter()
                        .filter(|(k, _)| *k == key)
                        .map(|(_, q)| *q)
                        .collect();
                    if pool.is_empty() {
                        SIGN_OFFS[nanos % SIGN_OFFS.len()].1
                    } else {
                        pool[nanos % pool.len()]
                    }
                };
                let q_random = SIGN_OFFS[nanos % SIGN_OFFS.len()].1;
                let q_angel = pick_for("angel");
                let q_frank = pick_for("frank");
                let q_danny = pick_for("danny");
                let q_andy = pick_for("andy");
                let q_doris = pick_for("doris");
                let q_tony = pick_for("tony");
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
                     duty_marker=\"/tmp/yarp-radio/.duty-${{user}}\"; \
                     shift_str=\"\"; \
                     if [ -f \"$duty_marker\" ]; then \
                       start=$(cat \"$duty_marker\" 2>/dev/null); \
                       now=$(date +%s); \
                       if [ -n \"$start\" ] && [ \"$start\" -gt 0 ] 2>/dev/null; then \
                         elapsed=$((now - start)); \
                         h=$((elapsed / 3600)); m=$(((elapsed % 3600) / 60)); s=$((elapsed % 60)); \
                         if [ $h -gt 0 ]; then shift_str=$(printf '%dh %02dm' $h $m); \
                         elif [ $m -gt 0 ]; then shift_str=$(printf '%dm %02ds' $m $s); \
                         else shift_str=$(printf '%ds' $s); fi; \
                       fi; \
                       rm -f \"$duty_marker\"; \
                     fi; \
                     printf '\\033[1;38;5;220m🌙 OFF DUTY\\033[0m \\033[1;38;5;179m%s\\033[0m\\n' \"$when\"; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m 👮 \\033[1;38;5;220m%s\\033[0m\\033[2;38;5;244m@\\033[0m\\033[1;38;5;220m%s\\033[0m\\n' 'officer' \"$user\" \"$host\"; \
                     prev_lc=$(printf '%s' \"$prev_call\" | tr '[:upper:]' '[:lower:]'); \
                     prev_av=''; \
                     case \"$prev_lc\" in \
                       nicholas|angel) prev_av='🎯' ;; \
                       frank|butterman.snr) prev_av='🦔' ;; \
                       danny|butterman) prev_av='🍦' ;; \
                       andy|wainwright|cartwright) prev_av='🤡' ;; \
                       doris|thatcher) prev_av='🚓' ;; \
                       tony) prev_av='📻' ;; \
                     esac; \
                     if [ -n \"$prev_call\" ]; then \
                       if [ -n \"$prev_av\" ]; then prev_marker=\"$prev_av \"; else prev_marker=\"📛 \"; fi; \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m %s\\033[1;38;5;220m@%s\\033[0m \\033[3;38;5;244mcleared\\033[0m\\n' 'callsign' \"$prev_marker\" \"$prev_call\"; \
                     fi; \
                     if [ -n \"$shift_str\" ]; then \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m ⏱  \\033[1;38;5;220m%s\\033[0m \\033[3;38;5;244mon the beat\\033[0m\\n' 'shift' \"$shift_str\"; \
                     fi; \
                     if [ \"$n\" -gt 0 ]; then \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m 📻 \\033[1;38;5;220m%d transmission(s)\\033[0m \\033[3;38;5;244mcleared\\033[0m\\n' 'radio' \"$n\"; \
                     else \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m 📻 \\033[3;38;5;244mqueue already empty\\033[0m\\n' 'radio'; \
                     fi; \
                     case \"$prev_lc\" in \
                       nicholas|angel) signoff={q_angel} ;; \
                       frank|butterman.snr) signoff={q_frank} ;; \
                       danny|butterman) signoff={q_danny} ;; \
                       andy|wainwright|cartwright) signoff={q_andy} ;; \
                       doris|thatcher) signoff={q_doris} ;; \
                       tony) signoff={q_tony} ;; \
                       *) signoff={q_random} ;; \
                     esac; \
                     signoff_av=\"$prev_av\"; \
                     if [ -n \"${{shift_str:-}}\" ]; then \
                       if [ \"${{h:-0}}\" -ge 8 ] 2>/dev/null; then \
                         signoff='Get yourself home. Long night. — Frank'; signoff_av='🦔'; \
                       elif [ \"${{h:-0}}\" -ge 4 ] 2>/dev/null; then \
                         signoff='Skip to the end. — Danny'; signoff_av='🍦'; \
                       elif [ \"${{h:-0}}\" -eq 0 ] && [ \"${{m:-0}}\" -lt 5 ] 2>/dev/null; then \
                         signoff='That was quick. — Andy'; signoff_av='🤡'; \
                       fi; \
                     fi; \
                     if [ -n \"$signoff_av\" ]; then \
                       printf '\\n  %s \\033[3;38;5;244m“%s”\\033[0m\\n' \"$signoff_av\" \"$signoff\"; \
                     else \
                       printf '\\n  \\033[3;38;5;244m“%s”\\033[0m\\n' \"$signoff\"; \
                     fi",
                    q_random = crate::personas::shell_quote_one(q_random),
                    q_angel = crate::personas::shell_quote_one(q_angel),
                    q_frank = crate::personas::shell_quote_one(q_frank),
                    q_danny = crate::personas::shell_quote_one(q_danny),
                    q_andy = crate::personas::shell_quote_one(q_andy),
                    q_doris = crate::personas::shell_quote_one(q_doris),
                    q_tony = crate::personas::shell_quote_one(q_tony),
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
                    "He's not Judge Judy and Executioner. — Andy",
                    "Murder. Mur-der. — Andy",
                    "It's not Sunday, the gun shop's shut. — Angel",
                    "It's all about the paperwork. — Angel",
                    "The greater good. — The NWA",
                    "All for the good of Sandford. — The NWA",
                    "Yarp. — Tony",
                    "Narp. — Tony",
                    "Yeah, but he gets to ride the horse. — Doris",
                    "Oh, give it some welly. — Doris",
                    "Forget it, Nicholas, it's Sandford. — Frank",
                    "Welcome to Sandford. — Frank",
                    "Pub? — Danny",
                    "Have you ever fired your gun in the air and yelled aaaaargh? — Danny",
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
                     callsign=\"${{YARP_CALLSIGN:-}}\"; \
                     shopt -s nullglob; \
                     queue=(/tmp/yarp-radio/*.msg); \
                     printf '\\033[1;38;5;220m🚓 SITREP\\033[0m \\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244mstation\\033[0m\\n\\n' {team}; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m 👮 \\033[1;38;5;220m%s\\033[0m\\033[2;38;5;244m@\\033[0m\\033[1;38;5;220m%s\\033[0m\\n' 'officer' \"$user\" \"$host\"; \
                     if [ -n \"$callsign\" ]; then \
                       cs_lc=$(printf '%s' \"$callsign\" | tr '[:upper:]' '[:lower:]'); \
                       sit_av=''; \
                       case \"$cs_lc\" in \
                         nicholas|angel) sit_av='🎯 ' ;; \
                         frank|butterman.snr) sit_av='🦔 ' ;; \
                         danny|butterman) sit_av='🍦 ' ;; \
                         andy|wainwright|cartwright) sit_av='🤡 ' ;; \
                         doris|thatcher) sit_av='🚓 ' ;; \
                         tony) sit_av='📻 ' ;; \
                       esac; \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m %s\\033[1;38;5;220m@%s\\033[0m \\033[3;38;5;244mon duty\\033[0m\\n' 'callsign' \"$sit_av\" \"$callsign\"; \
                     else \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m 📛 \\033[3;38;5;244munclaimed — /duty <name> to claim\\033[0m\\n' 'callsign'; \
                     fi; \
                     duty_marker=\"/tmp/yarp-radio/.duty-${{user}}\"; \
                     if [ -f \"$duty_marker\" ]; then \
                       start=$(cat \"$duty_marker\" 2>/dev/null); \
                       if [ -n \"$start\" ] && [ \"$start\" -gt 0 ] 2>/dev/null; then \
                         now=$(date +%s); \
                         elapsed=$((now - start)); \
                         hh=$((elapsed / 3600)); mm=$(((elapsed % 3600) / 60)); \
                         when=$(date -r \"$start\" '+%H:%M' 2>/dev/null || echo '—'); \
                         if [ $hh -ge 8 ]; then tier_icon='🌙'; \
                         elif [ $hh -ge 4 ]; then tier_icon='🔴'; \
                         elif [ $hh -ge 1 ]; then tier_icon='🟡'; \
                         else tier_icon='🟢'; \
                         fi; \
                         if [ $hh -gt 0 ]; then \
                           since_str=$(printf '%s \\033[2;38;5;240m(%dh %02dm)\\033[0m' \"$when\" $hh $mm); \
                         elif [ $mm -gt 0 ]; then \
                           since_str=$(printf '%s \\033[2;38;5;240m(%dm)\\033[0m' \"$when\" $mm); \
                         else \
                           since_str=$(printf '%s \\033[2;38;5;240m(just now)\\033[0m' \"$when\"); \
                         fi; \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m %s  \\033[1;38;5;220m%b\\033[0m\\n' 'since' \"$tier_icon\" \"$since_str\"; \
                       fi; \
                     fi; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m 👣 \\033[1;38;5;179m%s\\033[0m\\n' 'beat' \"$cwd\"; \
                     printf '  \\033[2;38;5;244m%-10s\\033[0m 🛂 \\033[1;38;5;220m%s\\033[0m \\033[3;38;5;244m(%s CLI-backed)\\033[0m\\n' 'roster' {size} {clis}; \
                     n=${{#queue[@]}}; \
                     direct=0; broadcast=0; for_others=0; others_buf=\"\"; \
                     if [ \"$n\" -gt 0 ]; then \
                       call_lc=$(printf '%s' \"$callsign\" | tr '[:upper:]' '[:lower:]'); \
                       full=\"${{user}}@${{host}}\"; \
                       for f in \"${{queue[@]}}\"; do \
                         t=$(awk '/^to: /{{sub(/^to: /,\"\"); print; exit}}' \"$f\" 2>/dev/null); \
                         if [ -z \"$t\" ]; then \
                           broadcast=$((broadcast+1)); continue; \
                         fi; \
                         t_lc=$(printf '%s' \"$t\" | tr '[:upper:]' '[:lower:]'); \
                         if [ \"$t\" = \"$user\" ] || [ \"$t\" = \"$full\" ] || {{ [ -n \"$call_lc\" ] && [ \"$t_lc\" = \"$call_lc\" ]; }}; then \
                           direct=$((direct+1)); \
                         else \
                           for_others=$((for_others+1)); \
                           others_buf=\"$others_buf$t_lc\"$'\\n'; \
                         fi; \
                       done; \
                       if [ \"$direct\" -gt 0 ] && [ \"$broadcast\" -gt 0 ]; then \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📻 \\033[1;38;5;35m%d direct\\033[0m \\033[2;38;5;240m· \\033[0m\\033[1;38;5;220m%d broadcast\\033[0m \\033[2;38;5;240m· %d total\\033[0m \\033[3;38;5;244m— /inbox to read\\033[0m\\n' 'radio' \"$direct\" \"$broadcast\" \"$n\"; \
                       elif [ \"$direct\" -gt 0 ]; then \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📻 \\033[1;38;5;35m%d direct\\033[0m \\033[2;38;5;240m· %d total\\033[0m \\033[3;38;5;244m— /inbox to read\\033[0m\\n' 'radio' \"$direct\" \"$n\"; \
                       elif [ \"$broadcast\" -gt 0 ]; then \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📻 \\033[1;38;5;220m%d broadcast\\033[0m \\033[2;38;5;240m· %d total\\033[0m \\033[3;38;5;244m— /inbox to read\\033[0m\\n' 'radio' \"$broadcast\" \"$n\"; \
                       else \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📻 \\033[1;38;5;220m%d transmission(s) pending\\033[0m \\033[3;38;5;244m— /inbox to read\\033[0m\\n' 'radio' \"$n\"; \
                       fi; \
                       if [ -z \"$callsign\" ] && [ \"$for_others\" -gt 0 ]; then \
                         others_summary=$(printf '%b' \"$others_buf\" | sort | uniq -c | sort -rn | awk 'BEGIN {{ a[\"nicholas\"]=\"🎯\"; a[\"angel\"]=\"🎯\"; a[\"frank\"]=\"🦔\"; a[\"butterman.snr\"]=\"🦔\"; a[\"danny\"]=\"🍦\"; a[\"butterman\"]=\"🍦\"; a[\"andy\"]=\"🤡\"; a[\"wainwright\"]=\"🤡\"; a[\"cartwright\"]=\"🤡\"; a[\"doris\"]=\"🚓\"; a[\"thatcher\"]=\"🚓\"; a[\"tony\"]=\"📻\" }} NR<=4 {{ av=a[$2]; sep=(NR==1?\"\":\" \"); if (av) printf sep \"%s @%s:%d\", av, $2, $1; else printf sep \"@%s:%d\", $2, $1 }}'); \
                         printf '  \\033[2;38;5;244m%-10s\\033[0m 📨 \\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244m— /duty <name> to read\\033[0m\\n' 'for others' \"$others_summary\"; \
                       fi; \
                     else \
                       hour=$(date +%H); \
                       case \"$hour\" in \
                         0[0-5]) radio_msg='everyone tucked in' ;; \
                         0[6-9]|10) radio_msg='kettle on' ;; \
                         11|12|13) radio_msg='off to the supermarket' ;; \
                         14|15|16|17) radio_msg='cornetto weather' ;; \
                         18|19|20|21) radio_msg='down the Crown for one' ;; \
                         *) radio_msg='all quiet on the air' ;; \
                       esac; \
                       printf '  \\033[2;38;5;244m%-10s\\033[0m \\033[3;38;5;244m%s\\033[0m\\n' 'radio' \"$radio_msg\"; \
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
                let sub = argument.map(|a| a.trim()).unwrap_or("");
                if sub.eq_ignore_ascii_case("clear") {
                    let clear_cmd = "shopt -s nullglob; \
                        mkdir -p /tmp/yarp-radio; \
                        files=(/tmp/yarp-radio/*.msg); \
                        me_user=\"${USER:-unknown}\"; \
                        me_full=\"${me_user}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                        me_call=\"${YARP_CALLSIGN:-}\"; \
                        me_call_lc=$(printf '%s' \"$me_call\" | tr '[:upper:]' '[:lower:]'); \
                        removed=0; \
                        for f in \"${files[@]}\"; do \
                          t=$(awk '/^to: /{sub(/^to: /,\"\"); print; exit}' \"$f\" 2>/dev/null); \
                          if [ -z \"$t\" ]; then \
                            rm -f \"$f\" && removed=$((removed+1)); \
                          else \
                            t_lc=$(printf '%s' \"$t\" | tr '[:upper:]' '[:lower:]'); \
                            if [ \"$t\" = \"$me_user\" ] || [ \"$t\" = \"$me_full\" ] || { [ -n \"$me_call_lc\" ] && [ \"$t_lc\" = \"$me_call_lc\" ]; }; then \
                              rm -f \"$f\" && removed=$((removed+1)); \
                            fi; \
                          fi; \
                        done; \
                        if [ \"$removed\" -eq 0 ]; then \
                          printf '\\033[3;38;5;244m📻 INBOX  nothing to clear\\033[0m\\n'; \
                          empty_flavors=(\"All quiet on the western front. — Frank\" \"Already shipshape. — Nicholas\" \"Pub? — Danny\" \"Perfectly innocent explanation. — Angel\"); \
                          printf '  \\033[3;38;5;240m“%s”\\033[0m\\n' \"${empty_flavors[$((RANDOM % ${#empty_flavors[@]}))]}\"; \
                        else \
                          printf '\\033[1;38;5;220m📻 INBOX\\033[0m \\033[38;5;179mqueue cleared\\033[0m \\033[2;38;5;240m· %d transmission(s) discarded\\033[0m\\n' \"$removed\"; \
                          done_flavors=(\"Right then, sorted. — Frank\" \"Paperwork: tidy. — Nicholas\" \"By the power of Greyskull. — Danny\" \"Crusty Jugglers. — Andy\" \"Yarp. — Tony\"); \
                          printf '  \\033[3;38;5;240m“%s”\\033[0m\\n' \"${done_flavors[$((RANDOM % ${#done_flavors[@]}))]}\"; \
                        fi";
                    self.try_execute_command(clear_cmd, ctx);
                    return true;
                }
                let cmd = "shopt -s nullglob; \
                    mkdir -p /tmp/yarp-radio; \
                    pruned=$(find /tmp/yarp-radio -maxdepth 1 -name '*.msg' -mmin +60 -print -delete 2>/dev/null | wc -l | tr -d ' '); \
                    files=(/tmp/yarp-radio/*.msg); \
                    me_user=\"${USER:-unknown}\"; \
                    me_full=\"${me_user}@$(hostname -s 2>/dev/null || echo localhost)\"; \
                    me_call=\"${YARP_CALLSIGN:-}\"; \
                    me_call_lc=$(printf '%s' \"$me_call\" | tr '[:upper:]' '[:lower:]'); \
                    me_av=''; \
                    case \"$me_call_lc\" in \
                      nicholas|angel) me_av='🎯' ;; \
                      frank|butterman.snr) me_av='🦔' ;; \
                      danny|butterman) me_av='🍦' ;; \
                      andy|wainwright|cartwright) me_av='🤡' ;; \
                      doris|thatcher) me_av='🚓' ;; \
                      tony) me_av='📻' ;; \
                    esac; \
                    if [ -n \"$me_call\" ]; then \
                      if [ -n \"$me_av\" ]; then me_hdr=\" $me_av @$me_call\"; else me_hdr=\" @$me_call\"; fi; \
                    else me_hdr=''; fi; \
                    if [ ${#files[@]} -eq 0 ]; then \
                      if [ \"${pruned:-0}\" -gt 0 ]; then \
                        printf '\\033[3;38;5;244m📻 INBOX\\033[0m\\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244mno traffic\\033[0m \\033[2;38;5;240m· expired %s stale\\033[0m\\n' \"$me_hdr\" \"$pruned\"; \
                      else \
                        printf '\\033[3;38;5;244m📻 INBOX\\033[0m\\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244mno traffic\\033[0m\\n' \"$me_hdr\"; \
                      fi; \
                      flavors=(\"It's all gone a bit Pete Tong. — Andy\" \"Nothing happens here. — Nicholas\" \"Yarp. — Michael\" \"Pub? — Danny\" \"All quiet on Sandford. — Frank\" \"The greater good. — The NWA\" \"Yeah, but he gets to ride the horse. — Doris\"); \
                      printf '  \\033[3;38;5;240m“%s”\\033[0m\\n' \"${flavors[$((RANDOM % ${#flavors[@]}))]}\"; \
                    else \
                      if [ \"${pruned:-0}\" -gt 0 ]; then \
                        printf '\\033[1;38;5;220m📻 INBOX\\033[0m\\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244m%d transmission(s)\\033[0m \\033[2;38;5;240m· expired %s stale\\033[0m\\n\\n' \"$me_hdr\" \"${#files[@]}\" \"$pruned\"; \
                      else \
                        printf '\\033[1;38;5;220m📻 INBOX\\033[0m\\033[1;38;5;179m%s\\033[0m \\033[3;38;5;244m%d transmission(s)\\033[0m\\n\\n' \"$me_hdr\" \"${#files[@]}\"; \
                      fi; \
                      direct_q=(); broadcast_q=(); relay_q=(); \
                      for f in \"${files[@]}\"; do \
                        t=$(awk '/^to: /{sub(/^to: /,\"\"); print; exit}' \"$f\" 2>/dev/null); \
                        if [ -z \"$t\" ]; then \
                          broadcast_q+=(\"$f\"); \
                        else \
                          t_lc=$(printf '%s' \"$t\" | tr '[:upper:]' '[:lower:]'); \
                          if [ \"$t\" = \"$me_user\" ] || [ \"$t\" = \"$me_full\" ] || { [ -n \"$me_call_lc\" ] && [ \"$t_lc\" = \"$me_call_lc\" ]; }; then \
                            direct_q+=(\"$f\"); \
                          else \
                            relay_q+=(\"$f\"); \
                          fi; \
                        fi; \
                      done; \
                      print_msg() { \
                        local f=\"$1\" idx=\"$2\" kind=\"$3\"; \
                        local base ts ts_s human sender target body_start line badge sender_color body_color tag consume sender_lc avatar sender_disp; \
                        base=$(basename \"$f\" .msg); \
                        ts=${base%%-*}; \
                        ts_s=$((ts / 1000)); \
                        human=$(date -r \"$ts_s\" '+%H:%M:%S' 2>/dev/null || echo \"$ts\"); \
                        sender=unknown; target=\"\"; body_start=1; \
                        while IFS= read -r line; do \
                          case \"$line\" in \
                            from:*) sender=\"${line#from: }\"; body_start=$((body_start+1));; \
                            to:*)   target=\"${line#to: }\";   body_start=$((body_start+1));; \
                            \"\")    body_start=$((body_start+1)); break;; \
                            *)      break;; \
                          esac; \
                        done < \"$f\"; \
                        case \"$kind\" in \
                          direct)    badge='\\033[1;38;5;35m▸ DIRECT\\033[0m '; sender_color='\\033[1;38;5;35m'; body_color='\\033[38;5;179m'; consume=1 ;; \
                          broadcast) badge='\\033[1;38;5;220m📻 ALL   \\033[0m '; sender_color='\\033[1;38;5;220m'; body_color='\\033[38;5;178m'; consume=1 ;; \
                          relay)     badge='\\033[2;38;5;240m▸ relay  \\033[0m '; sender_color='\\033[2;38;5;240m'; body_color='\\033[2;38;5;240m'; consume=0 ;; \
                        esac; \
                        if [ -n \"$target\" ]; then tag=\" → @${target}\"; else tag=''; fi; \
                        sender_lc=$(printf '%s' \"$sender\" | tr '[:upper:]' '[:lower:]' | sed 's/^@//'); \
                        case \"$sender_lc\" in \
                          nicholas|angel) avatar='🎯' ;; \
                          frank|butterman.snr) avatar='🦔' ;; \
                          danny|butterman) avatar='🍦' ;; \
                          andy|wainwright|cartwright) avatar='🤡' ;; \
                          doris|thatcher) avatar='🚓' ;; \
                          tony) avatar='📻' ;; \
                          *) avatar='' ;; \
                        esac; \
                        if [ -n \"$avatar\" ]; then sender_disp=\"$avatar $sender\"; else sender_disp=\"$sender\"; fi; \
                        printf '  \\033[2;3;38;5;240m%2d\\033[0m %b\\033[2;3;38;5;244m[%s]\\033[0m %b%s\\033[0m\\033[3;38;5;244m%s\\033[0m\\n' \"$idx\" \"$badge\" \"$human\" \"$sender_color\" \"$sender_disp\" \"$tag\"; \
                        tail -n +$body_start \"$f\" 2>/dev/null | while IFS= read -r line; do printf '    %b%s\\033[0m\\n' \"$body_color\" \"$line\"; done; \
                        if [ \"$consume\" = 1 ]; then rm -f \"$f\"; fi; \
                        echo; \
                      }; \
                      idx=0; \
                      if [ ${#direct_q[@]} -gt 0 ]; then \
                        printf '\\033[1;38;5;35m  ▸ DIRECT\\033[0m \\033[2;3;38;5;240m(%d)\\033[0m\\n' \"${#direct_q[@]}\"; \
                        for f in \"${direct_q[@]}\"; do idx=$((idx+1)); print_msg \"$f\" \"$idx\" direct; done; \
                      fi; \
                      if [ ${#broadcast_q[@]} -gt 0 ]; then \
                        printf '\\033[1;38;5;220m  📻 BROADCASTS\\033[0m \\033[2;3;38;5;240m(%d)\\033[0m\\n' \"${#broadcast_q[@]}\"; \
                        for f in \"${broadcast_q[@]}\"; do idx=$((idx+1)); print_msg \"$f\" \"$idx\" broadcast; done; \
                      fi; \
                      if [ ${#relay_q[@]} -gt 0 ]; then \
                        printf '\\033[2;38;5;240m  ▸ RELAY\\033[0m \\033[2;3;38;5;240m(%d)\\033[0m \\033[3;38;5;244mnot for you\\033[0m\\n' \"${#relay_q[@]}\"; \
                        for f in \"${relay_q[@]}\"; do idx=$((idx+1)); print_msg \"$f\" \"$idx\" relay; done; \
                      fi; \
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

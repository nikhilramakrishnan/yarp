//! Local-only `AIClient` implementation used by warp-oss.
//!
//! warp-oss does not have a Warp backend. The five methods that drive an LLM
//! call (dialogue, command suggestions, command metadata, code review copy)
//! will eventually route through a local provider — for now they return a
//! clear error so callers see something actionable instead of a `localhost.invalid`
//! connection failure. State-tracking methods (agent task registry, request
//! quota, model discovery) are real implementations backed by `~/.warp-oss/`.
//!
//! Cloud-only methods (artifact upload, ambient/scheduled agent runs, server
//! conversation history, orchestration messaging) return `Err` or empty
//! results — the corresponding feature flags are off in the OSS build, so
//! these paths shouldn't fire from happy-path UI.
use std::collections::HashMap;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use ai::index::full_source_code_embedding::{
    self,
    store_client::IntermediateNode,
    ContentHash, EmbeddingConfig, NodeHash, RepoMetadata,
};
use warp_graphql::ai::AgentTaskState;
use warp_multi_agent_api::ConversationData;

use crate::ai::agent::api::ServerConversationToken;
use crate::ai::agent::conversation::{AIAgentConversationFormat, ServerAIConversationMetadata};
use crate::ai::ambient_agents::{
    task::{AmbientAgentTaskState, TaskAttachment},
    AgentConfigSnapshot, AmbientAgentTask, AmbientAgentTaskId,
};
use crate::ai::generate_code_review_content::api::{
    GenerateCodeReviewContentRequest, GenerateCodeReviewContentResponse,
};
use crate::ai::llms::ModelsByFeature;
use crate::ai::request_usage_model::{RequestLimitInfo, RequestUsageInfo};
use crate::ai_assistant::{
    execution_context::WarpAiExecutionContext, requests::GenerateDialogueResult,
    utils::TranscriptPart, AIGeneratedCommand, GenerateCommandsFromNaturalLanguageError,
};
use crate::drive::workflows::ai_assist::{GeneratedCommandMetadata, GeneratedCommandMetadataError};
use crate::server::server_api::ai::{
    AIClient, AgentListItem, AgentMessageHeader, AgentRunEvent, ArtifactDownloadResponse,
    AttachmentFileInfo, CreateFileArtifactUploadRequest, CreateFileArtifactUploadResponse,
    DownloadAttachmentsResponse, FileArtifactRecord, ListAgentMessagesRequest,
    PrepareAttachmentUploadsResponse, ReadAgentMessageResponse, ReportAgentEventRequest,
    ReportAgentEventResponse, SendAgentMessageRequest, SendAgentMessageResponse,
    SpawnAgentRequest, SpawnAgentResponse, TaskListFilter, TaskStatusUpdate,
};
use crate::terminal::model::block::SerializedBlock;
use warp_graphql::queries::get_scheduled_agent_history::ScheduledAgentHistory;

use super::llm_provider::{LocalLlmProvider, Message, Role};
use super::LocalBackend;

const NOT_SUPPORTED: &str = "not supported in warp-oss; use a local agent harness instead";

const COMMANDS_SYSTEM_PROMPT: &str = r#"You translate natural-language requests into shell commands. Reply with ONLY a single JSON object — no prose, no code fences. Schema:
{
  "commands": [
    {
      "command": "the shell command, with {{arg_name}} placeholders for parameters",
      "description": "what this command does (one short sentence)",
      "parameters": [ { "id": "arg_name", "description": "what this arg is" } ]
    }
  ]
}
Return one to three commands. Prefer the most direct command first."#;

const METADATA_SYSTEM_PROMPT: &str = r#"You analyse a single shell command and return a structured description. Reply with ONLY a single JSON object — no prose, no code fences. Schema:
{
  "command": "the original command, parameterized with {{arg_name}} placeholders for any user-tweakable values",
  "title": "short imperative title (max 60 chars)",
  "description": "longer explanation of what the command does",
  "arguments": [ { "name": "arg_name", "description": "what this arg is", "default_value": "the original literal" } ]
}"#;

#[derive(serde::Deserialize)]
struct CommandsJson {
    commands: Vec<CommandJson>,
}

#[derive(serde::Deserialize)]
struct CommandJson {
    command: String,
    description: String,
    #[serde(default)]
    parameters: Vec<CommandParamJson>,
}

#[derive(serde::Deserialize)]
struct CommandParamJson {
    id: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct MetadataJson {
    command: String,
    title: String,
    description: String,
    #[serde(default)]
    arguments: Vec<MetadataArgJson>,
}

#[derive(serde::Deserialize)]
struct MetadataArgJson {
    name: String,
    description: String,
    #[serde(default)]
    default_value: String,
}

fn extract_json_object(raw: &str) -> &str {
    // LLMs sometimes wrap JSON in code fences or surrounding prose. Pull out the
    // first {...} balanced block.
    let bytes = raw.as_bytes();
    let mut start = None;
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    if let Some(s) = start {
                        return &raw[s..=i];
                    }
                }
            }
            _ => {}
        }
    }
    raw
}

/// Local-only `AIClient`. Holds a [`LocalBackend`] for file-backed state.
pub struct OssAiClient {
    backend: LocalBackend,
}

impl OssAiClient {
    pub fn new(backend: LocalBackend) -> Self {
        Self { backend }
    }

    fn tasks_path(&self) -> std::path::PathBuf {
        self.backend.paths().agent_tasks_file()
    }

    fn load_tasks(&self) -> Vec<AmbientAgentTask> {
        self.backend
            .file_store()
            .read_json::<Vec<AmbientAgentTask>>(&self.tasks_path())
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn save_tasks(&self, tasks: &[AmbientAgentTask]) -> anyhow::Result<()> {
        self.backend
            .file_store()
            .write_json(&self.tasks_path(), &tasks.to_vec())
    }

    fn fresh_task_id() -> AmbientAgentTaskId {
        // Loop on the (cosmically unlikely) chance that v4 generates the nil UUID.
        loop {
            let id = Uuid::new_v4();
            if let Ok(parsed) = id.to_string().parse::<AmbientAgentTaskId>() {
                return parsed;
            }
        }
    }
}

#[async_trait]
impl AIClient for OssAiClient {
    // ---- Inference: routed through the configured local LLM provider. ----

    async fn generate_commands_from_natural_language(
        &self,
        prompt: String,
        _ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> Result<Vec<AIGeneratedCommand>, GenerateCommandsFromNaturalLanguageError> {
        let provider = LocalLlmProvider::from_env();
        let response = provider
            .chat(
                Some(COMMANDS_SYSTEM_PROMPT),
                vec![Message {
                    role: Role::User,
                    content: prompt,
                }],
            )
            .await
            .map_err(|err| {
                log::warn!("warp-oss: command-suggestion LLM call failed: {err:#}");
                GenerateCommandsFromNaturalLanguageError::AiProviderError
            })?;
        let parsed: CommandsJson = serde_json::from_str(extract_json_object(&response))
            .map_err(|err| {
                log::warn!("warp-oss: command-suggestion JSON parse failed: {err:#}; raw={response}");
                GenerateCommandsFromNaturalLanguageError::BadPrompt
            })?;
        Ok(parsed
            .commands
            .into_iter()
            .map(|c| {
                AIGeneratedCommand::new(
                    c.command,
                    c.description,
                    c.parameters
                        .into_iter()
                        .map(|p| {
                            crate::ai_assistant::AIGeneratedCommandParameter::new(
                                p.id,
                                p.description,
                            )
                        })
                        .collect(),
                )
            })
            .collect())
    }

    async fn generate_dialogue_answer(
        &self,
        transcript: Vec<TranscriptPart>,
        prompt: String,
        _ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> anyhow::Result<GenerateDialogueResult> {
        let provider = LocalLlmProvider::from_env();
        let mut messages: Vec<Message> = Vec::new();
        for part in &transcript {
            messages.push(Message {
                role: Role::User,
                content: part.raw_user_prompt().to_string(),
            });
            let assistant = part.raw_assistant_answer().to_string();
            if !assistant.is_empty() {
                messages.push(Message {
                    role: Role::Assistant,
                    content: assistant,
                });
            }
        }
        messages.push(Message {
            role: Role::User,
            content: prompt,
        });
        let answer = provider.chat(None, messages).await?;
        Ok(GenerateDialogueResult::Success {
            answer,
            truncated: false,
            request_limit_info: RequestLimitInfo {
                is_unlimited: true,
                ..RequestLimitInfo::default()
            },
            transcript_summarized: false,
        })
    }

    async fn generate_metadata_for_command(
        &self,
        command: String,
    ) -> Result<GeneratedCommandMetadata, GeneratedCommandMetadataError> {
        let provider = LocalLlmProvider::from_env();
        let response = provider
            .chat(
                Some(METADATA_SYSTEM_PROMPT),
                vec![Message {
                    role: Role::User,
                    content: command,
                }],
            )
            .await
            .map_err(|err| {
                log::warn!("warp-oss: command-metadata LLM call failed: {err:#}");
                GeneratedCommandMetadataError::AiProviderError
            })?;
        let parsed: MetadataJson = serde_json::from_str(extract_json_object(&response))
            .map_err(|err| {
                log::warn!(
                    "warp-oss: command-metadata JSON parse failed: {err:#}; raw={response}"
                );
                GeneratedCommandMetadataError::BadCommand
            })?;
        Ok(GeneratedCommandMetadata {
            command: parsed.command,
            title: parsed.title,
            description: parsed.description,
            arguments: parsed
                .arguments
                .into_iter()
                .map(|a| crate::drive::workflows::ai_assist::GeneratedArgument {
                    name: a.name,
                    description: a.description,
                    default_value: a.default_value,
                })
                .collect(),
        })
    }

    async fn generate_code_review_content(
        &self,
        request: GenerateCodeReviewContentRequest,
    ) -> Result<GenerateCodeReviewContentResponse, anyhow::Error> {
        let provider = LocalLlmProvider::from_env();
        let system_prompt = code_review_system_prompt(&request);
        let user_content = format!(
            "Branch: {}\n\nRecent commits:\n{}\n\nDiff:\n{}\n",
            request.branch_name,
            request.commit_messages.join("\n"),
            request.diff
        );
        let content = provider
            .chat(
                Some(&system_prompt),
                vec![Message {
                    role: Role::User,
                    content: user_content,
                }],
            )
            .await?;
        Ok(GenerateCodeReviewContentResponse {
            content: content.trim().to_string(),
        })
    }

    // ---- Quota / model discovery. Synthesize unlimited. ----

    async fn get_request_limit_info(&self) -> Result<RequestUsageInfo, anyhow::Error> {
        Ok(RequestUsageInfo {
            request_limit_info: RequestLimitInfo {
                is_unlimited: true,
                ..RequestLimitInfo::default()
            },
            bonus_grants: Vec::new(),
        })
    }

    async fn get_feature_model_choices(&self) -> Result<ModelsByFeature, anyhow::Error> {
        Ok(ModelsByFeature::default())
    }

    async fn get_free_available_models(
        &self,
        _referrer: Option<String>,
    ) -> Result<ModelsByFeature, anyhow::Error> {
        Ok(ModelsByFeature::default())
    }

    // ---- Embeddings: defer until local embedding provider lands. ----

    async fn update_merkle_tree(
        &self,
        _embedding_config: EmbeddingConfig,
        _nodes: Vec<IntermediateNode>,
    ) -> anyhow::Result<HashMap<NodeHash, bool>> {
        Ok(HashMap::new())
    }

    async fn generate_code_embeddings(
        &self,
        _embedding_config: EmbeddingConfig,
        _fragments: Vec<full_source_code_embedding::Fragment>,
        _root_hash: NodeHash,
        _repo_metadata: RepoMetadata,
    ) -> anyhow::Result<HashMap<ContentHash, bool>> {
        Ok(HashMap::new())
    }

    // ---- Agent telemetry: silent no-op. ----

    async fn provide_negative_feedback_response_for_ai_conversation(
        &self,
        _conversation_id: String,
        _request_ids: Vec<String>,
    ) -> anyhow::Result<i32, anyhow::Error> {
        Ok(0)
    }

    // ---- Agent task registry: backed by ~/.warp-oss/agent_tasks.json. ----

    async fn create_agent_task(
        &self,
        prompt: String,
        _environment_uid: Option<String>,
        parent_run_id: Option<String>,
        config: Option<AgentConfigSnapshot>,
    ) -> anyhow::Result<AmbientAgentTaskId, anyhow::Error> {
        let task_id = Self::fresh_task_id();
        let now = Utc::now();
        let task = AmbientAgentTask {
            task_id,
            parent_run_id,
            title: prompt.lines().next().unwrap_or_default().to_string(),
            state: AmbientAgentTaskState::Pending,
            prompt,
            created_at: now,
            started_at: Some(now),
            updated_at: now,
            status_message: None,
            source: None,
            session_id: None,
            session_link: None,
            creator: None,
            conversation_id: None,
            request_usage: None,
            is_sandbox_running: false,
            agent_config_snapshot: config,
            artifacts: Vec::new(),
            last_event_sequence: None,
            children: Vec::new(),
        };
        let mut tasks = self.load_tasks();
        tasks.push(task);
        self.save_tasks(&tasks)?;
        Ok(task_id)
    }

    async fn update_agent_task(
        &self,
        task_id: AmbientAgentTaskId,
        task_state: Option<AgentTaskState>,
        session_id: Option<session_sharing_protocol::common::SessionId>,
        conversation_id: Option<String>,
        status_message: Option<TaskStatusUpdate>,
    ) -> anyhow::Result<(), anyhow::Error> {
        let mut tasks = self.load_tasks();
        let Some(task) = tasks.iter_mut().find(|t| t.task_id == task_id) else {
            return Err(anyhow!("agent task {task_id} not found"));
        };
        if let Some(state) = task_state {
            task.state = map_graphql_task_state(state);
        }
        if let Some(id) = session_id {
            task.session_id = Some(id.to_string());
        }
        if let Some(id) = conversation_id {
            task.conversation_id = Some(id);
        }
        if let Some(update) = status_message {
            task.status_message = Some(crate::ai::ambient_agents::TaskStatusMessage {
                message: update.message,
            });
        }
        task.updated_at = Utc::now();
        self.save_tasks(&tasks)
    }

    async fn spawn_agent(
        &self,
        _request: SpawnAgentRequest,
    ) -> anyhow::Result<SpawnAgentResponse, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn list_ambient_agent_tasks(
        &self,
        limit: i32,
        _filter: TaskListFilter,
    ) -> anyhow::Result<Vec<AmbientAgentTask>, anyhow::Error> {
        let mut tasks = self.load_tasks();
        tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        if limit > 0 {
            tasks.truncate(limit as usize);
        }
        Ok(tasks)
    }

    async fn list_agent_runs_raw(
        &self,
        _limit: i32,
        _filter: TaskListFilter,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Ok(serde_json::json!({ "runs": [] }))
    }

    async fn get_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<AmbientAgentTask, anyhow::Error> {
        self.load_tasks()
            .into_iter()
            .find(|t| &t.task_id == task_id)
            .ok_or_else(|| anyhow!("agent task {task_id} not found"))
    }

    async fn get_agent_run_raw(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let task = self.get_ambient_agent_task(task_id).await?;
        Ok(serde_json::to_value(task)?)
    }

    async fn get_scheduled_agent_history(
        &self,
        _schedule_id: &str,
    ) -> anyhow::Result<ScheduledAgentHistory, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn cancel_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<(), anyhow::Error> {
        let mut tasks = self.load_tasks();
        tasks.retain(|t| &t.task_id != task_id);
        self.save_tasks(&tasks)
    }

    // ---- Conversation history: TODO Phase 6+. Empty/Err for now. ----

    async fn get_ai_conversation(
        &self,
        _server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<(ConversationData, ServerAIConversationMetadata), anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn list_ai_conversation_metadata(
        &self,
        _conversation_ids: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<ServerAIConversationMetadata>> {
        Ok(Vec::new())
    }

    async fn get_ai_conversation_format(
        &self,
        _server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<AIAgentConversationFormat, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn get_block_snapshot(
        &self,
        _server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<SerializedBlock, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn delete_ai_conversation(
        &self,
        _server_conversation_token: String,
    ) -> anyhow::Result<(), anyhow::Error> {
        Ok(())
    }

    async fn list_agents(
        &self,
        _repo: Option<String>,
    ) -> anyhow::Result<Vec<AgentListItem>, anyhow::Error> {
        Ok(Vec::new())
    }

    // ---- Attachments / artifacts: cloud-only, no-op for now. ----

    async fn get_task_attachments(
        &self,
        _task_id: String,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn create_file_artifact_upload_target(
        &self,
        _request: CreateFileArtifactUploadRequest,
    ) -> anyhow::Result<CreateFileArtifactUploadResponse, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn confirm_file_artifact_upload(
        &self,
        _artifact_uid: String,
        _checksum: String,
    ) -> anyhow::Result<FileArtifactRecord, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn get_artifact_download(
        &self,
        _artifact_uid: &str,
    ) -> anyhow::Result<ArtifactDownloadResponse, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn prepare_attachments_for_upload(
        &self,
        _task_id: &AmbientAgentTaskId,
        _files: &[AttachmentFileInfo],
    ) -> anyhow::Result<PrepareAttachmentUploadsResponse, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn download_task_attachments(
        &self,
        _task_id: &AmbientAgentTaskId,
        _attachment_ids: &[String],
    ) -> anyhow::Result<DownloadAttachmentsResponse, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn get_handoff_snapshot_attachments(
        &self,
        _task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error> {
        Ok(Vec::new())
    }

    // ---- Orchestrations V2 messaging: no-op until local pub/sub lands. ----

    async fn send_agent_message(
        &self,
        _request: SendAgentMessageRequest,
    ) -> anyhow::Result<SendAgentMessageResponse, anyhow::Error> {
        Ok(SendAgentMessageResponse {
            message_ids: Vec::new(),
        })
    }

    async fn list_agent_messages(
        &self,
        _run_id: &str,
        _request: ListAgentMessagesRequest,
    ) -> anyhow::Result<Vec<AgentMessageHeader>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn poll_agent_events(
        &self,
        _run_ids: &[String],
        _since_sequence: i64,
        _limit: i32,
    ) -> anyhow::Result<Vec<AgentRunEvent>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn update_event_sequence_on_server(
        &self,
        _run_id: &str,
        _sequence: i64,
    ) -> anyhow::Result<(), anyhow::Error> {
        Ok(())
    }

    async fn report_agent_event(
        &self,
        _run_id: &str,
        _request: ReportAgentEventRequest,
    ) -> anyhow::Result<ReportAgentEventResponse, anyhow::Error> {
        Ok(ReportAgentEventResponse { sequence: 0 })
    }

    async fn mark_message_delivered(&self, _message_id: &str) -> anyhow::Result<(), anyhow::Error> {
        Ok(())
    }

    async fn read_agent_message(
        &self,
        message_id: &str,
    ) -> anyhow::Result<ReadAgentMessageResponse, anyhow::Error> {
        Err(anyhow!(
            "read_agent_message: message {message_id} not available locally"
        ))
    }

    async fn get_public_conversation(
        &self,
        _conversation_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }

    async fn get_run_conversation(
        &self,
        _run_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!(NOT_SUPPORTED))
    }
}

fn code_review_system_prompt(request: &GenerateCodeReviewContentRequest) -> String {
    use crate::ai::generate_code_review_content::api::OutputType;
    match request.output_type {
        OutputType::CommitMessage => {
            "You write conventional-commit-style messages from a diff. Reply with ONLY the commit \
             message: a single short subject line (max 72 chars) optionally followed by a blank line \
             and a body. No code fences, no preamble."
                .to_string()
        }
        OutputType::PrTitle => {
            "You write concise pull-request titles from a diff and recent commit messages. Reply with \
             ONLY the title (max 70 chars). No prefixes, no quotes."
                .to_string()
        }
        OutputType::PrDescription => {
            "You write pull-request descriptions in markdown from a diff and recent commit messages. \
             Use a Summary section (1-3 bullet points) and a Test plan section (markdown checklist). \
             Reply with ONLY the markdown body."
                .to_string()
        }
    }
}

fn map_graphql_task_state(state: AgentTaskState) -> AmbientAgentTaskState {
    match state {
        AgentTaskState::Blocked => AmbientAgentTaskState::Blocked,
        AgentTaskState::Cancelled => AmbientAgentTaskState::Cancelled,
        AgentTaskState::Claimed => AmbientAgentTaskState::Claimed,
        AgentTaskState::Error => AmbientAgentTaskState::Error,
        AgentTaskState::InProgress => AmbientAgentTaskState::InProgress,
        AgentTaskState::Succeeded => AmbientAgentTaskState::Succeeded,
        AgentTaskState::Failed => AmbientAgentTaskState::Failed,
    }
}

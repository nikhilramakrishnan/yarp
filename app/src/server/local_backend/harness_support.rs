//! Local-only `HarnessSupportClient` for yarp.
//!
//! Third-party CLI harnesses (Claude Code, OpenCode, Gemini) want a place to
//! park transcripts and block snapshots. In yarp proper this is GCS via signed
//! upload URLs; here we hand out `file://` targets under `~/.yarp/harness/`.
//! The shared harness upload helper writes those targets directly, so local
//! harness save/resume has the same success contract as the hosted path without
//! leaking fake network URLs.
//!
//! `create_external_conversation` is the only method that *must* succeed for a
//! harness launch — we generate a UUID and create `~/.yarp/harness/{id}/`
//! up front so the directory exists before subprocess spawn.

use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use http_client::Client as HttpClient;

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskId};
use crate::ai::artifacts::Artifact;
use crate::server::server_api::harness_support::{
    HarnessSupportClient, ReportArtifactResponse, ResolvePromptRequest, ResolvedHarnessPrompt,
    SnapshotUploadRequest, UploadTarget,
};

use super::LocalBackend;

pub struct OssHarnessSupportClient {
    backend: LocalBackend,
    http: HttpClient,
    task_id: Option<AmbientAgentTaskId>,
}

impl OssHarnessSupportClient {
    pub fn new(backend: LocalBackend, task_id: Option<AmbientAgentTaskId>) -> Self {
        Self {
            backend,
            http: HttpClient::new(),
            task_id,
        }
    }

    #[cfg(test)]
    fn new_for_test(backend: LocalBackend, task_id: Option<AmbientAgentTaskId>) -> Self {
        Self {
            backend,
            http: HttpClient::new_for_test(),
            task_id,
        }
    }

    fn file_target(&self, path: PathBuf) -> Result<UploadTarget> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create local upload dir {}", parent.display())
            })?;
        }
        let url = url::Url::from_file_path(&path).map_err(|_| {
            anyhow::anyhow!(
                "failed to convert local upload path to file URL: {}",
                path.display()
            )
        })?;
        Ok(UploadTarget {
            url: url.to_string(),
            method: "PUT".into(),
            headers: HashMap::new(),
        })
    }

    fn conversation_target(
        &self,
        conversation_id: &AIConversationId,
        filename: &str,
    ) -> Result<UploadTarget> {
        self.file_target(
            self.backend
                .paths()
                .harness_run_dir(&conversation_id.to_string())
                .join(filename),
        )
    }

    fn snapshot_target(
        &self,
        upload_id: &str,
        index: usize,
        filename: &str,
    ) -> Result<UploadTarget> {
        let filename = sanitize_filename(filename);
        self.file_target(
            self.backend
                .paths()
                .harness_dir()
                .join("snapshots")
                .join(upload_id)
                .join(format!("{index}-{filename}")),
        )
    }

    fn load_tasks(&self) -> Vec<AmbientAgentTask> {
        self.backend
            .file_store()
            .read_json::<Vec<AmbientAgentTask>>(&self.backend.paths().agent_tasks_file())
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn save_tasks(&self, tasks: &[AmbientAgentTask]) -> Result<()> {
        self.backend
            .file_store()
            .write_json(&self.backend.paths().agent_tasks_file(), &tasks.to_vec())
    }

    fn current_task(&self) -> Option<AmbientAgentTask> {
        let task_id = self.task_id?;
        self.load_tasks()
            .into_iter()
            .find(|task| task.task_id == task_id)
    }

    fn link_current_task_to_conversation(&self, conversation_id: &AIConversationId) -> Result<()> {
        let Some(task_id) = self.task_id else {
            return Ok(());
        };
        let mut tasks = self.load_tasks();
        if let Some(task) = tasks.iter_mut().find(|task| task.task_id == task_id) {
            task.conversation_id = Some(conversation_id.to_string());
            task.updated_at = Utc::now();
            self.save_tasks(&tasks)?;
        }
        Ok(())
    }
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "snapshot".to_string()
    } else {
        sanitized
    }
}

#[async_trait]
impl HarnessSupportClient for OssHarnessSupportClient {
    async fn create_external_conversation(&self, _format: &str) -> Result<AIConversationId> {
        let id = AIConversationId::new();
        let dir = self.backend.paths().harness_run_dir(&id.to_string());
        std::fs::create_dir_all(&dir).ok();
        self.link_current_task_to_conversation(&id)?;
        Ok(id)
    }

    async fn get_transcript_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> Result<UploadTarget> {
        self.conversation_target(conversation_id, "transcript.json")
    }

    async fn get_block_snapshot_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> Result<UploadTarget> {
        self.conversation_target(conversation_id, "block_snapshot.json")
    }

    async fn resolve_prompt(&self, request: ResolvePromptRequest) -> Result<ResolvedHarnessPrompt> {
        let task = self.current_task().ok_or_else(|| {
            anyhow::anyhow!("yarp: cannot resolve stored task prompt without a local task id")
        })?;
        let mut prompt = task.prompt;
        if let Some(attachments_dir) = request.attachments_dir.filter(|dir| !dir.is_empty()) {
            prompt.push_str("\n\nAttachments are available at: ");
            prompt.push_str(&attachments_dir);
        }
        let system_prompt = request.skill.map(|skill| {
            let mut content = skill.content;
            if let Some(path) = skill.path.filter(|path| !path.is_empty()) {
                content.push_str("\n\nSkill file: ");
                content.push_str(&path);
            }
            content
        });
        Ok(ResolvedHarnessPrompt {
            prompt,
            system_prompt,
            resumption_prompt: None,
        })
    }

    async fn report_artifact(&self, _artifact: &Artifact) -> Result<ReportArtifactResponse> {
        // Artifacts are tracked locally inside `AmbientAgentTask::artifacts` via
        // `OssAiClient::update_agent_task`; no separate upload step.
        Ok(ReportArtifactResponse {
            artifact_uid: String::new(),
        })
    }

    async fn notify_user(&self, _message: &str) -> Result<()> {
        Ok(())
    }

    async fn finish_task(&self, _success: bool, _summary: &str) -> Result<()> {
        Ok(())
    }

    async fn get_snapshot_upload_targets(
        &self,
        request: &SnapshotUploadRequest,
    ) -> Result<Vec<UploadTarget>> {
        let upload_id = uuid::Uuid::new_v4().simple().to_string();
        request
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| self.snapshot_target(&upload_id, i, &f.filename))
            .collect()
    }

    async fn fetch_transcript(&self, conversation_id: &AIConversationId) -> Result<bytes::Bytes> {
        let path = self
            .backend
            .paths()
            .harness_run_dir(&conversation_id.to_string())
            .join("transcript.json");
        let bytes = async_fs::read(&path).await.with_context(|| {
            format!(
                "status 404: local harness transcript not found at {}",
                path.display()
            )
        })?;
        Ok(bytes::Bytes::from(bytes))
    }

    fn http_client(&self) -> &HttpClient {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use futures::executor::block_on;
    use tempfile::tempdir;

    use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskId, AmbientAgentTaskState};
    use crate::server::server_api::harness_support::{
        ResolvePromptAttachedSkill, ResolvePromptRequest,
    };

    use super::*;

    fn task_id() -> AmbientAgentTaskId {
        uuid::Uuid::new_v4().to_string().parse().unwrap()
    }

    fn task(task_id: AmbientAgentTaskId, prompt: &str) -> AmbientAgentTask {
        let now = Utc::now();
        AmbientAgentTask {
            task_id,
            parent_run_id: None,
            title: "Case".into(),
            state: AmbientAgentTaskState::Pending,
            prompt: prompt.into(),
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
            agent_config_snapshot: None,
            artifacts: Vec::new(),
            last_event_sequence: None,
            children: Vec::new(),
        }
    }

    fn client_with_task(
        backend: &LocalBackend,
        task_id: AmbientAgentTaskId,
        prompt: &str,
    ) -> OssHarnessSupportClient {
        let tasks = vec![task(task_id, prompt)];
        backend
            .file_store()
            .write_json(&backend.paths().agent_tasks_file(), &tasks)
            .unwrap();
        OssHarnessSupportClient::new_for_test(backend.clone(), Some(task_id))
    }

    #[test]
    fn resolve_prompt_reads_local_task_prompt() {
        block_on(async {
            let tempdir = tempdir().unwrap();
            let backend = LocalBackend::with_root_for_test(tempdir.path().to_path_buf());
            let task_id = task_id();
            let client = client_with_task(&backend, task_id, "Investigate the follow-up");

            let prompt = client
                .resolve_prompt(ResolvePromptRequest {
                    skill: Some(ResolvePromptAttachedSkill {
                        name: "detective".into(),
                        content: "Coordinate multiple specialists.".into(),
                        path: Some("/tmp/detective/SKILL.md".into()),
                    }),
                    attachments_dir: Some("/tmp/attachments".into()),
                })
                .await
                .unwrap();

            assert_eq!(
                prompt.prompt,
                "Investigate the follow-up\n\nAttachments are available at: /tmp/attachments"
            );
            assert_eq!(
                prompt.system_prompt.as_deref(),
                Some("Coordinate multiple specialists.\n\nSkill file: /tmp/detective/SKILL.md")
            );
            assert_eq!(prompt.resumption_prompt, None);
        });
    }

    #[test]
    fn create_external_conversation_links_local_task() {
        block_on(async {
            let tempdir = tempdir().unwrap();
            let backend = LocalBackend::with_root_for_test(tempdir.path().to_path_buf());
            let task_id = task_id();
            let client = client_with_task(&backend, task_id, "Investigate the follow-up");

            let conversation_id = client.create_external_conversation("claude").await.unwrap();

            assert!(backend
                .paths()
                .harness_run_dir(&conversation_id.to_string())
                .is_dir());
            let tasks = backend
                .file_store()
                .read_json::<Vec<AmbientAgentTask>>(&backend.paths().agent_tasks_file())
                .unwrap()
                .unwrap();
            assert_eq!(
                tasks[0].conversation_id.as_deref(),
                Some(conversation_id.to_string().as_str())
            );
        });
    }
}

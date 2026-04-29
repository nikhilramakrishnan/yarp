//! Local-only `HarnessSupportClient` for warp-oss.
//!
//! Third-party CLI harnesses (Claude Code, OpenCode, Gemini) want a place to
//! park transcripts and block snapshots. In warp proper this is GCS via signed
//! upload URLs; here we hand out `https://localhost.invalid/...` sentinel URLs.
//! The actual upload-PUT calls fail silently (logged as `warn` by the harness
//! driver — non-blocking), and we serve `fetch_transcript` from disk when a
//! session is resumed.
//!
//! `create_external_conversation` is the only method that *must* succeed for a
//! harness launch — we generate a UUID and create `~/.warp-oss/harness/{id}/`
//! up front so the directory exists before subprocess spawn.

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use http_client::Client as HttpClient;

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::artifacts::Artifact;
use crate::server::server_api::harness_support::{
    HarnessSupportClient, ReportArtifactResponse, ResolvePromptRequest, ResolvedHarnessPrompt,
    SnapshotUploadRequest, UploadTarget,
};

use super::LocalBackend;

const SENTINEL_HOST: &str = "https://localhost.invalid";

pub struct OssHarnessSupportClient {
    backend: LocalBackend,
    http: HttpClient,
}

impl OssHarnessSupportClient {
    pub fn new(backend: LocalBackend) -> Self {
        Self {
            backend,
            http: HttpClient::new(),
        }
    }

    fn sentinel_target(&self, kind: &str, id: &str) -> UploadTarget {
        UploadTarget {
            url: format!("{SENTINEL_HOST}/warp-oss/{kind}/{id}"),
            method: "PUT".into(),
            headers: HashMap::new(),
        }
    }
}

#[async_trait]
impl HarnessSupportClient for OssHarnessSupportClient {
    async fn create_external_conversation(&self, _format: &str) -> Result<AIConversationId> {
        let id = AIConversationId::new();
        let dir = self.backend.paths().harness_run_dir(&id.to_string());
        std::fs::create_dir_all(&dir).ok();
        Ok(id)
    }

    async fn get_transcript_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> Result<UploadTarget> {
        Ok(self.sentinel_target("transcript", &conversation_id.to_string()))
    }

    async fn get_block_snapshot_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> Result<UploadTarget> {
        Ok(self.sentinel_target("block_snapshot", &conversation_id.to_string()))
    }

    async fn resolve_prompt(
        &self,
        _request: ResolvePromptRequest,
    ) -> Result<ResolvedHarnessPrompt> {
        // No remote prompt resolution in OSS — the harness CLI is invoked with
        // the raw user prompt the local launcher already has in hand. Returning
        // an empty resolved prompt would let the launcher fall through to its
        // local-prompt path; an Err makes the (rare) callers flag the gap loudly.
        Err(anyhow::anyhow!("warp-oss: resolve_prompt not supported"))
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
        Ok(request
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| self.sentinel_target("snapshot", &format!("{i}-{}", f.filename)))
            .collect())
    }

    async fn fetch_transcript(&self) -> Result<bytes::Bytes> {
        // We cannot resolve which conversation this refers to without context the
        // trait doesn't pass through. Resume from disk happens via the harness
        // launcher reading directly from `~/.warp-oss/harness/{id}/transcript.json`.
        Err(anyhow::anyhow!(
            "warp-oss: fetch_transcript via HarnessSupportClient is unsupported; resume reads disk directly"
        ))
    }

    fn http_client(&self) -> &HttpClient {
        &self.http
    }
}

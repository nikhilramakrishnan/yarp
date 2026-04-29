// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::{
    channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig},
    features::FeatureFlag,
    AppId,
};

// Features enabled for warp-oss. We commit to OSS as the sole channel and
// replace Warp's backend with a local-first implementation, so we can enable
// every flag whose feature works without a Warp account or a cloud service.
//
// What's intentionally NOT here: ProviderCommand, ArtifactCommand,
// OzIdentityFederation, CloudEnvironments, ScheduledAmbientAgents,
// WarpManagedSecrets, CreatingSharedSessions — those surfaces would expose
// non-functional UI without a backend we control. CrossRepoContext /
// FullSourceCodeEmbedding are deferred until we wire up a local embedding
// provider.
const OSS_FLAGS: &[FeatureFlag] = &[
    // Required for harness flow
    FeatureFlag::AgentHarness,
    FeatureFlag::OrchestrationV2,
    FeatureFlag::OzHandoff,
    FeatureFlag::ConversationApi,
    // Markdown / editor
    FeatureFlag::MarkdownTables,
    FeatureFlag::BlocklistMarkdownTableRendering,
    FeatureFlag::BlocklistMarkdownImages,
    FeatureFlag::MarkdownImages,
    FeatureFlag::EditableMarkdownMermaid,
    // Code review
    FeatureFlag::GitOperationsInCodeReview,
    FeatureFlag::CodeReviewScrollPreservation,
    FeatureFlag::ContextLineReviewComments,
    FeatureFlag::FileAndDiffSetComments,
    // Tabs / window
    FeatureFlag::DirectoryTabColors,
    FeatureFlag::VerticalTabsSummaryMode,
    // Terminal UX
    FeatureFlag::RemoveAutosuggestionDuringTabCompletions,
    FeatureFlag::ResizeFix,
    FeatureFlag::LazySceneBuilding,
    FeatureFlag::ToggleBootstrapBlock,
    #[cfg(target_os = "macos")]
    FeatureFlag::ImeMarkedText,
    FeatureFlag::SshDragAndDrop,
    // Agent ergonomics
    FeatureFlag::QueueSlashCommand,
    FeatureFlag::PendingUserQueryIndicator,
    FeatureFlag::RetryTruncatedCodeResponses,
    FeatureFlag::RememberFastForwardState,
    FeatureFlag::AgentViewBlockContext,
    FeatureFlag::AgentModeWorkflows,
    // Skills
    FeatureFlag::OzPlatformSkills,
];

// Simple wrapper around warp::run() for Warp OSS builds.
fn main() -> Result<()> {
    let mut state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("dev", "yarp", "Yarp"),
            logfile_name: "yarp.log".into(),
            server_config: WarpServerConfig::production(),
            oz_config: OzConfig::production(),
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    );
    state = state.with_additional_features(OSS_FLAGS);
    if cfg!(debug_assertions) {
        state = state.with_additional_features(warp_core::features::DEBUG_FLAGS);
    }
    ChannelState::set(state);

    write_default_llm_config_if_missing();

    warp::run()
}

/// On first launch, drop a commented template at `~/.yarp/llm_provider.json`
/// so users can configure the local LLM without grepping the source for env
/// var names. Existing files are never overwritten.
fn write_default_llm_config_if_missing() {
    let Some(home) = warp_core::paths::warp_home_config_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&home);
    let path = home.join("llm_provider.json");
    if path.exists() {
        return;
    }
    let template = r#"{
  "_comment_provider":  "anthropic | openai | ollama   (omit or empty = auto-detect)",
  "_comment_api_key":   "Anthropic or OpenAI API key. Ignored for ollama.",
  "_comment_model":     "Model id. Defaults: claude-opus-4-7 / gpt-5 / qwen2.5-coder",
  "_comment_base_url":  "Override endpoint. openai: https://api.openai.com/v1   ollama: http://localhost:11434",
  "provider": "",
  "api_key": "",
  "model": "",
  "base_url": ""
}
"#;
    let _ = std::fs::write(&path, template);
}

// If we're not using an external plist, embed the following as the Info.plist.
#[cfg(all(not(feature = "extern_plist"), target_os = "macos"))]
embed_plist::embed_info_plist_bytes!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleDisplayName</key>
    <string>Yarp</string>
    <key>CFBundleExecutable</key>
    <string>warp-oss</string>
    <key>CFBundleIdentifier</key>
    <string>dev.yarp.Yarp</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Yarp</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>UIDesignRequiresCompatibility</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array><dict><key>CFBundleURLName</key><string>Yarp</string><key>CFBundleURLSchemes</key><array><string>yarp</string></array></dict></array>
    <key>NSHumanReadableCopyright</key>
    <string>© 2026 Yarp contributors</string>
    </dict>
    </plist>
"#.as_bytes());

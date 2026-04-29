//! AI Provider settings page.
//!
//! Yarp's local LLM provider is configured via `~/.yarp/llm_provider.json`
//! (created automatically on first launch) and/or `YARP_LLM_*` env vars.
//! This page surfaces the current resolved configuration and a button to
//! open the JSON file in the user's default editor.

use super::{
    settings_page::{
        MatchData, PageType, SettingsPageEvent, SettingsPageMeta, SettingsPageViewHandle,
        SettingsWidget,
    },
    SettingsSection,
};
use crate::{
    appearance::Appearance,
    server::local_backend::llm_provider::{LocalLlmProvider, StoredLlmConfig},
};
use warp_core::paths::warp_home_config_dir;
use warpui::{
    elements::{Container, CrossAxisAlignment, Element, Flex, ParentElement},
    ui_components::components::UiComponent,
    AppContext, Entity, View, ViewContext, ViewHandle,
};

pub struct AIProviderPageView {
    page: PageType<Self>,
}

impl AIProviderPageView {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self {
            page: PageType::new_monolith(AIProviderWidget::default(), None, false),
        }
    }
}

impl Entity for AIProviderPageView {
    type Event = SettingsPageEvent;
}

impl View for AIProviderPageView {
    fn ui_name() -> &'static str {
        "AIProviderPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

#[derive(Default)]
struct AIProviderWidget;

fn config_path_str() -> String {
    warp_home_config_dir()
        .map(|p| p.join("llm_provider.json").display().to_string())
        .unwrap_or_else(|| "~/.yarp/llm_provider.json".to_string())
}

fn mask_key(key: &str) -> String {
    if key.is_empty() {
        return "(unset)".to_string();
    }
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }
    let prefix: String = key.chars().take(4).collect();
    let suffix: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}…{suffix}")
}

impl SettingsWidget for AIProviderWidget {
    type View = AIProviderPageView;

    fn search_terms(&self) -> &str {
        "ai provider llm anthropic openai ollama api key"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let ui = appearance.ui_builder();

        let stored = StoredLlmConfig::load();
        let resolved = LocalLlmProvider::from_env();

        let resolved_label = match &resolved {
            LocalLlmProvider::Anthropic { model, .. } => format!("anthropic — {model}"),
            LocalLlmProvider::OpenAi {
                base_url, model, ..
            } => format!("openai — {model} @ {base_url}"),
            LocalLlmProvider::Ollama { base_url, model } => {
                format!("ollama — {model} @ {base_url}")
            }
            LocalLlmProvider::Disabled => "disabled (no provider configured)".to_string(),
        };

        let title = ui.span("AI Provider".to_string()).build().finish();

        let intro = ui
            .span(
                "Yarp's built-in AI surfaces (command suggestions, commit messages, the AI \
                 assistant panel) call the LLM provider configured below. Third-party CLI \
                 harnesses (Claude Code, OpenCode, Gemini) use their own credentials and are \
                 unaffected."
                    .to_string(),
            )
            .with_soft_wrap()
            .build()
            .with_margin_top(12.)
            .finish();

        let resolved_row = ui
            .span(format!("Active: {resolved_label}"))
            .with_soft_wrap()
            .build()
            .with_margin_top(20.)
            .finish();

        let stored_provider = if stored.provider.is_empty() {
            "(auto-detect)".to_string()
        } else {
            stored.provider.clone()
        };
        let stored_model = if stored.model.is_empty() {
            "(provider default)".to_string()
        } else {
            stored.model.clone()
        };
        let stored_base_url = if stored.base_url.is_empty() {
            "(provider default)".to_string()
        } else {
            stored.base_url.clone()
        };

        let on_disk_header = ui
            .span("On-disk config".to_string())
            .build()
            .with_margin_top(20.)
            .finish();

        let on_disk_lines = vec![
            format!("  provider:  {stored_provider}"),
            format!("  api_key:   {}", mask_key(&stored.api_key)),
            format!("  model:     {stored_model}"),
            format!("  base_url:  {stored_base_url}"),
        ];

        let mut on_disk = Flex::column().with_child(on_disk_header);
        for line in on_disk_lines {
            on_disk = on_disk.with_child(ui.span(line).build().with_margin_top(2.).finish());
        }

        let env_header = ui
            .span("Env-var overrides (set in your shell to override the file)".to_string())
            .build()
            .with_margin_top(20.)
            .finish();

        let env_lines = [
            "  YARP_LLM_PROVIDER  (anthropic | openai | ollama)",
            "  YARP_LLM_API_KEY",
            "  YARP_LLM_MODEL",
            "  YARP_LLM_BASE_URL",
        ];
        let mut env_block = Flex::column().with_child(env_header);
        for line in env_lines {
            env_block = env_block.with_child(
                ui.span(line.to_string())
                    .build()
                    .with_margin_top(2.)
                    .finish(),
            );
        }

        let path = config_path_str();
        let path_row = ui
            .span(format!("File: {path}"))
            .with_soft_wrap()
            .build()
            .with_margin_top(20.)
            .finish();

        let hint_row = ui
            .span(
                "Tip: edit the file in any editor, then restart Yarp to pick up changes."
                    .to_string(),
            )
            .with_soft_wrap()
            .build()
            .with_margin_top(8.)
            .finish();

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(title)
                .with_child(intro)
                .with_child(resolved_row)
                .with_child(on_disk.finish())
                .with_child(env_block.finish())
                .with_child(path_row)
                .with_child(hint_row)
                .finish(),
        )
        .finish()
    }
}

impl SettingsPageMeta for AIProviderPageView {
    fn section() -> SettingsSection {
        SettingsSection::AIProvider
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<AIProviderPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<AIProviderPageView>) -> Self {
        SettingsPageViewHandle::AIProvider(view_handle)
    }
}

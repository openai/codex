use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;
use codex_tools::DiscoverableTool;

// Endpoint candidates cap display names at 64 Unicode characters and slugs at 64 ASCII
// characters. With the curated marketplace suffix, 50 rows, a 500-byte introduction and
// fragment markers, the final item is at most 17,946 UTF-8 bytes. Revisit this bound if
// upstream identity limits or the marketplace suffix change; never truncate these facts.
const MAX_RECOMMENDED_PLUGINS: usize = 50;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecommendedPluginsInstructions {
    plugins: Vec<DiscoverableTool>,
    intro: String,
}

impl RecommendedPluginsInstructions {
    pub(crate) fn from_plugins(
        plugins: &[DiscoverableTool],
        messages: &codex_prompts::ResolvedPluginRecommendationMessages,
    ) -> Option<Self> {
        if plugins.is_empty() {
            return None;
        }
        Some(Self {
            intro: messages.intro.clone(),
            plugins: plugins
                .iter()
                .take(MAX_RECOMMENDED_PLUGINS)
                .cloned()
                .collect(),
        })
    }
}

impl ContextualUserFragment for RecommendedPluginsInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("plugins.recommendations".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<recommended_plugins>", "</recommended_plugins>")
    }

    fn body(&self) -> ContentItem {
        let plugins = self
            .plugins
            .iter()
            .map(|plugin| format!("- {} ({})", plugin.name(), plugin.id()))
            .collect::<Vec<_>>()
            .join("\n");
        let intro = &self.intro;
        input_text(format!("\n{intro}\n\n{plugins}\n"))
    }
}

#[cfg(test)]
#[path = "recommended_plugins_instructions_tests.rs"]
mod tests;

use super::ContextualUserFragment;
use codex_tools::DiscoverableTool;

const RECOMMENDED_PLUGINS_INTRO: &str = "Here is a list of plugins that are available but not installed. If the user's query would benefit from one of these plugins, use the `request_plugin_install` tool to suggest that they install it. All entries have `tool_type: plugin`; pass `plugin` as `tool_type` and the parenthesized ID as `tool_id`. For example, suggest the Google Drive plugin if the query could possibly be better answered with access to Google Drive.";
const MAX_RECOMMENDED_PLUGINS: usize = 50;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecommendedPluginsInstructions {
    plugins: Vec<DiscoverableTool>,
}

impl RecommendedPluginsInstructions {
    pub(crate) fn from_plugins(plugins: &[DiscoverableTool]) -> Option<Self> {
        if plugins.is_empty() {
            return None;
        }
        Some(Self {
            plugins: plugins
                .iter()
                .take(MAX_RECOMMENDED_PLUGINS)
                .cloned()
                .collect(),
        })
    }
}

impl ContextualUserFragment for RecommendedPluginsInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<recommended_plugins>", "</recommended_plugins>")
    }

    fn body(&self) -> String {
        let plugins = self
            .plugins
            .iter()
            .map(|plugin| format!("- {} ({})", plugin.name(), plugin.id()))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n{RECOMMENDED_PLUGINS_INTRO}\n\n{plugins}\n")
    }
}

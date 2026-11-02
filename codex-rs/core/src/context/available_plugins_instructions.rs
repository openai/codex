use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AvailablePluginsInstructions(pub(crate) String);

impl From<codex_prompts::ResolvedGenericPluginMessages> for AvailablePluginsInstructions {
    fn from(messages: codex_prompts::ResolvedGenericPluginMessages) -> Self {
        let intro = messages.intro;
        let usage = messages.usage_instructions;
        Self(format!(
            "\n## Plugins\n{intro}\n### How to use plugins\n{usage}\n"
        ))
    }
}

impl Default for AvailablePluginsInstructions {
    fn default() -> Self {
        Self::from(codex_prompts::ResolvedModelMessages::bundled().generic_plugin())
    }
}

impl ContextualUserFragment for AvailablePluginsInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("plugins.usage_instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            PLUGINS_INSTRUCTIONS_OPEN_TAG,
            PLUGINS_INSTRUCTIONS_CLOSE_TAG,
        )
    }

    fn body(&self) -> ContentItem {
        input_text(self.0.clone())
    }
}

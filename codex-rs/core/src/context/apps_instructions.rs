//! Renders bundled Apps guidance and catalog updates with the same developer provenance.
//! Older clients recognize only the bundled markers when recovering from retained history.

use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::protocol::APPS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::CATALOG_APPS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::CATALOG_APPS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AppsInstructions(pub(crate) String);

impl Default for AppsInstructions {
    fn default() -> Self {
        Self(
            codex_prompts::ResolvedModelMessages::bundled()
                .apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME)
                .usage_instructions,
        )
    }
}

impl ContextualUserFragment for AppsInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("apps.instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (APPS_INSTRUCTIONS_OPEN_TAG, APPS_INSTRUCTIONS_CLOSE_TAG)
    }

    fn body(&self) -> ContentItem {
        input_text(self.0.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CatalogAppsInstructions(pub(crate) String);

impl ContextualUserFragment for CatalogAppsInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("apps.instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            CATALOG_APPS_INSTRUCTIONS_OPEN_TAG,
            CATALOG_APPS_INSTRUCTIONS_CLOSE_TAG,
        )
    }

    fn body(&self) -> ContentItem {
        input_text(self.0.clone())
    }
}

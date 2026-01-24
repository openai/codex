//! Extension functions for lib.rs to minimize upstream conflicts.

use codex_core::config::Config;
use codex_core::features::Feature;
use codex_lsp::LspServerManager;
use codex_retrieval::RetrievalFacade;
use std::sync::Arc;

/// Initialize LspServerManager if Feature::Lsp is enabled.
pub fn create_lsp_manager(config: &Config) -> Option<Arc<LspServerManager>> {
    if config.features.enabled(Feature::Lsp) {
        Some(codex_lsp::create_manager(Some(config.cwd.clone())))
    } else {
        None
    }
}

/// Initialize RetrievalFacade if Feature::Retrieval is enabled.
pub async fn create_retrieval_manager(config: &Config) -> Option<Arc<RetrievalFacade>> {
    if config.features.enabled(Feature::Retrieval) {
        codex_retrieval::create_manager(Some(config.cwd.clone())).await
    } else {
        None
    }
}

use std::sync::Arc;
use std::sync::Weak;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_login::AuthManager;

mod async_scorer;
mod sync_reviewer;

pub use sync_reviewer::GuardianExtension as GuardianReviewerExtension;

/// Installs the guardian contributors into the extension registry.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    auth_manager: Arc<AuthManager>,
    thread_manager: Weak<ThreadManager>,
) {
    async_scorer::install(registry, auth_manager, thread_manager.clone());
    sync_reviewer::install(registry, thread_manager);
}

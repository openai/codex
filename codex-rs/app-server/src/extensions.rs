use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;

pub(crate) fn thread_extensions() -> Arc<ExtensionRegistry<Config>> {
    Arc::new(ExtensionRegistryBuilder::<Config>::new().build())
}

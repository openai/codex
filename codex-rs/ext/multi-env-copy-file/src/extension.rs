//! Exposes file copying independently of which environments a turn selects.

use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;

use crate::tool::CopyFileTool;

struct FileCopyExtension;

impl ToolContributor for FileCopyExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor>> {
        vec![Arc::new(CopyFileTool)]
    }
}

/// Installs file copying after the host checks its feature flag.
pub fn install<C: Sync>(registry: &mut ExtensionRegistryBuilder<C>) {
    registry.tool_contributor(Arc::new(FileCopyExtension));
}

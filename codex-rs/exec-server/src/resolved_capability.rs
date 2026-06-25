use std::fmt;
use std::sync::Arc;

use codex_protocol::capabilities::SelectedCapabilityRoot;

use crate::Environment;
use crate::ExecutorFileSystem;

/// A selected capability root pinned to the exact environment instance that owns it.
///
/// This value is process-local and must not be persisted. Cloning it keeps the same
/// [`Environment`] alive so every consumer of one model step uses the same executor.
#[derive(Clone)]
pub struct ResolvedSelectedCapabilityRoot {
    selected_root: SelectedCapabilityRoot,
    environment: Arc<Environment>,
}

impl ResolvedSelectedCapabilityRoot {
    pub fn new(selected_root: SelectedCapabilityRoot, environment: Arc<Environment>) -> Self {
        Self {
            selected_root,
            environment,
        }
    }

    pub fn selected_root(&self) -> &SelectedCapabilityRoot {
        &self.selected_root
    }

    pub fn environment(&self) -> &Arc<Environment> {
        &self.environment
    }

    pub fn file_system(&self) -> Arc<dyn ExecutorFileSystem> {
        self.environment.get_filesystem()
    }
}

impl fmt::Debug for ResolvedSelectedCapabilityRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedSelectedCapabilityRoot")
            .field("selected_root", &self.selected_root)
            .finish_non_exhaustive()
    }
}

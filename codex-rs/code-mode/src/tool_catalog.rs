//! Negotiates a per-session tool catalog at cell admission. Concurrent admissions
//! can always send a self-contained request without changing the shared catalog.

use std::sync::Arc;

use codex_code_mode_protocol::ToolDefinition;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;

#[derive(Default)]
pub(crate) struct Catalog {
    revision: u64,
    tools: Option<Vec<Arc<ToolDefinition>>>,
}

pub(crate) struct CatalogAdmission {
    cache: OwnedMutexGuard<Catalog>,
    replacement: Option<Vec<Arc<ToolDefinition>>>,
}

impl CatalogAdmission {
    /// Takes ownership of a new snapshot until the host acknowledges admission.
    /// Clearing the old snapshot first keeps cancellation from reusing a stale revision.
    pub(crate) fn prepare(cache: &Arc<Mutex<Catalog>>, tools: &[ToolDefinition]) -> Option<Self> {
        if tools.is_empty() {
            return None;
        }
        let mut cache = Arc::clone(cache).try_lock_owned().ok()?;
        let replacement = if cache.tools.as_ref().is_some_and(|cached| {
            cached.len() == tools.len() && cached.iter().map(Arc::as_ref).eq(tools)
        }) {
            None
        } else {
            cache.revision = cache.revision.checked_add(1)?;
            cache.tools = None;
            Some(ToolDefinition::shared_catalog(tools))
        };
        Some(Self { cache, replacement })
    }

    pub(crate) fn revision(&self) -> u64 {
        self.cache.revision
    }

    pub(crate) fn reuses_catalog(&self) -> bool {
        self.replacement.is_none()
    }

    pub(crate) fn accepted(mut self) {
        if let Some(tools) = self.replacement.take() {
            self.cache.tools = Some(tools);
        }
    }
}

#[cfg(test)]
#[path = "tool_catalog_tests.rs"]
mod tests;

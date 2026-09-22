//! Shares the root user's selected routing tier across the entire agent tree.

use super::LocalAgentControl;
use std::sync::Arc;

impl LocalAgentControl {
    /// Returns the latest user-selected tier for this root and all its descendants.
    pub(crate) fn root_service_tier(&self) -> Option<String> {
        self.runtime
            .root_service_tier
            .load_full()
            .map(|service_tier| (*service_tier).clone())
    }

    /// Publishes a root-owned tier without mutating individual child sessions.
    pub(crate) fn set_root_service_tier(&self, service_tier: Option<String>) {
        self.runtime
            .root_service_tier
            .store(service_tier.map(Arc::new));
    }
}

//! Hook manager for coordinating hook execution.

use std::sync::Arc;

use crate::hooks::config::HooksConfig;
use crate::hooks::registry::HookRegistry;
use crate::hooks::types::{HookError, LifecycleEvent};

/// Central manager for the lifecycle hooks system.
pub struct HookManager {
    registry: Arc<HookRegistry>,
    config: HooksConfig,
}

impl HookManager {
    /// Create a new hook manager with the given configuration.
    pub async fn new(config: HooksConfig) -> Result<Self, HookError> {
        let registry = Arc::new(HookRegistry::new(config.clone()).await?);

        Ok(Self {
            registry,
            config,
        })
    }

    /// Trigger a lifecycle event and execute all matching hooks.
    pub async fn trigger_event(&self, event: LifecycleEvent) -> Result<(), HookError> {
        if !self.config.hooks.enabled {
            return Ok(());
        }

        // TODO: Implement in Phase 2.1
        tracing::debug!("Hook execution triggered for event: {:?}", event.event_type());

        Ok(())
    }

    /// Check if hooks are enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.hooks.enabled
    }

    /// Get the hook registry.
    pub fn registry(&self) -> Arc<HookRegistry> {
        self.registry.clone()
    }
}

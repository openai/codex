//! Hook registry for managing hook definitions and routing.

use std::collections::HashMap;

use crate::hooks::config::{HooksConfig, HookConfig};
use crate::hooks::types::{HookError, LifecycleEventType};

/// Registry for managing hook definitions and event routing.
pub struct HookRegistry {
    hooks_by_event: HashMap<LifecycleEventType, Vec<HookConfig>>,
    config: HooksConfig,
}

impl HookRegistry {
    /// Create a new hook registry with the given configuration.
    pub async fn new(config: HooksConfig) -> Result<Self, HookError> {
        let hooks_by_event = HashMap::new();

        // TODO: Implement in Phase 1.2 - populate hooks_by_event from config

        Ok(Self {
            hooks_by_event,
            config,
        })
    }

    /// Get all hooks for a specific event type.
    pub fn get_hooks_for_event(&self, event_type: LifecycleEventType) -> Vec<&HookConfig> {
        self.hooks_by_event
            .get(&event_type)
            .map(|hooks| hooks.iter().collect())
            .unwrap_or_default()
    }

    /// Register a new hook.
    pub fn register_hook(&mut self, hook: HookConfig) -> Result<(), HookError> {
        hook.validate()?;

        self.hooks_by_event
            .entry(hook.event)
            .or_insert_with(Vec::new)
            .push(hook);

        Ok(())
    }
}

use codex_config::McpServerConfig;
use codex_protocol::capabilities::SelectedCapabilityRoot;

use crate::ExtensionData;
use crate::ExtensionDataInit;

/// Input supplied while resolving MCP server contributions.
///
/// Thread-scoped implementations can read stable host inputs through [`Self::thread_init`] and
/// keep their cache in [`Self::thread_store`]. Implementations should not retain borrowed context
/// after contribution completes.
pub struct McpServerContributionContext<'a, C> {
    /// Host configuration visible during MCP resolution.
    config: &'a C,
    /// Extension-owned data for the active thread, when resolution is thread-scoped.
    thread_store: Option<&'a ExtensionData>,
    /// Stable host inputs for the active thread, when resolution is thread-scoped.
    thread_init: Option<&'a ExtensionDataInit>,
    /// Effective request originator for the active thread, when resolution is thread-scoped.
    originator: Option<&'a str>,
    /// Selected roots resolved against ready environments for this exact step.
    ready_selected_capability_roots: Option<&'a [SelectedCapabilityRoot]>,
}

impl<C> Clone for McpServerContributionContext<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for McpServerContributionContext<'_, C> {}

impl<'a, C> McpServerContributionContext<'a, C> {
    /// Creates context for resolution that is not associated with a running thread.
    pub fn global(config: &'a C) -> Self {
        Self {
            config,
            thread_store: None,
            thread_init: None,
            originator: None,
            ready_selected_capability_roots: None,
        }
    }

    /// Creates context for one model step using only currently available environments.
    pub fn for_step(
        config: &'a C,
        thread_init: &'a ExtensionDataInit,
        thread_store: &'a ExtensionData,
        originator: &'a str,
        ready_selected_capability_roots: &'a [SelectedCapabilityRoot],
    ) -> Self {
        Self {
            config,
            thread_store: Some(thread_store),
            thread_init: Some(thread_init),
            originator: Some(originator),
            ready_selected_capability_roots: Some(ready_selected_capability_roots),
        }
    }

    /// Returns the host configuration visible during resolution.
    pub fn config(&self) -> &'a C {
        self.config
    }

    /// Returns extension-owned state when resolving for a running thread.
    pub fn thread_store(&self) -> Option<&'a ExtensionData> {
        self.thread_store
    }

    /// Returns stable host inputs when resolving for a running thread.
    pub fn thread_init(&self) -> Option<&'a ExtensionDataInit> {
        self.thread_init
    }

    /// Returns the effective request originator when resolving for a running thread.
    pub fn originator(&self) -> Option<&'a str> {
        self.originator
    }

    /// Returns selected roots resolved against the ready environments for this model step.
    pub fn ready_selected_capability_roots(&self) -> Option<&'a [SelectedCapabilityRoot]> {
        self.ready_selected_capability_roots
    }
}

/// One extension-owned overlay for the runtime MCP server configuration.
#[derive(Clone, Debug)]
pub enum McpServerContribution {
    /// Adds or replaces a named MCP server.
    Set {
        name: String,
        config: Box<McpServerConfig>,
    },
    /// Registers a server declared by a plugin selected for this thread.
    SelectedPlugin {
        name: String,
        plugin_id: String,
        plugin_display_name: String,
        selection_order: usize,
        config: Box<McpServerConfig>,
    },
    /// Records a plugin selected for this thread and any connector IDs it declares.
    SelectedPluginPackage {
        plugin_id: String,
        plugin_display_name: String,
        connector_ids: Vec<String>,
    },
    /// Removes a named MCP server.
    Remove { name: String },
}

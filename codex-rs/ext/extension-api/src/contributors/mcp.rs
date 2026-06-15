use codex_config::McpServerConfig;

use crate::ExtensionDataInit;

/// Input supplied while resolving MCP server contributions.
///
/// Thread-scoped implementations can read the immutable host-seeded inputs
/// through [`Self::thread_init`]. Implementations should not retain borrowed
/// context after contribution completes.
pub struct McpServerContributionContext<'a, C> {
    /// Host configuration visible during MCP resolution.
    config: &'a C,
    /// Initial inputs for the active thread, when resolution is thread-scoped.
    thread_init: Option<&'a ExtensionDataInit>,
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
            thread_init: None,
        }
    }

    /// Creates context for one active thread runtime.
    pub fn for_thread(config: &'a C, thread_init: &'a ExtensionDataInit) -> Self {
        Self {
            config,
            thread_init: Some(thread_init),
        }
    }

    /// Returns the host configuration visible during resolution.
    pub fn config(&self) -> &'a C {
        self.config
    }

    /// Returns the frozen initial inputs when resolving for a running thread.
    pub fn thread_init(&self) -> Option<&'a ExtensionDataInit> {
        self.thread_init
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
    /// Removes a named MCP server.
    Remove { name: String },
}

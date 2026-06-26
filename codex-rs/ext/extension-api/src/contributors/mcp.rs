use codex_config::McpServerConfig;
use codex_mcp::EffectiveMcpServer;

use crate::ExtensionData;
use crate::ExtensionDataInit;

/// Whether contributors may discover new external MCP server state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpServerContributionMode {
    /// Contributors may initialize or refresh external discovery.
    Discover,
    /// Contributors must project only already-published state.
    Current,
}

/// Input supplied while resolving MCP server contributions.
///
/// Thread-scoped implementations can read stable host inputs through [`Self::thread_init`]. Model
/// step implementations can keep a cache in [`Self::thread_store`]. Implementations should not
/// retain borrowed context after contribution completes.
pub struct McpServerContributionContext<'a, C> {
    /// Host configuration visible during MCP resolution.
    config: &'a C,
    /// Extension-owned data for the active thread, when resolving a model step.
    thread_store: Option<&'a ExtensionData>,
    /// Stable host inputs for the active thread, when resolution is thread-scoped.
    thread_init: Option<&'a ExtensionDataInit>,
    /// Environment IDs whose selected roots may contribute to this exact step.
    available_environment_ids: Option<&'a [String]>,
    /// Whether contributors may initialize or refresh externally discovered servers.
    mode: McpServerContributionMode,
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
            available_environment_ids: None,
            mode: McpServerContributionMode::Discover,
        }
    }

    /// Creates global context that projects only already-published server state.
    ///
    /// Contributors must not initialize or refresh external discovery while resolving this
    /// context. Config-derived contributors can return their normal contributions.
    pub fn global_current(config: &'a C) -> Self {
        Self {
            config,
            thread_store: None,
            thread_init: None,
            available_environment_ids: None,
            mode: McpServerContributionMode::Current,
        }
    }

    /// Creates context for a thread-scoped operation outside a model step.
    pub fn for_thread(config: &'a C, thread_init: &'a ExtensionDataInit) -> Self {
        Self {
            config,
            thread_store: None,
            thread_init: Some(thread_init),
            available_environment_ids: None,
            mode: McpServerContributionMode::Discover,
        }
    }

    /// Creates context for one model step using only currently available environments.
    pub fn for_step(
        config: &'a C,
        thread_init: &'a ExtensionDataInit,
        thread_store: &'a ExtensionData,
        available_environment_ids: &'a [String],
    ) -> Self {
        Self {
            config,
            thread_store: Some(thread_store),
            thread_init: Some(thread_init),
            available_environment_ids: Some(available_environment_ids),
            mode: McpServerContributionMode::Discover,
        }
    }

    /// Returns the host configuration visible during resolution.
    pub fn config(&self) -> &'a C {
        self.config
    }

    /// Returns extension-owned state when resolving a model step.
    pub fn thread_store(&self) -> Option<&'a ExtensionData> {
        self.thread_store
    }

    /// Returns stable host inputs when resolving for a running thread.
    pub fn thread_init(&self) -> Option<&'a ExtensionDataInit> {
        self.thread_init
    }

    /// Returns the exact environment availability projection for a model step.
    ///
    /// `Some` means contributors must omit selected roots whose environment ID is absent from the
    /// slice. Global resolution returns `None` because it has no thread environments.
    pub fn available_environment_ids(&self) -> Option<&'a [String]> {
        self.available_environment_ids
    }

    /// Returns how contributors should project externally discovered state.
    pub fn mode(&self) -> McpServerContributionMode {
        self.mode
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
    /// Adds or replaces a named MCP server whose runtime-only state must not be serialized.
    SetEffective {
        name: String,
        server: Box<EffectiveMcpServer>,
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

/// MCP overlays paired with the contributor revision observed before resolution began.
///
/// Capturing the revision first means a publication that races contribution leaves the host with
/// an older stored revision, so the next safe-boundary comparison deterministically rebuilds the
/// runtime.
#[derive(Clone, Debug)]
pub struct McpServerContributions {
    pub revision: u64,
    pub contributions: Vec<McpServerContribution>,
}

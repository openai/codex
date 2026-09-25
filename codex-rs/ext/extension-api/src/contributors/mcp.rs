use codex_config::McpServerConfig;
use codex_exec_server_protocol::ExecutorCapabilityDiscoverySnapshot;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::SessionSource;

use crate::ExtensionData;
use crate::ExtensionDataInit;
use crate::ExtensionFuture;

/// Input supplied while resolving MCP server contributions.
///
/// Thread-scoped implementations can read stable host inputs through [`Self::thread_init`] and
/// keep their cache in [`Self::thread_store`]. Implementations should not retain borrowed context
/// after contribution completes.
pub struct McpServerContributionContext<'a, C> {
    /// Host configuration visible during MCP resolution.
    config: &'a C,
    /// Whether pending auth differs from published auth; invalidate catalogs before projection.
    auth_changed: bool,
    /// Extension-owned data for the active thread, when resolution is thread-scoped.
    thread_store: Option<&'a ExtensionData>,
    /// Stable host inputs for the active thread, when resolution is thread-scoped.
    thread_init: Option<&'a ExtensionDataInit>,
    /// Source of the active thread, when supplied by the host runtime.
    session_source: Option<&'a SessionSource>,
    /// Effective request originator for the active thread, when resolution is thread-scoped.
    originator: Option<&'a str>,
    /// Selected roots resolved against ready environments for this exact step.
    ready_selected_capability_roots: Option<&'a [SelectedCapabilityRoot]>,
    /// Executor-materialized capability files shared by all consumers in this exact step.
    executor_capability_discovery: Option<&'a ExecutorCapabilityDiscoverySnapshot>,
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
            auth_changed: false,
            thread_store: None,
            thread_init: None,
            session_source: None,
            originator: None,
            ready_selected_capability_roots: None,
            executor_capability_discovery: None,
        }
    }

    /// Creates context for one model step using only currently available environments.
    pub fn for_step(
        config: &'a C,
        thread_init: &'a ExtensionDataInit,
        thread_store: &'a ExtensionData,
        originator: &'a str,
        ready_selected_capability_roots: &'a [SelectedCapabilityRoot],
        executor_capability_discovery: Option<&'a ExecutorCapabilityDiscoverySnapshot>,
    ) -> Self {
        Self {
            config,
            auth_changed: false,
            thread_store: Some(thread_store),
            thread_init: Some(thread_init),
            session_source: None,
            originator: Some(originator),
            ready_selected_capability_roots: Some(ready_selected_capability_roots),
            executor_capability_discovery,
        }
    }

    /// Marks whether this projection replaces the published MCP authentication.
    pub fn with_auth_changed(mut self, auth_changed: bool) -> Self {
        self.auth_changed = auth_changed;
        self
    }

    /// Returns whether auth-scoped catalogs must be invalidated before projection.
    pub fn auth_changed(&self) -> bool {
        self.auth_changed
    }

    /// Attaches the stable source of the active thread to this contribution.
    pub fn with_session_source(mut self, session_source: &'a SessionSource) -> Self {
        self.session_source = Some(session_source);
        self
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

    /// Returns the active thread's source when supplied by the host runtime.
    pub fn session_source(&self) -> Option<&'a SessionSource> {
        self.session_source
    }

    /// Returns the effective request originator when resolving for a running thread.
    pub fn originator(&self) -> Option<&'a str> {
        self.originator
    }

    /// Returns selected roots resolved against the ready environments for this model step.
    pub fn ready_selected_capability_roots(&self) -> Option<&'a [SelectedCapabilityRoot]> {
        self.ready_selected_capability_roots
    }

    /// Returns the executor-materialized capability files for this model step, when enabled.
    pub fn executor_capability_discovery(&self) -> Option<&'a ExecutorCapabilityDiscoverySnapshot> {
        self.executor_capability_discovery
    }
}

/// Selected plugin identities and executor roots whose skills must be hidden.
#[derive(Clone, Debug, Default)]
pub struct SelectedPluginSnapshot {
    pub plugins: Vec<SelectedPluginIdentity>,
    /// Selected plugin roots suppressed by the effective plugin feature policy.
    pub disabled_plugin_roots: Vec<String>,
}

/// The configured identity of a selected plugin and, when present, the root owning its skills.
#[derive(Clone, Debug)]
pub struct SelectedPluginIdentity {
    /// Hosted plugins have no executor root and cannot own skills from an executor folder.
    pub selected_root_id: Option<String>,
    pub plugin_id: String,
}

/// An executor plugin and its deferred MCP data. Callers that only need the identity can drop
/// `mcp` without loading server or connector configuration. A plugin still owns its skills when
/// it has no MCP data.
pub struct SelectedPlugin<'a> {
    pub selected_root_id: String,
    pub plugin_id: String,
    pub mcp: ExtensionFuture<'a, SelectedPluginContribution>,
}

/// MCP data attributed by the host to the plugin that declared it.
#[derive(Clone)]
pub struct SelectedPluginContribution {
    pub plugin_display_name: String,
    /// Environment that supplied the plugin, independent of where its MCP servers run.
    pub source_environment_id: String,
    pub connector_ids: Vec<String>,
    pub servers: Vec<(String, McpServerConfig)>,
}

/// One extension-owned overlay for the runtime MCP server configuration.
#[derive(Clone, Debug)]
pub enum McpServerContribution {
    /// Adds or replaces a named MCP server.
    Set {
        name: String,
        config: Box<McpServerConfig>,
    },
    /// Adds an ordinary extension-owned server with its own HTTP protocol mode.
    /// The mode applies only if this registration wins server resolution; it
    /// does not grant controller-owned Apps cache or environment authority.
    SetWithProtocolMode {
        name: String,
        config: Box<McpServerConfig>,
        protocol_mode: crate::McpProtocolMode,
    },
    /// Registers the controller-owned Apps server under its reserved name.
    HostedApps {
        config: Box<McpServerConfig>,
        /// Overrides the HTTP protocol mode, or uses the hosted Apps default when absent.
        protocol_mode: Option<crate::McpProtocolMode>,
    },
    /// Attributes Apps connectors to an account-hosted plugin. Plugins from executor folders must
    /// use `McpServerContributor::selected_plugins`, even if they only provide Apps connectors.
    HostedPluginConnectors {
        plugin_id: String,
        plugin_display_name: String,
        connector_ids: Vec<String>,
    },
    /// Removes a named MCP server.
    Remove { name: String },
}

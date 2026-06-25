use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

use codex_config::McpServerConfig;

use crate::server::EffectiveMcpServer;

/// Plugin identity retained with an MCP registration for tool attribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPluginAttribution {
    plugin_id: String,
    display_name: String,
}

impl McpPluginAttribution {
    pub fn new(plugin_id: String, display_name: String) -> Self {
        Self {
            plugin_id,
            display_name,
        }
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// The component that declared an MCP server registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpServerSource {
    /// A plugin discovered through the process-wide legacy plugin manager.
    Plugin(McpPluginAttribution),
    /// A plugin explicitly selected for this thread through a capability root.
    SelectedPlugin(McpPluginAttribution),
    Config,
    Extension {
        id: String,
    },
}

impl McpServerSource {
    fn disabled_registration_is_name_veto(&self) -> bool {
        // A selected package's policy applies to its registration, not to a higher runtime source
        // that happens to use the same logical server name.
        !matches!(self, Self::SelectedPlugin(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RegistrationPrecedence {
    Plugin(Reverse<usize>),
    SelectedPlugin(Reverse<usize>),
    Config,
    Extension(usize),
}

impl RegistrationPrecedence {
    fn tier(self) -> u8 {
        match self {
            Self::Plugin(_) => 0,
            Self::SelectedPlugin(_) => 1,
            Self::Config => 2,
            Self::Extension(_) => 3,
        }
    }
}

/// One named MCP server declaration before source resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct McpServerRegistration {
    name: String,
    source: McpServerSource,
    server: McpServerRegistrationValue,
    precedence: RegistrationPrecedence,
}

#[derive(Clone, Debug, PartialEq)]
enum McpServerRegistrationValue {
    Configured(Box<McpServerConfig>),
    Effective(EffectiveMcpServer),
}

impl McpServerRegistrationValue {
    fn config(&self) -> &McpServerConfig {
        match self {
            Self::Configured(config) => config.as_ref(),
            Self::Effective(server) => server.config(),
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        match self {
            Self::Configured(config) => config.enabled = enabled,
            Self::Effective(server) => server.set_enabled(enabled),
        }
    }
}

impl McpServerRegistration {
    pub fn from_config(name: String, config: McpServerConfig) -> Self {
        Self::new(
            name,
            McpServerSource::Config,
            McpServerRegistrationValue::Configured(Box::new(config)),
            RegistrationPrecedence::Config,
        )
    }

    pub fn from_plugin(
        name: String,
        attribution: McpPluginAttribution,
        plugin_order: usize,
        config: McpServerConfig,
    ) -> Self {
        Self::new(
            name,
            McpServerSource::Plugin(attribution),
            McpServerRegistrationValue::Configured(Box::new(config)),
            RegistrationPrecedence::Plugin(Reverse(plugin_order)),
        )
    }

    /// Registers a thread-selected plugin above discovered plugins and below config.
    pub fn from_selected_plugin(
        name: String,
        attribution: McpPluginAttribution,
        selection_order: usize,
        config: McpServerConfig,
    ) -> Self {
        Self::new(
            name,
            McpServerSource::SelectedPlugin(attribution),
            McpServerRegistrationValue::Configured(Box::new(config)),
            RegistrationPrecedence::SelectedPlugin(Reverse(selection_order)),
        )
    }

    pub fn from_extension(
        name: String,
        id: impl Into<String>,
        contribution_order: usize,
        config: McpServerConfig,
    ) -> Self {
        Self::new(
            name,
            McpServerSource::Extension { id: id.into() },
            McpServerRegistrationValue::Configured(Box::new(config)),
            RegistrationPrecedence::Extension(contribution_order),
        )
    }

    pub fn from_effective_extension(
        name: String,
        id: impl Into<String>,
        contribution_order: usize,
        server: EffectiveMcpServer,
    ) -> Self {
        Self::new(
            name,
            McpServerSource::Extension { id: id.into() },
            McpServerRegistrationValue::Effective(server),
            RegistrationPrecedence::Extension(contribution_order),
        )
    }

    fn new(
        name: String,
        source: McpServerSource,
        server: McpServerRegistrationValue,
        precedence: RegistrationPrecedence,
    ) -> Self {
        Self {
            name,
            source,
            server,
            precedence,
        }
    }
}

/// One side of an MCP server conflict, including whether it registers or
/// removes the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpServerConflictAction {
    Register(McpServerSource),
    Remove(McpServerSource),
}

/// A same-tier name collision and the final outcome after all precedence is applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServerConflict {
    pub name: String,
    pub outcome: McpServerConflictAction,
    pub contenders: Vec<McpServerConflictAction>,
}

#[derive(Clone, Debug)]
enum CatalogAction {
    Register(Box<McpServerRegistration>),
    Remove {
        name: String,
        source: McpServerSource,
        precedence: RegistrationPrecedence,
    },
}

impl CatalogAction {
    fn name(&self) -> &str {
        match self {
            Self::Register(registration) => &registration.name,
            Self::Remove { name, .. } => name,
        }
    }

    fn precedence(&self) -> RegistrationPrecedence {
        match self {
            Self::Register(registration) => registration.precedence,
            Self::Remove { precedence, .. } => *precedence,
        }
    }

    fn conflict_action(&self) -> McpServerConflictAction {
        match self {
            Self::Register(registration) => {
                McpServerConflictAction::Register(registration.source.clone())
            }
            Self::Remove { source, .. } => McpServerConflictAction::Remove(source.clone()),
        }
    }
}

/// Mutable inputs used to produce an immutable resolved catalog.
#[derive(Clone, Debug, Default)]
pub struct McpCatalogBuilder {
    actions: Vec<CatalogAction>,
    explicit_disabled_server_names: BTreeSet<String>,
    inherited_disabled_server_names: BTreeSet<String>,
}

impl McpCatalogBuilder {
    pub fn register(&mut self, registration: McpServerRegistration) {
        self.actions
            .push(CatalogAction::Register(Box::new(registration)));
    }

    /// Applies the legacy name-scoped disabled veto after source resolution.
    pub fn disable(&mut self, name: String) {
        self.explicit_disabled_server_names.insert(name);
    }

    pub fn remove_extension(
        &mut self,
        name: String,
        id: impl Into<String>,
        contribution_order: usize,
    ) {
        self.actions.push(CatalogAction::Remove {
            name,
            source: McpServerSource::Extension { id: id.into() },
            precedence: RegistrationPrecedence::Extension(contribution_order),
        });
    }

    pub fn build(mut self) -> ResolvedMcpCatalog {
        // Stable sorting makes action order the tie-breaker when precedence is equal.
        self.actions.sort_by_key(CatalogAction::precedence);

        let mut winners = BTreeMap::<String, CatalogAction>::new();
        let mut actions_by_name_and_tier = BTreeMap::<(String, u8), Vec<&CatalogAction>>::new();
        for action in &self.actions {
            winners.insert(action.name().to_string(), action.clone());
            actions_by_name_and_tier
                .entry((action.name().to_string(), action.precedence().tier()))
                .or_default()
                .push(action);
        }

        let mut conflicts = Vec::new();
        for ((name, _), actions) in actions_by_name_and_tier {
            if actions.len() < 2 {
                continue;
            }
            let Some(outcome) = winners.get(&name).map(CatalogAction::conflict_action) else {
                continue;
            };
            conflicts.push(McpServerConflict {
                name,
                outcome,
                contenders: actions
                    .into_iter()
                    .map(CatalogAction::conflict_action)
                    .collect(),
            });
        }

        let mut disabled_server_names = self.explicit_disabled_server_names.clone();
        disabled_server_names.extend(self.inherited_disabled_server_names.iter().cloned());
        let mut derived_disabled_server_names = self.inherited_disabled_server_names;
        let mut servers = BTreeMap::new();
        for (name, action) in winners {
            let CatalogAction::Register(registration) = action else {
                continue;
            };
            let mut registration = *registration;
            let persist_disabled_name = registration.source.disabled_registration_is_name_veto();
            if !registration.server.config().enabled || disabled_server_names.contains(&name) {
                registration.server.set_enabled(/*enabled*/ false);
                if persist_disabled_name {
                    // Preserve legacy disabled winners across later runtime overlays.
                    disabled_server_names.insert(name.clone());
                    derived_disabled_server_names.insert(name.clone());
                }
            }
            servers.insert(
                name,
                ResolvedMcpServer {
                    source: registration.source,
                    server: registration.server,
                },
            );
        }

        ResolvedMcpCatalog {
            actions: self.actions,
            explicit_disabled_server_names: self.explicit_disabled_server_names,
            derived_disabled_server_names,
            servers,
            conflicts,
        }
    }
}

/// A single winning MCP registration.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedMcpServer {
    source: McpServerSource,
    server: McpServerRegistrationValue,
}

impl ResolvedMcpServer {
    pub fn source(&self) -> &McpServerSource {
        &self.source
    }

    pub fn config(&self) -> &McpServerConfig {
        self.server.config()
    }

    fn configured_config(&self) -> Option<&McpServerConfig> {
        match &self.server {
            McpServerRegistrationValue::Configured(config) => Some(config.as_ref()),
            McpServerRegistrationValue::Effective(_) => None,
        }
    }

    fn effective_server(&self) -> Option<&EffectiveMcpServer> {
        match &self.server {
            McpServerRegistrationValue::Configured(_) => None,
            McpServerRegistrationValue::Effective(server) => Some(server),
        }
    }
}

/// Immutable result of MCP registration resolution.
#[derive(Clone, Debug, Default)]
pub struct ResolvedMcpCatalog {
    actions: Vec<CatalogAction>,
    explicit_disabled_server_names: BTreeSet<String>,
    derived_disabled_server_names: BTreeSet<String>,
    servers: BTreeMap<String, ResolvedMcpServer>,
    conflicts: Vec<McpServerConflict>,
}

impl ResolvedMcpCatalog {
    pub fn builder() -> McpCatalogBuilder {
        McpCatalogBuilder::default()
    }

    pub fn to_builder(&self) -> McpCatalogBuilder {
        McpCatalogBuilder {
            actions: self.actions.clone(),
            explicit_disabled_server_names: self.explicit_disabled_server_names.clone(),
            inherited_disabled_server_names: self.derived_disabled_server_names.clone(),
        }
    }

    /// Rebuilds the catalog while retaining only explicit disabled-name vetoes.
    ///
    /// Use this when inserting a source that participates in base source resolution. Disabled
    /// winners from the previous resolution are recomputed after the new source is registered.
    /// Runtime overlays should continue to use [`Self::to_builder`] so resolved vetoes persist.
    pub fn to_builder_recomputing_disabled_vetoes(&self) -> McpCatalogBuilder {
        McpCatalogBuilder {
            actions: self.actions.clone(),
            explicit_disabled_server_names: self.explicit_disabled_server_names.clone(),
            inherited_disabled_server_names: BTreeSet::new(),
        }
    }

    /// Returns the winning registration, including runtime-only servers.
    pub fn server(&self, name: &str) -> Option<&ResolvedMcpServer> {
        self.servers.get(name)
    }

    pub fn configured_servers(&self) -> HashMap<String, McpServerConfig> {
        self.servers
            .iter()
            .filter_map(|(name, server)| {
                server
                    .configured_config()
                    .map(|config| (name.clone(), config.clone()))
            })
            .collect()
    }

    pub(crate) fn effective_servers(&self) -> HashMap<String, EffectiveMcpServer> {
        self.servers
            .iter()
            .filter_map(|(name, server)| {
                server
                    .effective_server()
                    .map(|server| (name.clone(), server.clone()))
            })
            .collect()
    }

    /// Returns package attribution for each winning plugin-owned server.
    pub fn plugin_attributions_by_server_name(&self) -> HashMap<String, McpPluginAttribution> {
        self.servers
            .iter()
            .filter_map(|(name, server)| match server.source() {
                McpServerSource::Plugin(attribution)
                | McpServerSource::SelectedPlugin(attribution) => {
                    Some((name.clone(), attribution.clone()))
                }
                McpServerSource::Config | McpServerSource::Extension { .. } => None,
            })
            .collect()
    }

    /// Returns the names of winning servers supplied by thread-selected plugins.
    pub(crate) fn selected_plugin_server_names(&self) -> impl Iterator<Item = &str> {
        self.servers.iter().filter_map(|(name, server)| {
            matches!(server.source(), McpServerSource::SelectedPlugin(_)).then_some(name.as_str())
        })
    }

    pub fn conflicts(&self) -> &[McpServerConflict] {
        &self.conflicts
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;

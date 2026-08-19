use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

use codex_config::McpServerConfig;
use codex_config::McpServerDisabledReason;
use codex_config::RequirementSource;
use codex_protocol::mcp_policy::EnvironmentMcpPolicy;

use crate::CODEX_APPS_MCP_SERVER_NAME;

/// Plugin identity retained with an MCP registration for tool attribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPluginAttribution {
    plugin_id: String,
    display_name: String,
    agent_plugin: bool,
}

impl McpPluginAttribution {
    pub fn new(plugin_id: String, display_name: String) -> Self {
        Self {
            plugin_id,
            display_name,
            agent_plugin: false,
        }
    }

    pub fn agent_plugin(plugin_id: String, display_name: String) -> Self {
        Self {
            plugin_id,
            display_name,
            agent_plugin: true,
        }
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn is_agent_plugin(&self) -> bool {
        self.agent_plugin
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
    Compatibility {
        id: String,
    },
    Extension {
        id: String,
        host_owned_apps: bool,
    },
}

impl McpServerSource {
    pub fn is_agent_plugin(&self) -> bool {
        match self {
            Self::Plugin(attribution) | Self::SelectedPlugin(attribution) => {
                attribution.is_agent_plugin()
            }
            Self::Config | Self::Compatibility { .. } | Self::Extension { .. } => false,
        }
    }

    pub(crate) fn is_host_owned_apps(&self, name: &str, config: &McpServerConfig) -> bool {
        name == CODEX_APPS_MCP_SERVER_NAME
            && config.is_local_environment()
            && matches!(
                self,
                Self::Compatibility { .. }
                    | Self::Extension {
                        host_owned_apps: true,
                        ..
                    }
            )
    }

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
    Compatibility,
    Extension(usize),
}

impl RegistrationPrecedence {
    fn tier(self) -> u8 {
        match self {
            Self::Plugin(_) => 0,
            Self::SelectedPlugin(_) => 1,
            Self::Config => 2,
            Self::Compatibility => 3,
            Self::Extension(_) => 4,
        }
    }
}

/// One named MCP server declaration before source resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct McpServerRegistration {
    name: String,
    source: McpServerSource,
    config: McpServerConfig,
    precedence: RegistrationPrecedence,
}

impl McpServerRegistration {
    pub fn from_config(name: String, config: McpServerConfig) -> Self {
        Self::new(
            name,
            McpServerSource::Config,
            config,
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
            config,
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
            config,
            RegistrationPrecedence::SelectedPlugin(Reverse(selection_order)),
        )
    }

    pub fn from_compatibility(
        name: String,
        id: impl Into<String>,
        config: McpServerConfig,
    ) -> Self {
        Self::new(
            name,
            McpServerSource::Compatibility { id: id.into() },
            config,
            RegistrationPrecedence::Compatibility,
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
            McpServerSource::Extension {
                id: id.into(),
                host_owned_apps: false,
            },
            config,
            RegistrationPrecedence::Extension(contribution_order),
        )
    }

    /// Registers the controller-owned Apps server contributed by a host extension.
    pub fn from_hosted_apps(
        id: impl Into<String>,
        contribution_order: usize,
        config: McpServerConfig,
    ) -> Self {
        let host_owned_apps = config.is_local_environment();
        Self::new(
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
            McpServerSource::Extension {
                id: id.into(),
                host_owned_apps,
            },
            config,
            RegistrationPrecedence::Extension(contribution_order),
        )
    }

    fn new(
        name: String,
        source: McpServerSource,
        config: McpServerConfig,
        precedence: RegistrationPrecedence,
    ) -> Self {
        Self {
            name,
            source,
            config,
            precedence,
        }
    }
}

/// The authority available for MCP servers running in one environment.
#[derive(Clone, Copy, Debug)]
pub enum McpEnvironmentAuthority<'a> {
    /// The selected environment adds no restrictions to the controller policy.
    Unrestricted,
    /// The owner supplied the final restrictions for this environment.
    Restricted(&'a EnvironmentMcpPolicy),
    /// An explicitly selected plugin can use its executor without attaching that executor.
    SelectedPluginsOnly,
    /// The attachment is pending or failed, so its owner policy is not available.
    Unavailable,
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
    disabled_server_names: BTreeSet<String>,
}

impl McpCatalogBuilder {
    pub fn register(&mut self, registration: McpServerRegistration) {
        self.actions
            .push(CatalogAction::Register(Box::new(registration)));
    }

    /// Applies the legacy name-scoped disabled veto after source resolution.
    pub fn disable(&mut self, name: String) {
        self.disabled_server_names.insert(name);
    }

    pub fn remove_compatibility(&mut self, name: String, id: impl Into<String>) {
        self.actions.push(CatalogAction::Remove {
            name,
            source: McpServerSource::Compatibility { id: id.into() },
            precedence: RegistrationPrecedence::Compatibility,
        });
    }

    pub fn remove_extension(
        &mut self,
        name: String,
        id: impl Into<String>,
        contribution_order: usize,
    ) {
        self.actions.push(CatalogAction::Remove {
            name,
            source: McpServerSource::Extension {
                id: id.into(),
                host_owned_apps: false,
            },
            precedence: RegistrationPrecedence::Extension(contribution_order),
        });
    }

    /// Applies environment authority before resolving immutable server registrations.
    pub fn build_with_environment_authority<'a>(
        mut self,
        mut authority_for_environment: impl FnMut(&str) -> McpEnvironmentAuthority<'a>,
    ) -> ResolvedMcpCatalog {
        for action in &mut self.actions {
            let CatalogAction::Register(registration) = action else {
                continue;
            };
            // Controller-owned Apps and existing managed denials are not attachment-owned.
            if !registration.config.enabled
                || registration
                    .source
                    .is_host_owned_apps(&registration.name, &registration.config)
            {
                continue;
            }

            let allowed = match authority_for_environment(&registration.config.environment_id) {
                McpEnvironmentAuthority::Unrestricted => true,
                McpEnvironmentAuthority::SelectedPluginsOnly => {
                    matches!(&registration.source, McpServerSource::SelectedPlugin(_))
                }
                McpEnvironmentAuthority::Unavailable => false,
                McpEnvironmentAuthority::Restricted(policy) => match &registration.source {
                    McpServerSource::Config
                    | McpServerSource::Compatibility { .. }
                    | McpServerSource::Extension { .. } => {
                        policy.servers.as_ref().is_none_or(|requirements| {
                            requirements
                                .get(&registration.name)
                                .is_some_and(|requirement| {
                                    registration.config.matches_requirement(requirement)
                                })
                        })
                    }
                    McpServerSource::Plugin(attribution)
                    | McpServerSource::SelectedPlugin(attribution) => {
                        // Empty server policy denies every plugin; otherwise use package policy.
                        !policy.servers.as_ref().is_some_and(BTreeMap::is_empty)
                            && policy
                                .plugins
                                .as_ref()
                                .filter(|plugins| {
                                    plugins.values().any(|plugin| plugin.mcp_servers.is_some())
                                })
                                .is_none_or(|plugins| {
                                    plugins
                                        .get(attribution.plugin_id())
                                        .and_then(|plugin| plugin.mcp_servers.as_ref())
                                        .and_then(|requirements| {
                                            requirements.get(&registration.name)
                                        })
                                        .is_some_and(|requirement| {
                                            registration.config.matches_requirement(requirement)
                                        })
                                })
                    }
                },
            };

            if !allowed {
                registration.config.enabled = false;
                registration.config.disabled_reason = Some(McpServerDisabledReason::Requirements {
                    source: RequirementSource::Unknown,
                });
            }
        }
        self.build()
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

        let mut disabled_server_names = self.disabled_server_names;
        let servers = winners
            .into_iter()
            .filter_map(|(name, action)| match action {
                CatalogAction::Register(registration) => {
                    let mut registration = *registration;
                    let persist_disabled_name =
                        registration.source.disabled_registration_is_name_veto();
                    if !registration.config.enabled || disabled_server_names.contains(&name) {
                        registration.config.enabled = false;
                        if persist_disabled_name {
                            // Preserve legacy disabled winners across later runtime overlays.
                            disabled_server_names.insert(name.clone());
                        }
                    }
                    Some((
                        name,
                        ResolvedMcpServer {
                            source: registration.source,
                            config: registration.config,
                        },
                    ))
                }
                CatalogAction::Remove { .. } => None,
            })
            .collect();

        ResolvedMcpCatalog {
            actions: self.actions,
            disabled_server_names,
            servers,
            conflicts,
        }
    }
}

/// A single winning MCP registration.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedMcpServer {
    source: McpServerSource,
    config: McpServerConfig,
}

impl ResolvedMcpServer {
    pub fn source(&self) -> &McpServerSource {
        &self.source
    }

    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }
}

/// Immutable result of MCP registration resolution.
#[derive(Clone, Debug, Default)]
pub struct ResolvedMcpCatalog {
    actions: Vec<CatalogAction>,
    disabled_server_names: BTreeSet<String>,
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
            disabled_server_names: self.disabled_server_names.clone(),
        }
    }

    pub fn server(&self, name: &str) -> Option<&ResolvedMcpServer> {
        self.servers.get(name)
    }

    pub fn configured_servers(&self) -> HashMap<String, McpServerConfig> {
        self.servers
            .iter()
            .map(|(name, server)| (name.clone(), server.config.clone()))
            .collect()
    }

    /// Returns whether both catalogs resolve to the same winning servers and sources.
    pub fn has_same_servers(&self, other: &Self) -> bool {
        self.servers == other.servers
    }

    /// Replaces the resolved server set while preserving known server sources.
    ///
    /// Names not present in the existing catalog are treated as config-owned.
    pub fn with_materialized_servers(&self, servers: HashMap<String, McpServerConfig>) -> Self {
        let mut builder = Self::builder();
        for (name, config) in servers {
            let source = self
                .server(&name)
                .map(|server| server.source.clone())
                .unwrap_or(McpServerSource::Config);
            let precedence = match &source {
                McpServerSource::Plugin(_) => RegistrationPrecedence::Plugin(Reverse(0)),
                McpServerSource::SelectedPlugin(_) => {
                    RegistrationPrecedence::SelectedPlugin(Reverse(0))
                }
                McpServerSource::Config => RegistrationPrecedence::Config,
                McpServerSource::Compatibility { .. } => RegistrationPrecedence::Compatibility,
                McpServerSource::Extension { .. } => RegistrationPrecedence::Extension(0),
            };
            builder.register(McpServerRegistration::new(name, source, config, precedence));
        }
        builder.build()
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
                McpServerSource::Config
                | McpServerSource::Compatibility { .. }
                | McpServerSource::Extension { .. } => None,
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

//! Resolve server declarations without changing their identities. The exposure mode
//! selects which servers are available and how they are presented to the model.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerDisabledReason;
use codex_config::McpServerIdpOAuthConfig;
use codex_config::McpServerTransportConfig;
use codex_config::RequirementSource;
use codex_protocol::mcp_policy::EnvironmentMcpPolicy;
use codex_utils_path_uri::PathUri;

use crate::CODEX_APPS_MCP_SERVER_NAME;
use crate::McpProtocolMode;
use crate::McpServerKey;
use crate::server::McpCredentialPolicy;

/// Which servers to expose; their internal keys are the same in either mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpServerExposure {
    #[default]
    Legacy,
    EnvironmentQualified,
}

impl McpServerExposure {
    /// The environment selector shown alongside a server in model-visible output.
    pub fn environment_id(self, key: &McpServerKey) -> Option<&str> {
        match self {
            Self::Legacy => None,
            Self::EnvironmentQualified => key.scoped_environment_id(),
        }
    }
}

/// Plugin identity retained with an MCP registration for tool attribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPluginAttribution {
    plugin_id: String,
    display_name: String,
    agent_plugin: bool,
    host_root: Option<PathUri>,
}

impl McpPluginAttribution {
    pub fn new(plugin_id: String, display_name: String) -> Self {
        Self {
            plugin_id,
            display_name,
            agent_plugin: false,
            host_root: None,
        }
    }

    pub fn agent_plugin(plugin_id: String, display_name: String) -> Self {
        Self {
            plugin_id,
            display_name,
            agent_plugin: true,
            host_root: None,
        }
    }

    /// Records the exact host-discovered plugin root.
    pub fn with_host_root(mut self, host_root: PathUri) -> Self {
        self.host_root = Some(host_root);
        self
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

    /// Returns the host-discovered root captured with this server registration.
    pub fn host_root(&self) -> Option<&PathUri> {
        self.host_root.as_ref()
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
    key: McpServerKey,
    source: McpServerSource,
    config: McpServerConfig,
    credential_policy: McpCredentialPolicy,
    protocol_mode: Option<McpProtocolMode>,
    precedence: RegistrationPrecedence,
}

impl McpServerRegistration {
    /// Registers host-owned configuration, which may resolve credentials on the host.
    pub fn from_config(name: String, config: McpServerConfig) -> Self {
        Self::new(
            name,
            McpServerSource::Config,
            config,
            RegistrationPrecedence::Config,
            McpCredentialPolicy::HostFallbackAllowed,
        )
    }

    /// Registers executor-discovered configuration without authority to read
    /// host environment credentials.
    pub fn from_executor_config(name: String, config: McpServerConfig) -> Self {
        Self::new(
            name,
            McpServerSource::Config,
            config,
            RegistrationPrecedence::Config,
            McpCredentialPolicy::ExecutorOnly,
        )
    }

    /// Keeps this executor's server separate from same-named servers on other executors.
    pub fn with_environment_scope(mut self) -> Self {
        self.key = McpServerKey::in_environment(&self.config.environment_id, self.key.name());
        self
    }

    /// Registers a plugin discovered by the host's process-wide plugin manager.
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
            McpCredentialPolicy::HostFallbackAllowed,
        )
    }

    /// Registers a thread-selected plugin above discovered plugins and below config.
    /// HTTP credential authority comes from the selected root, not the server's execution environment.
    pub fn from_selected_plugin(
        name: String,
        attribution: McpPluginAttribution,
        selection_order: usize,
        source_environment_id: &str,
        config: McpServerConfig,
    ) -> Self {
        let credential_policy = if source_environment_id != DEFAULT_MCP_SERVER_ENVIRONMENT_ID
            && matches!(
                &config.transport,
                McpServerTransportConfig::StreamableHttp { .. }
            ) {
            McpCredentialPolicy::ExecutorOnly
        } else {
            McpCredentialPolicy::HostFallbackAllowed
        };
        Self::new(
            name,
            McpServerSource::SelectedPlugin(attribution),
            config,
            RegistrationPrecedence::SelectedPlugin(Reverse(selection_order)),
            credential_policy,
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
            McpCredentialPolicy::HostFallbackAllowed,
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
            McpCredentialPolicy::HostFallbackAllowed,
        )
    }

    /// Overrides the protocol for this registration if it wins HTTP server resolution.
    pub fn with_protocol_mode(mut self, protocol_mode: McpProtocolMode) -> Self {
        self.protocol_mode = Some(protocol_mode);
        self
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
            McpCredentialPolicy::HostFallbackAllowed,
        )
    }

    fn new(
        name: String,
        source: McpServerSource,
        config: McpServerConfig,
        precedence: RegistrationPrecedence,
        credential_policy: McpCredentialPolicy,
    ) -> Self {
        let key = match &source {
            McpServerSource::Plugin(_) | McpServerSource::SelectedPlugin(_) => {
                McpServerKey::in_environment(&config.environment_id, name)
            }
            McpServerSource::Config if credential_policy == McpCredentialPolicy::ExecutorOnly => {
                McpServerKey::in_environment(&config.environment_id, name)
            }
            McpServerSource::Config
            | McpServerSource::Compatibility { .. }
            | McpServerSource::Extension { .. } => McpServerKey::unscoped(name),
        };
        Self {
            key,
            source,
            config,
            credential_policy,
            protocol_mode: None,
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
    pub server_key: McpServerKey,
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
    fn key(&self) -> McpServerKey {
        match self {
            Self::Register(registration) => registration.key.clone(),
            Self::Remove { name, .. } => McpServerKey::unscoped(name),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Register(registration) => registration.key.name(),
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
    exposure: McpServerExposure,
    actions: Vec<CatalogAction>,
    disabled_server_names: BTreeSet<String>,
    requirements_disabled_server_names: BTreeMap<String, RequirementSource>,
    ema_idp: Option<McpServerIdpOAuthConfig>,
}

impl McpCatalogBuilder {
    pub fn set_exposure(&mut self, exposure: McpServerExposure) {
        self.exposure = exposure;
    }

    /// Enables EMA with the IdP selected from trusted configuration.
    /// Without this policy, finalization disables EMA registrations.
    pub fn enable_ema(&mut self, idp: McpServerIdpOAuthConfig) {
        self.ema_idp = Some(idp);
    }

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
                    .is_host_owned_apps(registration.key.name(), &registration.config)
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
                                .get(registration.key.name())
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
                                            requirements.get(registration.key.name())
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
        // Keep source actions unbound so later catalog revisions resolve afresh.
        for action in &mut self.actions {
            if let CatalogAction::Register(registration) = action
                && let Some(oauth) = &mut registration.config.oauth
            {
                oauth.ema_registration = None;
            }
        }
        // Stable sorting makes action order the tie-breaker when precedence is equal.
        self.actions.sort_by_key(CatalogAction::precedence);

        let mut winners = BTreeMap::<McpServerKey, CatalogAction>::new();
        let mut actions_by_key_and_tier =
            BTreeMap::<(McpServerKey, u8), Vec<&CatalogAction>>::new();
        for action in &self.actions {
            // Legacy clients see one winner per name. Group declarations together
            // without changing the identity of the server that wins.
            let key = match self.exposure {
                McpServerExposure::Legacy => McpServerKey::unscoped(action.name()),
                McpServerExposure::EnvironmentQualified => action.key(),
            };
            if self.exposure == McpServerExposure::Legacy
                && let CatalogAction::Register(registration) = action
                && matches!(registration.source, McpServerSource::Config)
                && registration.key.scoped_environment_id().is_some()
                && let Some(previous) = winners.get(&key)
            {
                // Preserve the first executor's server. Its config may replace
                // a plugin in that same environment, but not another owner's server.
                let replaces_plugin = matches!(previous,
                    CatalogAction::Register(previous)
                        if matches!(previous.source, McpServerSource::Plugin(_) | McpServerSource::SelectedPlugin(_))
                            && previous.config.environment_id == registration.config.environment_id);
                if !replaces_plugin {
                    continue;
                }
            }
            winners.insert(key.clone(), action.clone());
            actions_by_key_and_tier
                .entry((key, action.precedence().tier()))
                .or_default()
                .push(action);
        }

        // Thread-wide overrides and removals apply to every environment, but use
        // different keys. Remove any environment-specific server when a thread-wide
        // entry with the same name has equal or higher priority.
        let overridden = winners
            .iter()
            .filter_map(|(key, action)| match action {
                CatalogAction::Register(registration)
                    if key.scoped_environment_id().is_some()
                        && winners
                            .get(&McpServerKey::unscoped(registration.key.name()))
                            .is_some_and(|unscoped| {
                                unscoped.precedence() >= registration.precedence
                            }) =>
                {
                    Some(key.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for key in overridden {
            winners.remove(&key);
        }

        let mut conflicts = Vec::new();
        for ((key, _), actions) in actions_by_key_and_tier {
            if actions.len() < 2 {
                continue;
            }
            let Some(outcome) = winners
                .get(&key)
                .or_else(|| winners.get(&McpServerKey::unscoped(actions[0].name())))
                .map(CatalogAction::conflict_action)
            else {
                continue;
            };
            conflicts.push(McpServerConflict {
                server_key: key,
                outcome,
                contenders: actions
                    .into_iter()
                    .map(CatalogAction::conflict_action)
                    .collect(),
            });
        }

        let mut disabled_server_names = self.disabled_server_names;
        let mut requirements_disabled_server_names = self.requirements_disabled_server_names;
        let ema_idp = self.ema_idp;
        let servers = winners
            .into_iter()
            .filter_map(|(key, action)| match action {
                CatalogAction::Register(registration) => {
                    let mut registration = *registration;
                    let name = registration.key.name();
                    let persist_disabled_name = key.scoped_environment_id().is_none()
                        && registration.source.disabled_registration_is_name_veto()
                        && !(matches!(
                            registration.source,
                            McpServerSource::Plugin(_) | McpServerSource::SelectedPlugin(_)
                        ) && matches!(
                            registration.config.auth,
                            codex_config::McpServerAuth::EmaAuth
                        ));
                    if !registration.config.enabled && persist_disabled_name {
                        match &registration.config.disabled_reason {
                            Some(
                                McpServerDisabledReason::Requirements { source }
                                | McpServerDisabledReason::ExplicitAndRequirements { source },
                            ) => {
                                requirements_disabled_server_names
                                    .insert(name.to_owned(), source.clone());
                                if matches!(
                                    registration.config.disabled_reason,
                                    Some(McpServerDisabledReason::ExplicitAndRequirements { .. })
                                ) {
                                    disabled_server_names.insert(name.to_owned());
                                }
                            }
                            Some(McpServerDisabledReason::Unknown) | None => {
                                disabled_server_names.insert(name.to_owned());
                            }
                        }
                    }
                    if disabled_server_names.contains(name) {
                        registration.config.enabled = false;
                    }
                    // Only trusted Apps may ignore a previous registration's requirements denial.
                    // Retain it so a later custom replacement is still blocked.
                    if !registration
                        .source
                        .is_host_owned_apps(name, &registration.config)
                        && let Some(source) = requirements_disabled_server_names.get(name)
                    {
                        registration.config.disable_by_requirements(source.clone());
                    }
                    if matches!(
                        registration.config.auth,
                        codex_config::McpServerAuth::EmaAuth
                    ) {
                        let allowed = matches!(&registration.source, McpServerSource::Config)
                            && ema_idp.as_ref().is_some_and(|idp| {
                                registration.config.resolve_ema_registration(idp).is_ok()
                            });
                        // EMA denial must not become a persistent name veto.
                        registration.config.enabled &= allowed;
                    }
                    Some((
                        registration.key.clone(),
                        ResolvedMcpServer {
                            key: registration.key,
                            source: registration.source,
                            config: registration.config,
                            credential_policy: registration.credential_policy,
                            protocol_mode: registration.protocol_mode,
                        },
                    ))
                }
                CatalogAction::Remove { .. } => None,
            })
            .collect();

        ResolvedMcpCatalog {
            exposure: self.exposure,
            actions: self.actions,
            disabled_server_names,
            requirements_disabled_server_names,
            ema_idp,
            servers,
            conflicts,
        }
    }
}

/// A single winning MCP registration.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedMcpServer {
    key: McpServerKey,
    source: McpServerSource,
    config: McpServerConfig,
    credential_policy: McpCredentialPolicy,
    protocol_mode: Option<McpProtocolMode>,
}

impl ResolvedMcpServer {
    pub fn name(&self) -> &str {
        self.key.name()
    }

    pub fn key(&self) -> &McpServerKey {
        &self.key
    }

    pub fn source(&self) -> &McpServerSource {
        &self.source
    }

    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    pub(crate) fn credential_policy(&self) -> McpCredentialPolicy {
        self.credential_policy
    }

    pub fn protocol_mode(&self) -> Option<McpProtocolMode> {
        self.protocol_mode
    }
}

/// Immutable result of MCP registration resolution.
#[derive(Clone, Debug, Default)]
pub struct ResolvedMcpCatalog {
    exposure: McpServerExposure,
    actions: Vec<CatalogAction>,
    disabled_server_names: BTreeSet<String>,
    requirements_disabled_server_names: BTreeMap<String, RequirementSource>,
    ema_idp: Option<McpServerIdpOAuthConfig>,
    servers: BTreeMap<McpServerKey, ResolvedMcpServer>,
    conflicts: Vec<McpServerConflict>,
}

impl ResolvedMcpCatalog {
    pub fn exposure(&self) -> McpServerExposure {
        self.exposure
    }

    /// Projects an internal key onto the original name-only API, omitting shadowed peers.
    pub(crate) fn legacy_server_name<'a>(&'a self, key: &'a McpServerKey) -> Option<&'a str> {
        match self.resolve_server_name(key.name()) {
            Some(resolved_key) => (resolved_key == key).then_some(resolved_key.name()),
            None => key.scoped_environment_id().is_none().then_some(key.name()),
        }
    }

    pub fn builder() -> McpCatalogBuilder {
        McpCatalogBuilder::default()
    }

    pub fn to_builder(&self) -> McpCatalogBuilder {
        McpCatalogBuilder {
            exposure: self.exposure,
            actions: self.actions.clone(),
            disabled_server_names: self.disabled_server_names.clone(),
            requirements_disabled_server_names: self.requirements_disabled_server_names.clone(),
            ema_idp: self.ema_idp.clone(),
        }
    }

    /// Looks up one exact registration without falling back to a same-named peer.
    pub fn server(&self, key: &McpServerKey) -> Option<&ResolvedMcpServer> {
        self.servers.get(key)
    }

    /// Resolves a legacy name-only request to its winning registration.
    pub fn server_by_name(&self, name: &str) -> Option<&ResolvedMcpServer> {
        self.server(self.resolve_server_name(name)?)
    }

    /// Resolves a legacy name-only request to its original first winning registration.
    pub fn resolve_server_name(&self, name: &str) -> Option<&McpServerKey> {
        if let Some((key, _)) = self.servers.get_key_value(&McpServerKey::unscoped(name)) {
            return Some(key);
        }
        self.actions.iter().find_map(|action| {
            (action.name() == name)
                .then(|| self.servers.get_key_value(&action.key()))
                .flatten()
                .map(|(key, _)| key)
        })
    }

    /// Resolves a client target, preserving name-only routing when the scope is omitted.
    /// The operation using this key checks whether the registration exists.
    pub fn resolve_server_key(&self, name: &str, environment_id: Option<&str>) -> McpServerKey {
        match environment_id {
            Some(environment_id) => McpServerKey::in_environment(environment_id, name),
            None => self
                .resolve_server_name(name)
                .cloned()
                .unwrap_or_else(|| McpServerKey::unscoped(name)),
        }
    }

    /// Iterates every resolved registration, including same-named scoped peers.
    pub fn servers(&self) -> impl Iterator<Item = (&McpServerKey, &ResolvedMcpServer)> {
        self.servers.iter()
    }

    pub fn configured_servers(&self) -> HashMap<String, McpServerConfig> {
        self.servers()
            .filter_map(|(key, server)| {
                self.legacy_server_name(key)
                    .map(|name| (name.to_owned(), server.config.clone()))
            })
            .collect()
    }

    /// Returns whether both catalogs have the same winning servers, exposure, sources, and EMA policy.
    pub fn has_same_servers(&self, other: &Self) -> bool {
        self.exposure == other.exposure
            && self.servers == other.servers
            && self.ema_idp == other.ema_idp
    }

    /// Replaces the resolved server set while preserving known sources and EMA policy.
    ///
    /// # Panics
    ///
    /// Panics if a materialized server is missing from the catalog.
    pub fn with_materialized_servers(&self, servers: HashMap<String, McpServerConfig>) -> Self {
        let mut builder = McpCatalogBuilder {
            exposure: self.exposure,
            disabled_server_names: self.disabled_server_names.clone(),
            requirements_disabled_server_names: self.requirements_disabled_server_names.clone(),
            ema_idp: self.ema_idp.clone(),
            ..Default::default()
        };
        for (name, config) in servers {
            #[expect(
                clippy::expect_used,
                reason = "materialized servers must have catalog registrations"
            )]
            let previous = self
                .server_by_name(&name)
                .expect("materialized MCP server must have a catalog registration");
            let source = previous.source.clone();
            let precedence = match &source {
                McpServerSource::Plugin(_) => RegistrationPrecedence::Plugin(Reverse(0)),
                McpServerSource::SelectedPlugin(_) => {
                    RegistrationPrecedence::SelectedPlugin(Reverse(0))
                }
                McpServerSource::Config => RegistrationPrecedence::Config,
                McpServerSource::Compatibility { .. } => RegistrationPrecedence::Compatibility,
                McpServerSource::Extension { .. } => RegistrationPrecedence::Extension(0),
            };
            let credential_policy = previous.credential_policy();
            let mut registration = McpServerRegistration::new(
                previous.name().to_owned(),
                source,
                config,
                precedence,
                credential_policy,
            );
            if previous.key.scoped_environment_id().is_some() {
                registration = registration.with_environment_scope();
            }
            registration.protocol_mode = previous.protocol_mode();
            builder.register(registration);
        }
        builder.build()
    }

    /// Returns package attribution for each winning plugin-owned server.
    pub fn plugin_attributions_by_server(&self) -> HashMap<McpServerKey, McpPluginAttribution> {
        self.servers
            .iter()
            .filter_map(|(key, server)| match server.source() {
                McpServerSource::Plugin(attribution)
                | McpServerSource::SelectedPlugin(attribution) => {
                    Some((key.clone(), attribution.clone()))
                }
                McpServerSource::Config
                | McpServerSource::Compatibility { .. }
                | McpServerSource::Extension { .. } => None,
            })
            .collect()
    }

    /// Returns the keys of winning servers supplied by thread-selected plugins.
    pub(crate) fn selected_plugin_server_keys(&self) -> impl Iterator<Item = &McpServerKey> {
        self.servers.iter().filter_map(|(key, server)| {
            matches!(server.source(), McpServerSource::SelectedPlugin(_)).then_some(key)
        })
    }

    pub fn conflicts(&self) -> &[McpServerConflict] {
        &self.conflicts
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;

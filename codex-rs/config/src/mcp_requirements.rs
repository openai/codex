use crate::ConfigRequirements;
use crate::Constrained;
use crate::ConstraintResult;
use crate::McpServerIdentity;
use crate::McpServerRequirement;
use crate::PluginRequirementsToml;
use crate::RequirementSource;
use crate::Sourced;
use crate::mcp_types::McpServerConfig;
use crate::mcp_types::McpServerDisabledReason;
use crate::mcp_types::McpServerTransportConfig;
use regex_lite::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// String matching operations available to managed MCP server matchers.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "match", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpServerValueMatcher {
    Exact { value: String },
    Prefix { value: String },
    Regex { expression: String },
}

impl McpServerValueMatcher {
    fn compile_full_regex(expression: &str) -> Result<Regex, String> {
        Regex::new(&format!(r"\A(?:{expression})\z"))
            .map_err(|err| format!("invalid regex `{expression}`: {err}"))
    }

    fn validate(&self) -> Result<(), String> {
        let Self::Regex { expression } = self else {
            return Ok(());
        };
        Self::compile_full_regex(expression).map(|_| ())
    }

    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact { value } => candidate == value,
            Self::Prefix { value } => candidate.starts_with(value),
            Self::Regex { expression } => Self::compile_full_regex(expression)
                .ok()
                .is_some_and(|regex| regex.is_match(candidate)),
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpServerCommandMatcher {
    pub command: String,
    pub args: Vec<McpServerValueMatcher>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpServerUrlMatcher {
    pub url: McpServerValueMatcher,
}

/// A managed matcher for either a stdio command invocation or a direct MCP URL.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum McpServerMatcher {
    Command(McpServerCommandMatcher),
    Url(McpServerUrlMatcher),
}

impl McpServerMatcher {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Command(matcher) => {
                for (index, arg) in matcher.args.iter().enumerate() {
                    arg.validate().map_err(|err| {
                        format!("invalid argument matcher at index {index}: {err}")
                    })?;
                }
                Ok(())
            }
            Self::Url(matcher) => matcher.url.validate(),
        }
    }

    pub fn matches(&self, server: &McpServerConfig) -> bool {
        match (self, &server.transport) {
            (Self::Command(matcher), McpServerTransportConfig::Stdio { command, args, .. }) => {
                matcher.command == *command
                    && matcher.args.len() == args.len()
                    && matcher
                        .args
                        .iter()
                        .zip(args)
                        .all(|(matcher, arg)| matcher.matches(arg))
            }
            (Self::Url(matcher), McpServerTransportConfig::StreamableHttp { url, .. }) => {
                matcher.url.matches(url)
            }
            (Self::Command(_), McpServerTransportConfig::StreamableHttp { .. })
            | (Self::Url(_), McpServerTransportConfig::Stdio { .. }) => false,
        }
    }
}

/// Owned policy for applying exact and matcher requirements to configured MCP servers.
#[derive(Debug, Clone)]
pub struct McpServerPolicy {
    exact_requirements: Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
    matchers: Option<Sourced<BTreeMap<String, McpServerMatcher>>>,
}

impl McpServerPolicy {
    fn new(
        exact_requirements: Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
        matchers: Option<Sourced<BTreeMap<String, McpServerMatcher>>>,
    ) -> Self {
        Self {
            exact_requirements,
            matchers,
        }
    }

    /// Constrains configured MCP servers using the exact and matcher policies.
    pub fn constrain(
        self,
        mcp_servers: HashMap<String, McpServerConfig>,
    ) -> ConstraintResult<Constrained<HashMap<String, McpServerConfig>>> {
        if self.exact_requirements.is_none() && self.matchers.is_none() {
            return Ok(Constrained::allow_any(mcp_servers));
        }

        Constrained::normalized(mcp_servers, move |mut servers| {
            self.apply_to_configured_servers(&mut servers);
            servers
        })
    }

    fn apply_to_configured_servers(&self, mcp_servers: &mut HashMap<String, McpServerConfig>) {
        let source = RequirementSource::composite(
            self.exact_requirements
                .iter()
                .map(|requirements| requirements.source.clone())
                .chain(
                    self.matchers
                        .iter()
                        .map(|requirements| requirements.source.clone()),
                ),
        );
        for (name, server) in mcp_servers {
            let exact_requirement = self
                .exact_requirements
                .as_ref()
                .and_then(|requirements| requirements.value.get(name));
            let matcher = self
                .matchers
                .as_ref()
                .and_then(|matchers| matchers.value.get(name));
            let allowed = (exact_requirement.is_some() || matcher.is_some())
                && exact_requirement
                    .is_none_or(|requirement| mcp_server_matches_requirement(requirement, server))
                && matcher.is_none_or(|matcher| matcher.matches(server));
            apply_requirement_result(server, allowed, &source);
        }
    }
}

impl ConfigRequirements {
    /// Returns the owned policy used to constrain ordinary configured MCP servers.
    pub fn mcp_server_policy(&self) -> McpServerPolicy {
        McpServerPolicy::new(self.mcp_servers.clone(), self.mcp_server_matchers.clone())
    }

    /// Applies managed MCP requirements to servers supplied by one plugin.
    pub fn apply_to_plugin_mcp_servers(
        &self,
        plugin_config_name: &str,
        mcp_servers: &mut HashMap<String, McpServerConfig>,
    ) {
        apply_plugin_requirements(
            plugin_config_name,
            mcp_servers,
            self.plugins.as_ref(),
            self.mcp_server_matchers.as_ref(),
        );

        if let Some(empty_allowlist) = self
            .mcp_servers
            .as_ref()
            .filter(|requirements| requirements.value.is_empty())
        {
            for server in mcp_servers.values_mut() {
                apply_requirement_result(server, /*allowed*/ false, &empty_allowlist.source);
            }
        }
    }
}

fn apply_plugin_requirements(
    plugin_config_name: &str,
    mcp_servers: &mut HashMap<String, McpServerConfig>,
    plugin_requirements: Option<&Sourced<BTreeMap<String, PluginRequirementsToml>>>,
    matchers: Option<&Sourced<BTreeMap<String, McpServerMatcher>>>,
) {
    if plugin_requirements.is_none() && matchers.is_none() {
        return;
    }

    let source = RequirementSource::composite(
        plugin_requirements
            .iter()
            .map(|requirements| requirements.source.clone())
            .chain(
                matchers
                    .iter()
                    .map(|requirements| requirements.source.clone()),
            ),
    );
    let plugin_mcp_requirements = plugin_requirements
        .and_then(|requirements| requirements.value.get(plugin_config_name))
        .and_then(|plugin| plugin.mcp_servers.as_ref());

    for (name, server) in mcp_servers {
        let allowed_by_plugin_requirement = plugin_requirements.is_none()
            || plugin_mcp_requirements
                .and_then(|requirements| requirements.get(name))
                .is_some_and(|requirement| mcp_server_matches_requirement(requirement, server));
        let allowed_by_matcher = matchers.is_none_or(|matchers| {
            matchers
                .value
                .get(name)
                .is_some_and(|matcher| matcher.matches(server))
        });
        apply_requirement_result(
            server,
            allowed_by_plugin_requirement && allowed_by_matcher,
            &source,
        );
    }
}

fn apply_requirement_result(
    server: &mut McpServerConfig,
    allowed: bool,
    source: &RequirementSource,
) {
    if allowed {
        server.disabled_reason = None;
    } else {
        server.enabled = false;
        server.disabled_reason = Some(McpServerDisabledReason::Requirements {
            source: source.clone(),
        });
    }
}

fn mcp_server_matches_requirement(
    requirement: &McpServerRequirement,
    server: &McpServerConfig,
) -> bool {
    match &requirement.identity {
        McpServerIdentity::Command {
            command: want_command,
        } => matches!(
            &server.transport,
            McpServerTransportConfig::Stdio { command: got_command, .. }
                if got_command == want_command
        ),
        McpServerIdentity::Url { url: want_url } => matches!(
            &server.transport,
            McpServerTransportConfig::StreamableHttp { url: got_url, .. }
                if got_url == want_url
        ),
    }
}

#[cfg(test)]
#[path = "mcp_requirements_tests.rs"]
mod tests;

use crate::mcp_types::McpServerConfig;
use crate::mcp_types::McpServerTransportConfig;
use regex_lite::Regex;
use serde::Deserialize;

/// String matching operations available to managed MCP server matchers.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "match", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpServerValueMatcher {
    Exact { value: String },
    Prefix { value: String },
    Regex { expression: String },
}

impl McpServerValueMatcher {
    fn validate(&self) -> Result<(), String> {
        let Self::Regex { expression } = self else {
            return Ok(());
        };
        Regex::new(expression)
            .map(|_| ())
            .map_err(|err| format!("invalid regex `{expression}`: {err}"))
    }

    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact { value } => candidate == value,
            Self::Prefix { value } => candidate.starts_with(value),
            Self::Regex { expression } => Regex::new(expression)
                .ok()
                .and_then(|regex| regex.find(candidate))
                .is_some_and(|matched| matched.start() == 0 && matched.end() == candidate.len()),
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

#[cfg(test)]
#[path = "mcp_requirements_tests.rs"]
mod tests;

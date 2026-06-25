use crate::mcp_types::McpServerConfig;
use crate::mcp_types::McpServerTransportConfig;
use regex_lite::Regex;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum McpServerIdentity {
    Command { command: String },
    Url { url: String },
}

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
        Regex::new(&format!(r"\A(?:{expression})\z")).map_err(|err| {
            format!("regex `{expression}` cannot be used for full-value matching: {err}")
        })
    }

    fn validate(&self) -> Result<(), String> {
        let Self::Regex { expression } = self else {
            return Ok(());
        };

        Regex::new(expression).map_err(|err| format!("invalid regex `{expression}`: {err}"))?;
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
    pub executable: String,
    pub args: Vec<McpServerValueMatcher>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawMcpServerCommandIdentity {
    command: McpServerCommandMatcher,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawMcpServerUrlIdentity {
    url: McpServerValueMatcher,
}

/// A requirement for one named MCP server.
///
/// The `Identity` variant preserves the released exact-match contract. The
/// command and URL variants are the normalized matcher-based forms accepted
/// under the `identity` key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerRequirement {
    Identity { identity: McpServerIdentity },
    Command(McpServerCommandMatcher),
    Url(McpServerValueMatcher),
}

#[derive(Deserialize)]
struct RawMcpServerRequirement {
    identity: RawMcpServerIdentity,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawMcpServerIdentity {
    Exact(McpServerIdentity),
    Command(RawMcpServerCommandIdentity),
    Url(RawMcpServerUrlIdentity),
}

impl<'de> Deserialize<'de> for McpServerRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let RawMcpServerRequirement { identity } =
            RawMcpServerRequirement::deserialize(deserializer)?;
        match identity {
            RawMcpServerIdentity::Exact(identity) => Ok(Self::Identity { identity }),
            RawMcpServerIdentity::Command(matcher) => Ok(Self::Command(matcher.command)),
            RawMcpServerIdentity::Url(matcher) => Ok(Self::Url(matcher.url)),
        }
    }
}

impl McpServerRequirement {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Identity { .. } => Ok(()),
            Self::Command(matcher) => {
                for (index, arg) in matcher.args.iter().enumerate() {
                    arg.validate().map_err(|err| {
                        format!("invalid argument matcher at index {index}: {err}")
                    })?;
                }
                Ok(())
            }
            Self::Url(matcher) => matcher.validate(),
        }
    }

    pub fn matches(&self, server: &McpServerConfig) -> bool {
        match (self, &server.transport) {
            (
                Self::Identity {
                    identity:
                        McpServerIdentity::Command {
                            command: want_command,
                        },
                },
                McpServerTransportConfig::Stdio {
                    command: got_command,
                    ..
                },
            ) => got_command == want_command,
            (
                Self::Identity {
                    identity: McpServerIdentity::Url { url: want_url },
                },
                McpServerTransportConfig::StreamableHttp { url: got_url, .. },
            ) => got_url == want_url,
            (Self::Command(matcher), McpServerTransportConfig::Stdio { command, args, .. }) => {
                matcher.executable == *command
                    && matcher.args.len() == args.len()
                    && matcher
                        .args
                        .iter()
                        .zip(args)
                        .all(|(matcher, arg)| matcher.matches(arg))
            }
            (Self::Url(matcher), McpServerTransportConfig::StreamableHttp { url, .. }) => {
                matcher.matches(url)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "mcp_requirements_tests.rs"]
mod tests;

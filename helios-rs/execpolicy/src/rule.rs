use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use shlex::try_join;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// Matches a single command token, either a fixed string or one of several allowed alternatives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternToken {
    Single(String),
    Alts(Vec<String>),
}

impl PatternToken {
    fn matches(&self, token: &str) -> bool {
        match self {
            Self::Single(expected) => expected == token,
            Self::Alts(alternatives) => alternatives.iter().any(|alt| alt == token),
        }
    }

    pub fn alternatives(&self) -> &[String] {
        match self {
            Self::Single(expected) => std::slice::from_ref(expected),
            Self::Alts(alternatives) => alternatives,
        }
    }
}

/// Prefix matcher for commands with support for alternative match tokens.
/// First token is fixed since we key by the first token in policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixPattern {
    pub first: Arc<str>,
    pub rest: Arc<[PatternToken]>,
}

impl PrefixPattern {
    pub fn matches_prefix(&self, cmd: &[String]) -> Option<Vec<String>> {
        let pattern_length = self.rest.len() + 1;
        if cmd.len() < pattern_length || cmd[0] != self.first.as_ref() {
            return None;
        }

        for (pattern_token, cmd_token) in self.rest.iter().zip(&cmd[1..pattern_length]) {
            if !pattern_token.matches(cmd_token) {
                return None;
            }
        }

        Some(cmd[..pattern_length].to_vec())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleMatch {
    PrefixRuleMatch {
        #[serde(rename = "matchedPrefix")]
        matched_prefix: Vec<String>,
        decision: Decision,
        /// Optional rationale for why this rule exists.
        ///
        /// This can be supplied for any decision and may be surfaced in different contexts
        /// (e.g., prompt reasons or rejection messages).
        #[serde(skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    HeuristicsRuleMatch {
        command: Vec<String>,
        decision: Decision,
    },
}

impl RuleMatch {
    pub fn decision(&self) -> Decision {
        match self {
            Self::PrefixRuleMatch { decision, .. } => *decision,
            Self::HeuristicsRuleMatch { decision, .. } => *decision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixRule {
    pub pattern: PrefixPattern,
    pub decision: Decision,
    pub justification: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkRuleProtocol {
    Http,
    Https,
    Socks5Tcp,
    Socks5Udp,
}

impl NetworkRuleProtocol {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "http" => Ok(Self::Http),
            "https" | "https_connect" | "http-connect" => Ok(Self::Https),
            "socks5_tcp" => Ok(Self::Socks5Tcp),
            "socks5_udp" => Ok(Self::Socks5Udp),
            other => Err(Error::InvalidRule(format!(
                "network_rule protocol must be one of http, https, socks5_tcp, socks5_udp (got {other})"
            ))),
        }
    }

    pub fn as_policy_string(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks5Tcp => "socks5_tcp",
            Self::Socks5Udp => "socks5_udp",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRule {
    pub host: String,
    pub protocol: NetworkRuleProtocol,
    pub decision: Decision,
    pub justification: Option<String>,
}

pub(crate) fn normalize_network_rule_host(raw: &str) -> Result<String> {
    let mut host = raw.trim();
    if host.is_empty() {
        return Err(Error::InvalidRule(
            "network_rule host cannot be empty".to_string(),
        ));
    }
    if host.contains("://") || host.contains('/') || host.contains('?') || host.contains('#') {
        return Err(Error::InvalidRule(
            "network_rule host must be a hostname or IP literal (without scheme or path)"
                .to_string(),
        ));
    }

    if let Some(stripped) = host.strip_prefix('[') {
        let Some((inside, rest)) = stripped.split_once(']') else {
            return Err(Error::InvalidRule(
                "network_rule host has an invalid bracketed IPv6 literal".to_string(),
            ));
        };
        let port_ok = rest
            .strip_prefix(':')
            .is_some_and(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()));
        if !rest.is_empty() && !port_ok {
            return Err(Error::InvalidRule(format!(
                "network_rule host contains an unsupported suffix: {raw}"
            )));
        }
        host = inside;
    } else if host.matches(':').count() == 1
        && let Some((candidate, port)) = host.rsplit_once(':')
        && !candidate.is_empty()
        && !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
    {
        host = candidate;
    }

    let normalized = host.trim_end_matches('.').trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(Error::InvalidRule(
            "network_rule host cannot be empty".to_string(),
        ));
    }
    if normalized.contains('*') {
        return Err(Error::InvalidRule(
            "network_rule host must be a specific host; wildcards are not allowed".to_string(),
        ));
    }
    if normalized.chars().any(char::is_whitespace) {
        return Err(Error::InvalidRule(
            "network_rule host cannot contain whitespace".to_string(),
        ));
    }

    Ok(normalized)
}

pub trait Rule: Any + Debug + Send + Sync {
    fn program(&self) -> &str;

    fn matches(&self, cmd: &[String]) -> Option<RuleMatch>;

    fn as_any(&self) -> &dyn Any;
}

pub type RuleRef = Arc<dyn Rule>;

impl Rule for PrefixRule {
    fn program(&self) -> &str {
        self.pattern.first.as_ref()
    }

    fn matches(&self, cmd: &[String]) -> Option<RuleMatch> {
        self.pattern
            .matches_prefix(cmd)
            .map(|matched_prefix| RuleMatch::PrefixRuleMatch {
                matched_prefix,
                decision: self.decision,
                justification: self.justification.clone(),
            })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Count how many rules match each provided example and error if any example is unmatched.
pub(crate) fn validate_match_examples(rules: &[RuleRef], matches: &[Vec<String>]) -> Result<()> {
    let mut unmatched_examples = Vec::new();

    for example in matches {
        if rules.iter().any(|rule| rule.matches(example).is_some()) {
            continue;
        }

        unmatched_examples.push(
            try_join(example.iter().map(String::as_str))
                .unwrap_or_else(|_| "unable to render example".to_string()),
        );
    }

    if unmatched_examples.is_empty() {
        Ok(())
    } else {
        Err(Error::ExampleDidNotMatch {
            rules: rules.iter().map(|rule| format!("{rule:?}")).collect(),
            examples: unmatched_examples,
        })
    }
}

/// Ensure that no rule matches any provided negative example.
pub(crate) fn validate_not_match_examples(
    rules: &[RuleRef],
    not_matches: &[Vec<String>],
) -> Result<()> {
    for example in not_matches {
        if let Some(rule) = rules.iter().find(|rule| rule.matches(example).is_some()) {
            return Err(Error::ExampleDidMatch {
                rule: format!("{rule:?}"),
                example: try_join(example.iter().map(String::as_str))
                    .unwrap_or_else(|_| "unable to render example".to_string()),
            });
        }
    }

    Ok(())
}

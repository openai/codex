use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use shlex::try_join;
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
        matched_prefix: Vec<String>,
        decision: Decision,
    },
}

impl RuleMatch {
    pub fn decision(&self) -> Decision {
        match self {
            Self::PrefixRuleMatch { decision, .. } => *decision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixRule {
    pub pattern: PrefixPattern,
    pub decision: Decision,
}

impl PrefixRule {
    pub fn matches(&self, cmd: &[String]) -> Option<RuleMatch> {
        self.pattern
            .matches_prefix(cmd)
            .map(|matched_prefix| RuleMatch::PrefixRuleMatch {
                matched_prefix,
                decision: self.decision,
            })
    }

    pub fn validate_matches(&self, matches: &[Vec<String>]) -> Vec<bool> {
        matches
            .iter()
            .map(|example| self.matches(example).is_some())
            .collect()
    }

    pub fn validate_not_matches(&self, not_matches: &[Vec<String>]) -> Result<()> {
        for example in not_matches {
            if self.matches(example).is_some() {
                return Err(Error::ExampleDidMatch {
                    rule: format!("{self:?}"),
                    example: try_join(example.iter().map(String::as_str))
                        .unwrap_or_else(|_| "unable to render example".to_string()),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rule {
    Prefix(PrefixRule),
}

impl Rule {
    pub fn program(&self) -> &str {
        match self {
            Self::Prefix(rule) => rule.pattern.first.as_ref(),
        }
    }

    pub fn matches(&self, cmd: &[String]) -> Option<RuleMatch> {
        match self {
            Self::Prefix(rule) => rule.matches(cmd),
        }
    }

    /// Return a boolean for each example indicating whether this rule matches it.
    pub fn validate_matches(&self, matches: &[Vec<String>]) -> Vec<bool> {
        match self {
            Self::Prefix(rule) => rule.validate_matches(matches),
        }
    }

    pub fn validate_not_matches(&self, not_matches: &[Vec<String>]) -> Result<()> {
        match self {
            Self::Prefix(rule) => rule.validate_not_matches(not_matches),
        }
    }
}

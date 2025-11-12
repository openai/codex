use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use shlex::try_join;

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

    pub fn alternatives(&self) -> Vec<String> {
        match self {
            Self::Single(expected) => vec![expected.clone()],
            Self::Alts(alternatives) => alternatives.clone(),
        }
    }
}

/// Prefix matcher for commands with support for alternative match tokens.
/// First token is fixed since we key by the first token in policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixPattern {
    pub first: String,
    pub rest: Vec<PatternToken>,
}

impl PrefixPattern {
    pub fn len(&self) -> usize {
        self.rest.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn matches_prefix(&self, cmd: &[String]) -> Option<Vec<String>> {
        if cmd.len() < self.len() || cmd[0] != self.first {
            return None;
        }

        for (pattern_token, cmd_token) in self.rest.iter().zip(&cmd[1..self.len()]) {
            if !pattern_token.matches(cmd_token) {
                return None;
            }
        }

        Some(cmd[..self.len()].to_vec())
    }
}

#[derive(Clone, Debug)]
pub enum Rule {
    Prefix(PrefixRule),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixRule {
    pub pattern: PrefixPattern,
    pub decision: Decision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

impl Rule {
    pub fn program(&self) -> &str {
        match self {
            Self::Prefix(rule) => &rule.pattern.first,
        }
    }

    pub fn matches(&self, cmd: &[String]) -> Option<RuleMatch> {
        match self {
            Self::Prefix(rule) => rule.matches(cmd),
        }
    }

    pub fn validate_examples(
        &self,
        matches: &[Vec<String>],
        not_matches: &[Vec<String>],
    ) -> Result<()> {
        match self {
            Self::Prefix(rule) => rule.validate_examples(matches, not_matches),
        }
    }
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

    pub fn validate_examples(
        &self,
        matches: &[Vec<String>],
        not_matches: &[Vec<String>],
    ) -> Result<()> {
        for example in matches {
            if self.matches(example).is_none() {
                return Err(Error::ExampleDidNotMatch {
                    rule: self.description(),
                    example: join_command(example),
                });
            }
        }
        for example in not_matches {
            if self.matches(example).is_some() {
                return Err(Error::ExampleDidMatch {
                    rule: self.description(),
                    example: join_command(example),
                });
            }
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!(
            "prefix_rule(pattern = [{}], decision = {})",
            render_pattern(&self.pattern),
            render_decision(self.decision)
        )
    }
}

fn join_command(command: &[String]) -> String {
    try_join(command.iter().map(String::as_str))
        .unwrap_or_else(|_| "unable to render example".to_string())
}

fn render_pattern(pattern: &PrefixPattern) -> String {
    let mut tokens = vec![pattern.first.clone()];
    tokens.extend(pattern.rest.iter().map(render_pattern_token));
    tokens.join(", ")
}

fn render_pattern_token(token: &PatternToken) -> String {
    match token {
        PatternToken::Single(value) => value.clone(),
        PatternToken::Alts(values) => format!("[{}]", values.join(", ")),
    }
}

fn render_decision(decision: Decision) -> &'static str {
    match decision {
        Decision::Allow => "allow",
        Decision::Prompt => "prompt",
        Decision::Forbidden => "forbidden",
    }
}

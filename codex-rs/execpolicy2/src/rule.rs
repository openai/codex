use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use shlex::try_join;

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
pub struct Rule {
    pub id: String,
    pub pattern: PrefixPattern,
    pub decision: Decision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleMatch {
    pub rule_id: String,
    pub matched_prefix: Vec<String>,
    pub decision: Decision,
}

impl Rule {
    pub fn matches(&self, cmd: &[String]) -> Option<RuleMatch> {
        self.pattern
            .matches_prefix(cmd)
            .map(|matched_prefix| RuleMatch {
                rule_id: self.id.clone(),
                matched_prefix,
                decision: self.decision,
            })
    }

    pub fn validate_examples(
        &self,
        positive: &[Vec<String>],
        negative: &[Vec<String>],
    ) -> Result<()> {
        for example in positive {
            if self.matches(example).is_none() {
                return Err(Error::ExampleDidNotMatch {
                    rule_id: self.id.clone(),
                    example: join_command(example),
                });
            }
        }
        for example in negative {
            if self.matches(example).is_some() {
                return Err(Error::ExampleDidMatch {
                    rule_id: self.id.clone(),
                    example: join_command(example),
                });
            }
        }
        Ok(())
    }
}

fn join_command(command: &[String]) -> String {
    try_join(command.iter().map(String::as_str))
        .expect("failed to render command with shlex::try_join")
}

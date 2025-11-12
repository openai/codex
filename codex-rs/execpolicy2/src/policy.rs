use crate::decision::Decision;
use crate::rule::Rule;
use crate::rule::RuleMatch;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Policy {
    rules_by_program: MultiMap<String, Rule>,
}

impl Policy {
    pub fn new(rules_by_program: MultiMap<String, Rule>) -> Self {
        Self { rules_by_program }
    }

    pub fn rules(&self) -> &MultiMap<String, Rule> {
        &self.rules_by_program
    }

    pub fn check(&self, cmd: &[String]) -> Evaluation {
        let rules = match cmd.first() {
            Some(first) => match self.rules_by_program.get_vec(first) {
                Some(rules) => rules,
                None => return Evaluation::NoMatch,
            },
            None => return Evaluation::NoMatch,
        };

        let matched_rules: Vec<RuleMatch> =
            rules.iter().filter_map(|rule| rule.matches(cmd)).collect();
        match matched_rules.iter().map(RuleMatch::decision).max() {
            Some(decision) => Evaluation::Match {
                decision,
                matched_rules,
            },
            None => Evaluation::NoMatch,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Evaluation {
    NoMatch,
    Match {
        decision: Decision,
        matched_rules: Vec<RuleMatch>,
    },
}

impl Evaluation {
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match { .. })
    }
}

impl std::fmt::Display for Evaluation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch => f.write_str("noMatch"),
            Self::Match {
                decision,
                matched_rules,
            } => {
                writeln!(f, "match {{")?;
                writeln!(f, "  decision: {decision},")?;
                writeln!(f, "  matchedRules: [")?;
                for rule in matched_rules {
                    writeln!(f, "    {rule},")?;
                }
                write!(f, "  ]\n}}")
            }
        }
    }
}

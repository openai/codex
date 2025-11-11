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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Evaluation {
    NoMatch,
    Match {
        decision: Decision,
        matched_rules: Vec<RuleMatch>,
    },
}

impl Policy {
    pub fn new(rules_by_program: MultiMap<String, Rule>) -> Self {
        Self { rules_by_program }
    }

    pub fn rules(&self) -> &MultiMap<String, Rule> {
        &self.rules_by_program
    }

    pub fn evaluate(&self, cmd: &[String]) -> Evaluation {
        let rules = match cmd.first() {
            Some(first) => match self.rules_by_program.get_vec(first) {
                Some(rules) => rules,
                None => return Evaluation::NoMatch,
            },
            None => return Evaluation::NoMatch,
        };
        let mut matched_rules: Vec<RuleMatch> = Vec::new();
        let mut strictest_decision: Option<Decision> = None;
        for rule in rules {
            if let Some(matched) = rule.matches(cmd) {
                let decision = match strictest_decision {
                    None => matched.decision,
                    Some(current) => std::cmp::max(matched.decision, current),
                };
                strictest_decision = Some(decision);
                matched_rules.push(matched);
            }
        }
        match strictest_decision {
            Some(decision) => Evaluation::Match {
                decision,
                matched_rules,
            },
            None => Evaluation::NoMatch,
        }
    }
}

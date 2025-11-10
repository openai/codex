use crate::decision::Decision;
use crate::rule::Rule;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Policy {
    rules_by_program: MultiMap<String, Rule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub decision: Decision,
    pub matched_rules: Vec<crate::rule::RuleMatch>,
}

impl Policy {
    pub fn new(rules_by_program: MultiMap<String, Rule>) -> Self {
        Self { rules_by_program }
    }

    pub fn rules(&self) -> &MultiMap<String, Rule> {
        &self.rules_by_program
    }

    pub fn evaluate(&self, cmd: &[String]) -> Option<Evaluation> {
        let first = cmd.first()?;
        let Some(rules) = self.rules_by_program.get_vec(first) else {
            return None;
        };
        let mut matched_rules: Vec<crate::rule::RuleMatch> = Vec::new();
        let mut best_decision: Option<Decision> = None;
        for rule in rules {
            if let Some(matched) = rule.matches(cmd) {
                let decision = match best_decision {
                    None => matched.decision,
                    Some(current) => {
                        if matched.decision.is_stricter_than(current) {
                            matched.decision
                        } else {
                            current
                        }
                    }
                };
                best_decision = Some(decision);
                matched_rules.push(matched);
            }
        }
        best_decision.map(|decision| Evaluation {
            decision,
            matched_rules,
        })
    }
}

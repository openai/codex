use crate::decision::Decision;
use crate::rule::Rule;
use crate::rule::RuleMatch;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Policy {
    rules: Vec<Rule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub rule_id: String,
    pub decision: Decision,
    pub matched_prefix: Vec<String>,
    pub remainder: Vec<String>,
}

impl From<RuleMatch> for Evaluation {
    fn from(value: RuleMatch) -> Self {
        Self {
            rule_id: value.rule_id,
            decision: value.decision,
            matched_prefix: value.matched_prefix,
            remainder: value.remainder,
        }
    }
}

impl Policy {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn evaluate(&self, cmd: &[String]) -> Option<Evaluation> {
        let mut best: Option<Evaluation> = None;
        for rule in &self.rules {
            if let Some(matched) = rule.matches(cmd) {
                let eval = Evaluation::from(matched);
                best = match best {
                    None => Some(eval),
                    Some(current) => {
                        if eval.decision.is_stricter_than(current.decision) {
                            Some(eval)
                        } else {
                            Some(current)
                        }
                    }
                };
            }
        }
        best
    }
}

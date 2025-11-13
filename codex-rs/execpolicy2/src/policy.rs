use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use crate::rule::RuleMatch;
use crate::rule::RuleRef;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;
use shlex::try_join;

#[derive(Clone, Debug)]
pub struct Policy {
    rules_by_program: MultiMap<String, RuleRef>,
}

impl Policy {
    pub fn new(rules_by_program: MultiMap<String, RuleRef>) -> Self {
        Self { rules_by_program }
    }

    pub fn rules(&self) -> &MultiMap<String, RuleRef> {
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

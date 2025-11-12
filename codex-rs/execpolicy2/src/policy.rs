use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use crate::rule::Rule;
use crate::rule::RuleMatch;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;
use shlex::try_join;

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

/// Count how many rules match each provided example and error if any example is unmatched.
pub(crate) fn validate_match_examples(rules: &[Rule], matches: &[Vec<String>]) -> Result<()> {
    let match_counts = rules.iter().fold(vec![0; matches.len()], |counts, rule| {
        counts
            .iter()
            .zip(rule.validate_matches(matches))
            .map(|(count, matched)| if matched { count + 1 } else { *count })
            .collect()
    });

    let unmatched_examples: Vec<String> = matches
        .iter()
        .zip(&match_counts)
        .filter_map(|(example, count)| {
            if *count == 0 {
                Some(
                    try_join(example.iter().map(String::as_str))
                        .unwrap_or_else(|_| "unable to render example".to_string()),
                )
            } else {
                None
            }
        })
        .collect();

    if unmatched_examples.is_empty() {
        Ok(())
    } else {
        Err(Error::ExampleDidNotMatch {
            rules: rules.iter().map(|rule| format!("{rule:?}")).collect(),
            examples: unmatched_examples,
        })
    }
}

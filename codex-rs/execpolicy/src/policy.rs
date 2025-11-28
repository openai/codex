use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use crate::rule::PatternToken;
use crate::rule::PrefixPattern;
use crate::rule::PrefixRule;
use crate::rule::RuleMatch;
use crate::rule::RuleRef;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Policy {
    rules_by_program: MultiMap<String, RuleRef>,
}

impl Policy {
    pub fn new(rules_by_program: MultiMap<String, RuleRef>) -> Self {
        Self { rules_by_program }
    }

    pub fn empty() -> Self {
        Self::new(MultiMap::new())
    }

    pub fn rules(&self) -> &MultiMap<String, RuleRef> {
        &self.rules_by_program
    }

    pub fn add_prefix_rule(&mut self, prefix: &[String], decision: Decision) -> Result<()> {
        let (first_token, rest) = prefix
            .split_first()
            .ok_or_else(|| Error::InvalidPattern("prefix cannot be empty".to_string()))?;

        let rule: RuleRef = Arc::new(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from(first_token.as_str()),
                rest: rest
                    .iter()
                    .map(|token| PatternToken::Single(token.clone()))
                    .collect::<Vec<_>>()
                    .into(),
            },
            decision,
        });

        self.rules_by_program.insert(first_token.clone(), rule);
        Ok(())
    }

    pub fn check(&self, cmd: &[String]) -> Evaluation {
        let rules = match cmd.first() {
            Some(first) => match self.rules_by_program.get_vec(first) {
                Some(rules) => rules,
                None => return Evaluation::NoMatch {},
            },
            None => return Evaluation::NoMatch {},
        };

        let matched_rules: Vec<RuleMatch> =
            rules.iter().filter_map(|rule| rule.matches(cmd)).collect();
        match matched_rules.iter().map(RuleMatch::decision).max() {
            Some(decision) => Evaluation::Match {
                decision,
                matched_rules,
            },
            None => Evaluation::NoMatch {},
        }
    }

    pub fn check_multiple<Commands>(&self, commands: Commands) -> Evaluation
    where
        Commands: IntoIterator,
        Commands::Item: AsRef<[String]>,
    {
        let matched_rules: Vec<RuleMatch> = commands
            .into_iter()
            .flat_map(|command| match self.check(command.as_ref()) {
                Evaluation::Match { matched_rules, .. } => matched_rules,
                Evaluation::NoMatch { .. } => Vec::new(),
            })
            .collect();

        match matched_rules.iter().map(RuleMatch::decision).max() {
            Some(decision) => Evaluation::Match {
                decision,
                matched_rules,
            },
            None => Evaluation::NoMatch {},
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Evaluation {
    NoMatch {},
    Match {
        decision: Decision,
        #[serde(rename = "matchedRules")]
        matched_rules: Vec<RuleMatch>,
    },
}

impl Evaluation {
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match { .. })
    }
}

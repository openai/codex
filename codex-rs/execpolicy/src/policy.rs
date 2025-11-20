use crate::decision::Decision;
use crate::rule::RuleMatch;
use crate::rule::RuleRef;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;

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

    pub fn check_multiple<Commands>(&self, commands: Commands) -> Evaluation
    where
        Commands: IntoIterator,
        Commands::Item: AsRef<[String]>,
    {
        let matched_rules: Vec<RuleMatch> = commands
            .into_iter()
            .flat_map(|command| match self.check(command.as_ref()) {
                Evaluation::Match { matched_rules, .. } => matched_rules,
                Evaluation::NoMatch => Vec::new(),
            })
            .collect();

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
        #[serde(rename = "matchedRules")]
        matched_rules: Vec<RuleMatch>,
    },
}

impl Evaluation {
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match { .. })
    }
}

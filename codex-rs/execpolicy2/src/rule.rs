use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub prefixes: Vec<Vec<String>>,
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
        for prefix in &self.prefixes {
            if prefix.len() > cmd.len() {
                continue;
            }
            if cmd
                .iter()
                .zip(prefix)
                .all(|(cmd_tok, prefix_tok)| cmd_tok == prefix_tok)
            {
                return Some(RuleMatch {
                    rule_id: self.id.clone(),
                    matched_prefix: prefix.clone(),
                    decision: self.decision,
                });
            }
        }
        None
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
                    example: example.join(" "),
                });
            }
        }
        for example in negative {
            if self.matches(example).is_some() {
                return Err(Error::ExampleDidMatch {
                    rule_id: self.id.clone(),
                    example: example.join(" "),
                });
            }
        }
        Ok(())
    }
}

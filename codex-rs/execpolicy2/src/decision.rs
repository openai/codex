use serde::Deserialize;
use serde::Serialize;

use crate::error::Error;
use crate::error::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Allow,
    Prompt,
    Forbidden,
}

impl Decision {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "allow" => Ok(Self::Allow),
            "prompt" => Ok(Self::Prompt),
            "forbidden" => Ok(Self::Forbidden),
            other => Err(Error::InvalidDecision(other.to_string())),
        }
    }

    /// Returns true if `self` is stricter (less permissive) than `other`.
    pub fn is_stricter_than(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Decision::Forbidden, Decision::Prompt | Decision::Allow)
                | (Decision::Prompt, Decision::Allow)
        )
    }
}

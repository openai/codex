use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize, Serialize)]
pub struct DirectiveResponse {
    pub directive: String,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerifierDecision {
    Pass,
    Fail,
}

impl VerifierDecision {
    pub fn is_pass(self) -> bool {
        matches!(self, VerifierDecision::Pass)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VerifierVerdict {
    pub verdict: VerifierDecision,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VerifierReport {
    pub role: String,
    pub verdict: VerifierDecision,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AggregatedVerifierVerdict {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub overall: VerifierDecision,
    pub verdicts: Vec<VerifierReport>,
}

impl From<&AggregatedVerifierVerdict> for String {
    fn from(value: &AggregatedVerifierVerdict) -> Self {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    }
}

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyDecisionPayload {
    pub decision: String,
    pub source: String,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub reason: Option<String>,
    pub port: Option<u16>,
}

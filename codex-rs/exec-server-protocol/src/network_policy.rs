use serde::Deserialize;
use serde::Serialize;

use crate::ProcessId;

pub const NETWORK_POLICY_REQUEST_METHOD: &str = "network/policyRequest";
pub const MAX_NETWORK_POLICY_HOST_BYTES: usize = 253;
pub const MAX_NETWORK_POLICY_PROCESS_ID_BYTES: usize = 256;
pub const MAX_NETWORK_POLICY_REASON_BYTES: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyRequestParams {
    pub process_id: ProcessId,
    pub request: ExecServerNetworkPolicyRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecServerNetworkPolicyRequest {
    pub protocol: ExecServerNetworkProtocol,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecServerNetworkProtocol {
    Http,
    HttpsConnect,
    Socks5Tcp,
    Socks5Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyRequestResponse {
    pub decision: ExecServerNetworkPolicyDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecServerNetworkPolicyDecision {
    Allow,
    Deny { reason: String },
    Ask { reason: String },
}

#[cfg(test)]
#[path = "network_policy_tests.rs"]
mod tests;

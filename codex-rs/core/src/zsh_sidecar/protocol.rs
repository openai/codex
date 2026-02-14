use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

pub(crate) const JSONRPC_VERSION: &str = "2.0";
pub(crate) const METHOD_ZSH_INITIALIZE: &str = "zsh/initialize";
pub(crate) const METHOD_ZSH_EXEC_START: &str = "zsh/execStart";
pub(crate) const METHOD_ZSH_EXEC_STDIN: &str = "zsh/execStdin";
pub(crate) const METHOD_ZSH_EXEC_RESIZE: &str = "zsh/execResize";
pub(crate) const METHOD_ZSH_EXEC_INTERRUPT: &str = "zsh/execInterrupt";
pub(crate) const METHOD_ZSH_SHUTDOWN: &str = "zsh/shutdown";
pub(crate) const METHOD_ZSH_REQUEST_APPROVAL: &str = "zsh/requestApproval";
pub(crate) const METHOD_ZSH_EVENT_EXEC_STARTED: &str = "zsh/event/execStarted";
pub(crate) const METHOD_ZSH_EVENT_EXEC_STDOUT: &str = "zsh/event/execStdout";
pub(crate) const METHOD_ZSH_EVENT_EXEC_STDERR: &str = "zsh/event/execStderr";
pub(crate) const METHOD_ZSH_EVENT_TERMINAL_INTERACTION: &str = "zsh/event/terminalInteraction";
pub(crate) const METHOD_ZSH_EVENT_EXEC_EXITED: &str = "zsh/event/execExited";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum JsonRpcId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JsonRpcRequest<T> {
    pub(crate) jsonrpc: String,
    pub(crate) id: JsonRpcId,
    pub(crate) method: String,
    pub(crate) params: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JsonRpcNotification<T> {
    pub(crate) jsonrpc: String,
    pub(crate) method: String,
    pub(crate) params: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JsonRpcSuccess<T> {
    pub(crate) jsonrpc: String,
    pub(crate) id: JsonRpcId,
    pub(crate) result: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JsonRpcFailure {
    pub(crate) jsonrpc: String,
    pub(crate) id: JsonRpcId,
    pub(crate) error: JsonRpcError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZshCapabilities {
    pub(crate) interactive_pty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeParams {
    pub(crate) session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeResult {
    pub(crate) protocol_version: u32,
    pub(crate) capabilities: ZshCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct EmptyResult {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecStartParams {
    pub(crate) exec_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: String,
    #[serde(default)]
    pub(crate) env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub(crate) tty: Option<bool>,
    #[serde(default)]
    pub(crate) cols: Option<u16>,
    #[serde(default)]
    pub(crate) rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecStdinParams {
    pub(crate) exec_id: String,
    pub(crate) chunk_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecResizeParams {
    pub(crate) exec_id: String,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecInterruptParams {
    pub(crate) exec_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShutdownParams {
    #[serde(default)]
    pub(crate) grace_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecStartedEvent {
    pub(crate) exec_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecStdoutEvent {
    pub(crate) exec_id: String,
    pub(crate) chunk_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecStderrEvent {
    pub(crate) exec_id: String,
    pub(crate) chunk_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalInteractionEvent {
    pub(crate) exec_id: String,
    pub(crate) interaction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecExitedEvent {
    pub(crate) exec_id: String,
    pub(crate) exit_code: i32,
    #[serde(default)]
    pub(crate) signal: Option<String>,
    #[serde(default)]
    pub(crate) timed_out: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecPolicyAmendmentProposal {
    pub(crate) command_prefix: Vec<String>,
    #[serde(default)]
    pub(crate) rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalDecision {
    Approved,
    ApprovedForSession,
    ApprovedExecpolicyAmendment,
    Denied,
    Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestApprovalParams {
    pub(crate) approval_id: String,
    pub(crate) exec_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: String,
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) proposed_execpolicy_amendment: Option<ExecPolicyAmendmentProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestApprovalResult {
    pub(crate) decision: ApprovalDecision,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn serialize_initialize_request_uses_expected_shape() {
        let request = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: JsonRpcId::Number(1),
            method: METHOD_ZSH_INITIALIZE.to_string(),
            params: InitializeParams {
                session_id: "session-1".to_string(),
            },
        };

        let value = serde_json::to_value(request).expect("serialize");
        assert_eq!(
            value,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "zsh/initialize",
                "params": { "sessionId": "session-1" }
            })
        );
    }

    #[test]
    fn deserialize_request_approval_callback() {
        let value = json!({
            "jsonrpc": "2.0",
            "id": "cb-1",
            "method": "zsh/requestApproval",
            "params": {
                "approvalId": "ap-1",
                "execId": "exec-1",
                "command": ["git", "status"],
                "cwd": "/tmp",
                "reason": "policy",
                "proposedExecpolicyAmendment": {
                    "commandPrefix": ["git", "status"],
                    "rationale": "safe command"
                }
            }
        });
        let request: JsonRpcRequest<RequestApprovalParams> =
            serde_json::from_value(value).expect("deserialize");

        assert_eq!(request.method, METHOD_ZSH_REQUEST_APPROVAL);
        assert_eq!(
            request.params.proposed_execpolicy_amendment,
            Some(ExecPolicyAmendmentProposal {
                command_prefix: vec!["git".to_string(), "status".to_string()],
                rationale: Some("safe command".to_string()),
            })
        );
    }

    #[test]
    fn serialize_approval_result_uses_snake_case_decision() {
        let response = JsonRpcSuccess {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: JsonRpcId::String("cb-1".to_string()),
            result: RequestApprovalResult {
                decision: ApprovalDecision::ApprovedForSession,
            },
        };
        let value = serde_json::to_value(response).expect("serialize");
        assert_eq!(
            value,
            json!({
                "jsonrpc": "2.0",
                "id": "cb-1",
                "result": {
                    "decision": "approved_for_session"
                }
            })
        );
    }
}

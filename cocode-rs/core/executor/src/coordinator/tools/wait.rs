use serde::{Deserialize, Serialize};

/// Request to wait for an agent to complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitRequest {
    /// The ID of the agent to wait for.
    pub agent_id: String,

    /// Optional timeout in seconds. `None` means wait indefinitely.
    #[serde(default)]
    pub timeout_secs: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_request_no_timeout() {
        let json = r#"{"agent_id":"agent-456"}"#;
        let req: WaitRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.agent_id, "agent-456");
        assert!(req.timeout_secs.is_none());
    }

    #[test]
    fn test_wait_request_with_timeout() {
        let req = WaitRequest {
            agent_id: "agent-789".to_string(),
            timeout_secs: Some(30),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: WaitRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.agent_id, "agent-789");
        assert_eq!(back.timeout_secs, Some(30));
    }
}

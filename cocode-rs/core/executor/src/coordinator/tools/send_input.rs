use serde::{Deserialize, Serialize};

/// Request to send input to a running agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendInputRequest {
    /// The ID of the agent to send input to.
    pub agent_id: String,

    /// The input text to deliver.
    pub input: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_input_request_serde() {
        let req = SendInputRequest {
            agent_id: "agent-123".to_string(),
            input: "run tests".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: SendInputRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.agent_id, "agent-123");
        assert_eq!(back.input, "run tests");
    }
}

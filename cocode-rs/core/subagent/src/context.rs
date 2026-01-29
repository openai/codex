use serde::{Deserialize, Serialize};

/// Context linking a child subagent session back to its parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildToolUseContext {
    /// Session ID of the parent agent that spawned this child.
    pub parent_session_id: String,

    /// Session ID assigned to the child subagent.
    pub child_session_id: String,

    /// The turn number in the parent at which the child was forked.
    pub forked_from_turn: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_child_context_serde_roundtrip() {
        let ctx = ChildToolUseContext {
            parent_session_id: "parent-123".to_string(),
            child_session_id: "child-456".to_string(),
            forked_from_turn: 7,
        };
        let json = serde_json::to_string(&ctx).expect("serialize");
        let back: ChildToolUseContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.parent_session_id, "parent-123");
        assert_eq!(back.child_session_id, "child-456");
        assert_eq!(back.forked_from_turn, 7);
    }
}

//! Hook result types.
//!
//! After a hook executes, it produces a `HookResult` that determines how the
//! agent loop should proceed.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The outcome of a single hook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HookResult {
    /// Continue normal execution (hook did not intervene).
    Continue,

    /// Reject the current action.
    Reject {
        /// Human-readable reason for rejection.
        reason: String,
    },

    /// Modify the input before the action proceeds.
    ModifyInput {
        /// The replacement input.
        new_input: Value,
    },
}

/// A completed hook execution with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutcome {
    /// Name of the hook that ran.
    pub hook_name: String,

    /// The result produced by the hook.
    pub result: HookResult,

    /// Wall-clock duration of hook execution in milliseconds.
    pub duration_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continue_serde() {
        let result = HookResult::Continue;
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: HookResult = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(parsed, HookResult::Continue));
    }

    #[test]
    fn test_reject_serde() {
        let result = HookResult::Reject {
            reason: "not allowed".to_string(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("not allowed"));
        let parsed: HookResult = serde_json::from_str(&json).expect("deserialize");
        if let HookResult::Reject { reason } = parsed {
            assert_eq!(reason, "not allowed");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_modify_input_serde() {
        let result = HookResult::ModifyInput {
            new_input: serde_json::json!({"modified": true}),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: HookResult = serde_json::from_str(&json).expect("deserialize");
        if let HookResult::ModifyInput { new_input } = parsed {
            assert_eq!(new_input["modified"], true);
        } else {
            panic!("Expected ModifyInput");
        }
    }

    #[test]
    fn test_hook_outcome() {
        let outcome = HookOutcome {
            hook_name: "lint-check".to_string(),
            result: HookResult::Continue,
            duration_ms: 42,
        };
        let json = serde_json::to_string(&outcome).expect("serialize");
        let parsed: HookOutcome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.hook_name, "lint-check");
        assert_eq!(parsed.duration_ms, 42);
        assert!(matches!(parsed.result, HookResult::Continue));
    }
}

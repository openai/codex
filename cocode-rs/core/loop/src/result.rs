use serde::{Deserialize, Serialize};

/// Describes why the agent loop stopped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopReason {
    /// The loop exhausted its maximum turn budget.
    MaxTurnsReached,

    /// The model emitted an explicit stop signal (end_turn, stop, etc.).
    ModelStopSignal,

    /// The user cancelled the loop (e.g. Ctrl-C).
    UserInterrupted,

    /// The loop terminated due to an error.
    Error {
        /// Human-readable error description.
        message: String,
    },

    /// The loop exited plan mode.
    PlanModeExit {
        /// Whether the plan was approved by the user.
        approved: bool,
    },

    /// A hook requested the loop to stop.
    HookStopped,
}

/// Aggregate result of a completed agent loop run.
#[derive(Debug, Clone)]
pub struct LoopResult {
    /// The reason the loop stopped.
    pub stop_reason: StopReason,

    /// Total number of turns completed.
    pub turns_completed: i32,

    /// Cumulative input tokens consumed across all turns.
    pub total_input_tokens: i32,

    /// Cumulative output tokens generated across all turns.
    pub total_output_tokens: i32,

    /// Final text response from the model (last assistant message text).
    pub final_text: String,

    /// All content blocks from the last response.
    pub last_response_content: Vec<hyper_sdk::ContentBlock>,
}

impl LoopResult {
    /// Create a result for model stop signal.
    pub fn completed(
        turns: i32,
        input_tokens: i32,
        output_tokens: i32,
        text: String,
        content: Vec<hyper_sdk::ContentBlock>,
    ) -> Self {
        Self {
            stop_reason: StopReason::ModelStopSignal,
            turns_completed: turns,
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            final_text: text,
            last_response_content: content,
        }
    }

    /// Create a result for max turns reached.
    pub fn max_turns_reached(turns: i32, input_tokens: i32, output_tokens: i32) -> Self {
        Self {
            stop_reason: StopReason::MaxTurnsReached,
            turns_completed: turns,
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            final_text: String::new(),
            last_response_content: Vec::new(),
        }
    }

    /// Create a result for hook stop.
    pub fn hook_stopped(turns: i32, input_tokens: i32, output_tokens: i32) -> Self {
        Self {
            stop_reason: StopReason::HookStopped,
            turns_completed: turns,
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            final_text: String::new(),
            last_response_content: Vec::new(),
        }
    }

    /// Create a result for user interruption.
    pub fn interrupted(turns: i32, input_tokens: i32, output_tokens: i32) -> Self {
        Self {
            stop_reason: StopReason::UserInterrupted,
            turns_completed: turns,
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            final_text: String::new(),
            last_response_content: Vec::new(),
        }
    }

    /// Create a result for an error.
    pub fn error(turns: i32, input_tokens: i32, output_tokens: i32, message: String) -> Self {
        Self {
            stop_reason: StopReason::Error { message },
            turns_completed: turns,
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            final_text: String::new(),
            last_response_content: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_reason_variants() {
        let reasons = vec![
            StopReason::MaxTurnsReached,
            StopReason::ModelStopSignal,
            StopReason::UserInterrupted,
            StopReason::Error {
                message: "timeout".to_string(),
            },
            StopReason::PlanModeExit { approved: true },
            StopReason::PlanModeExit { approved: false },
            StopReason::HookStopped,
        ];
        // Verify all variants can be cloned and debug-printed.
        for reason in &reasons {
            let _cloned = reason.clone();
            let _debug = format!("{reason:?}");
        }
    }

    #[test]
    fn test_loop_result_completed() {
        let result = LoopResult::completed(
            5,
            1000,
            500,
            "Hello".to_string(),
            vec![hyper_sdk::ContentBlock::text("Hello")],
        );
        assert_eq!(result.turns_completed, 5);
        assert_eq!(result.total_input_tokens, 1000);
        assert_eq!(result.total_output_tokens, 500);
        assert_eq!(result.final_text, "Hello");
        assert_eq!(result.last_response_content.len(), 1);
    }

    #[test]
    fn test_loop_result_max_turns() {
        let result = LoopResult::max_turns_reached(10, 2000, 1000);
        assert_eq!(result.turns_completed, 10);
        assert!(result.final_text.is_empty());
        assert!(matches!(result.stop_reason, StopReason::MaxTurnsReached));
    }

    #[test]
    fn test_loop_result_error() {
        let result = LoopResult::error(3, 500, 200, "timeout".to_string());
        match &result.stop_reason {
            StopReason::Error { message } => assert_eq!(message, "timeout"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_stop_reason_serde_roundtrip() {
        let reason = StopReason::Error {
            message: "provider unavailable".to_string(),
        };
        let json = serde_json::to_string(&reason).expect("serialize");
        let back: StopReason = serde_json::from_str(&json).expect("deserialize");
        match back {
            StopReason::Error { message } => assert_eq!(message, "provider unavailable"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_plan_mode_exit_serde() {
        let reason = StopReason::PlanModeExit { approved: true };
        let json = serde_json::to_string(&reason).expect("serialize");
        let back: StopReason = serde_json::from_str(&json).expect("deserialize");
        match back {
            StopReason::PlanModeExit { approved } => assert!(approved),
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}

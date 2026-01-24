//! Loop execution extension for exec mode.
//!
//! Provides iterative execution support for --iter and --time flags.
//! This module encapsulates loop state tracking to minimize changes to lib.rs.

use codex_core::CodexThread;
use codex_core::loop_driver::LoopCondition;
use codex_core::loop_driver::LoopPromptBuilder;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::user_input::UserInput;
use std::path::PathBuf;
use std::time::Instant;
use tracing::info;
use tracing::warn;

/// Tracks loop iteration state across multiple turns.
pub struct LoopState {
    start_time: Instant,
    current_iteration: i32,
    iterations_succeeded: i32,
    iterations_failed: i32,
    original_prompt: String,
    condition: Option<LoopCondition>,
}

impl LoopState {
    /// Creates a new loop state tracker.
    pub fn new(condition: Option<LoopCondition>, original_prompt: String) -> Self {
        Self {
            start_time: Instant::now(),
            current_iteration: 0,
            iterations_succeeded: 0,
            iterations_failed: 0,
            original_prompt,
            condition,
        }
    }

    /// Checks if we should continue to the next iteration.
    pub fn should_continue(&self) -> bool {
        match &self.condition {
            Some(LoopCondition::Iters { count }) => self.current_iteration + 1 < *count,
            Some(LoopCondition::Duration { seconds }) => {
                self.start_time.elapsed().as_secs() < (*seconds as u64)
            }
            None => false,
        }
    }

    /// Tracks completion of an iteration.
    pub fn track_iteration(&mut self, had_error: bool) {
        if had_error {
            self.iterations_failed += 1;
        } else {
            self.iterations_succeeded += 1;
        }
        self.current_iteration += 1;
    }

    /// Returns true if any iterations failed.
    pub fn has_failures(&self) -> bool {
        self.iterations_failed > 0
    }

    /// Builds the continuation prompt for the next iteration.
    pub fn build_continuation_prompt(&self) -> String {
        LoopPromptBuilder::build(&self.original_prompt, self.current_iteration)
    }

    /// Logs iteration start info.
    pub fn log_iteration_start(&self) {
        info!(
            iteration = self.current_iteration,
            succeeded = self.iterations_succeeded,
            failed = self.iterations_failed,
            "Starting next iteration"
        );
    }

    /// Logs loop completion info.
    pub fn log_loop_complete(&self) {
        if self.condition.is_some() {
            info!(
                total_iterations = self.current_iteration,
                succeeded = self.iterations_succeeded,
                failed = self.iterations_failed,
                "Loop complete"
            );
        }
    }

    /// Logs loop interruption warning.
    #[allow(dead_code)]
    pub fn log_interrupted(&self) {
        if self.condition.is_some() && self.current_iteration > 0 {
            warn!(
                iteration = self.current_iteration,
                succeeded = self.iterations_succeeded,
                failed = self.iterations_failed,
                "Loop interrupted, shutting down"
            );
        }
    }
}

/// Parameters for submitting the next iteration turn.
pub struct NextTurnParams {
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub model: String,
    pub effort: Option<ReasoningEffort>,
    pub summary: ReasoningSummary,
}

/// Submits the next iteration turn.
pub async fn submit_next_iteration(
    thread: &CodexThread,
    loop_state: &LoopState,
    params: &NextTurnParams,
) -> anyhow::Result<()> {
    let continuation_prompt = loop_state.build_continuation_prompt();
    let items = vec![UserInput::Text {
        text: continuation_prompt,
        text_elements: vec![],
    }];

    thread
        .submit(Op::UserTurn {
            items,
            cwd: params.cwd.clone(),
            approval_policy: params.approval_policy,
            sandbox_policy: params.sandbox_policy.clone(),
            model: params.model.clone(),
            effort: params.effort,
            summary: params.summary,
            final_output_json_schema: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    Ok(())
}

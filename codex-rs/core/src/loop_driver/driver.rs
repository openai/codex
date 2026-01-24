use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

use super::condition::LoopCondition;
use super::context::IterationRecord;
use super::context::LoopContext;
use super::git_ops;
use super::prompt;
use super::prompt::LoopPromptBuilder;
use super::summarizer;
use crate::auth::AuthManager;
use crate::client::ModelClient;
use crate::codex::Codex;
use crate::config::Config;
use crate::spawn_task::LogFileSink;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;

/// Progress information for callback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopProgress {
    /// Current iteration number (0-indexed, after completion).
    pub iteration: i32,
    /// Number of iterations that succeeded.
    pub succeeded: i32,
    /// Number of iterations that failed.
    pub failed: i32,
    /// Elapsed time in seconds.
    pub elapsed_seconds: i64,
}

/// Result of loop execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopResult {
    /// Number of iterations attempted.
    pub iterations_attempted: i32,
    /// Number of iterations that succeeded.
    pub iterations_succeeded: i32,
    /// Number of iterations that failed.
    pub iterations_failed: i32,
    /// Reason the loop stopped.
    pub stop_reason: LoopStopReason,
    /// Total elapsed time in seconds.
    pub elapsed_seconds: i64,
}

/// Reason why the loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStopReason {
    /// Completed all iterations (count mode).
    Completed,
    /// Duration elapsed (time mode).
    DurationElapsed,
    /// Cancelled via CancellationToken.
    Cancelled,
    /// Task returned None (aborted internally).
    TaskAborted,
}

/// Context for lazy ModelClient creation in summarization.
///
/// Passed to LoopDriver to enable LLM-based iteration summarization.
/// The ModelClient is created lazily on first use.
#[derive(Clone)]
pub struct SummarizerContext {
    /// Auth manager for API authentication.
    pub auth_manager: Arc<AuthManager>,
    /// Config with model provider settings.
    pub config: Arc<Config>,
    /// Conversation ID for telemetry.
    pub conversation_id: ThreadId,
}

/// Driver for loop-based agent execution.
///
/// Wraps the standard run_task() with loop/time-based execution control.
/// **Key behavior:** Continue-on-error - iterations continue after failure.
///
/// # Example
///
/// ```rust,ignore
/// let condition = LoopCondition::Iters { count: 5 };
/// let mut driver = LoopDriver::new(condition, cancellation_token);
///
/// while driver.should_continue() {
///     let query = driver.build_query("original query");
///     // Execute iteration...
///     driver.mark_iteration_complete(success);
/// }
///
/// let result = driver.finish();
/// println!("Completed {} of {} iterations", result.iterations_succeeded, result.iterations_attempted);
/// ```
pub struct LoopDriver {
    condition: LoopCondition,
    start_time: Instant,
    iteration: i32,
    iterations_failed: i32,
    cancellation_token: CancellationToken,
    custom_loop_prompt: Option<String>,
    /// Optional progress callback for real-time updates.
    progress_callback: Option<Box<dyn Fn(LoopProgress) + Send + Sync>>,

    // === Context passing fields ===
    /// Loop context (optional, enabled via with_context_passing).
    context: Option<LoopContext>,
    /// Working directory for git operations.
    cwd: Option<PathBuf>,
    /// ModelClient for summary/commit message generation (direct).
    model_client: Option<Arc<ModelClient>>,
    /// Summarizer context for lazy ModelClient creation.
    summarizer_ctx: Option<SummarizerContext>,
    /// Cached ModelClient created from summarizer_ctx.
    cached_summarization_client: Option<Arc<ModelClient>>,
}

impl LoopDriver {
    /// Create a new LoopDriver.
    ///
    /// # Arguments
    ///
    /// * `condition` - Loop termination condition
    /// * `token` - Cancellation token for graceful shutdown
    pub fn new(condition: LoopCondition, token: CancellationToken) -> Self {
        Self {
            condition,
            start_time: Instant::now(),
            iteration: 0,
            iterations_failed: 0,
            cancellation_token: token,
            custom_loop_prompt: None,
            progress_callback: None,
            context: None,
            cwd: None,
            model_client: None,
            summarizer_ctx: None,
            cached_summarization_client: None,
        }
    }

    /// Set custom loop prompt (instead of default git-based prompt).
    pub fn with_custom_prompt(mut self, prompt: String) -> Self {
        self.custom_loop_prompt = Some(prompt);
        self
    }

    /// Set progress callback for real-time iteration updates.
    ///
    /// The callback is invoked after each iteration completes (success or failure).
    /// Use this to persist progress to metadata or update UI.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let driver = LoopDriver::new(condition, token)
    ///     .with_progress_callback(|progress| {
    ///         println!("Iteration {}: {} succeeded, {} failed",
    ///             progress.iteration, progress.succeeded, progress.failed);
    ///     });
    /// ```
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(LoopProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Enable context passing with full LLM-based summarization.
    ///
    /// When enabled, each iteration will:
    /// 1. Collect changed files
    /// 2. Generate iteration summary (LLM-based via lazy client creation)
    /// 3. Execute git commit (LLM-generated message)
    /// 4. Inject history into next iteration's prompt
    pub fn with_context_passing(
        mut self,
        base_commit: String,
        initial_prompt: String,
        plan_content: Option<String>,
        cwd: PathBuf,
        summarizer_ctx: SummarizerContext,
    ) -> Self {
        let total = match &self.condition {
            LoopCondition::Iters { count } => *count,
            LoopCondition::Duration { .. } => -1, // Unknown for duration mode
        };

        self.context = Some(LoopContext::new(
            base_commit,
            initial_prompt,
            plan_content,
            total,
        ));
        self.cwd = Some(cwd);
        self.summarizer_ctx = Some(summarizer_ctx);
        self
    }

    /// Enable basic context passing without LLM features.
    ///
    /// When enabled, each iteration will:
    /// 1. Collect changed files
    /// 2. Generate file-based summary (no LLM)
    /// 3. Execute git commit (fallback message)
    /// 4. Inject history into next iteration's prompt
    ///
    /// Use this when ModelClient is not available.
    pub fn with_basic_context_passing(
        mut self,
        base_commit: String,
        initial_prompt: String,
        plan_content: Option<String>,
        cwd: PathBuf,
    ) -> Self {
        let total = match &self.condition {
            LoopCondition::Iters { count } => *count,
            LoopCondition::Duration { .. } => -1, // Unknown for duration mode
        };

        self.context = Some(LoopContext::new(
            base_commit,
            initial_prompt,
            plan_content,
            total,
        ));
        self.cwd = Some(cwd);
        // model_client remains None - will use fallback summary/commit message
        self
    }

    /// Check if context passing is enabled.
    pub fn context_enabled(&self) -> bool {
        self.context.is_some()
    }

    /// Get current context (if enabled).
    pub fn get_context(&self) -> Option<&LoopContext> {
        self.context.as_ref()
    }

    /// Current iteration number (0-indexed).
    pub fn current_iteration(&self) -> i32 {
        self.iteration
    }

    /// Elapsed time since driver started.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get the loop condition.
    pub fn condition(&self) -> &LoopCondition {
        &self.condition
    }

    /// Get the cancellation token.
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Check if loop should continue.
    ///
    /// Returns false if:
    /// - Cancellation token is cancelled
    /// - Iteration count reached (iters mode)
    /// - Duration elapsed (time mode)
    pub fn should_continue(&self) -> bool {
        // 1. Check cancellation first
        if self.cancellation_token.is_cancelled() {
            return false;
        }

        // 2. Check condition
        match &self.condition {
            LoopCondition::Iters { count } => self.iteration < *count,
            LoopCondition::Duration { seconds } => {
                (self.start_time.elapsed().as_secs() as i64) < *seconds
            }
        }
    }

    /// Build query for current iteration.
    pub fn build_query(&self, original: &str) -> String {
        LoopPromptBuilder::build_with_custom(
            original,
            self.iteration,
            self.custom_loop_prompt.as_deref(),
        )
    }

    /// Build prompt with context injection (if enabled).
    fn build_prompt_with_context(&self, original: &str) -> String {
        match &self.context {
            Some(ctx) if self.iteration > 0 => {
                prompt::build_enhanced_prompt(original, self.iteration, ctx)
            }
            _ => self.build_query(original),
        }
    }

    /// Generate a simple file-based summary for an iteration (static version).
    ///
    /// This is a fallback when conversation history is not available.
    /// It generates a summary based on the changed files and success status.
    fn generate_file_based_summary_static(
        _iteration: i32,
        changed_files: &[String],
        success: bool,
    ) -> String {
        let status = if success { "succeeded" } else { "failed" };

        if changed_files.is_empty() {
            return format!("Iteration {status} with no file changes.");
        }

        // Group files by extension
        let mut by_ext: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();
        for file in changed_files {
            let ext = std::path::Path::new(file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("other");
            by_ext.entry(ext).or_default().push(file);
        }

        // Build summary
        let file_count = changed_files.len();
        let ext_summary: Vec<String> = by_ext
            .iter()
            .map(|(ext, files)| format!("{} .{ext} file(s)", files.len()))
            .collect();

        format!(
            "Iteration {status}. Modified {file_count} file(s): {}.",
            ext_summary.join(", ")
        )
    }

    /// Generate a fallback commit message when LLM is not available (static version).
    fn generate_fallback_commit_message_static(iteration: i32, changed_files: &[String]) -> String {
        let file_count = changed_files.len();
        let files_display = if file_count <= 5 {
            changed_files.join(", ")
        } else {
            format!(
                "{}, ... ({} more)",
                changed_files[..5].join(", "),
                file_count - 5
            )
        };

        format!(
            "[iter-{iteration}] Iteration {iteration} changes\n\nModified files: {files_display}"
        )
    }

    /// Mark iteration as complete.
    ///
    /// # Arguments
    /// * `success` - Whether the iteration succeeded
    ///
    /// # Returns
    /// Current progress after this iteration
    pub fn mark_iteration_complete(&mut self, success: bool) -> LoopProgress {
        if !success {
            self.iterations_failed += 1;
            warn!(
                iteration = self.iteration,
                "Iteration failed, continuing to next iteration..."
            );
        } else {
            info!(
                iteration = self.iteration,
                elapsed_secs = self.start_time.elapsed().as_secs(),
                "Iteration succeeded"
            );
        }

        self.iteration += 1;

        let progress = LoopProgress {
            iteration: self.iteration,
            succeeded: self.iteration - self.iterations_failed,
            failed: self.iterations_failed,
            elapsed_seconds: self.start_time.elapsed().as_secs() as i64,
        };

        // Trigger progress callback
        if let Some(ref callback) = self.progress_callback {
            callback(progress.clone());
        }

        progress
    }

    /// Finish the loop and return the result.
    pub fn finish(self) -> LoopResult {
        let result = LoopResult {
            iterations_attempted: self.iteration,
            iterations_succeeded: self.iteration - self.iterations_failed,
            iterations_failed: self.iterations_failed,
            stop_reason: self.determine_stop_reason(),
            elapsed_seconds: self.start_time.elapsed().as_secs() as i64,
        };

        info!(
            attempted = result.iterations_attempted,
            succeeded = result.iterations_succeeded,
            failed = result.iterations_failed,
            elapsed_secs = result.elapsed_seconds,
            reason = ?result.stop_reason,
            "Loop execution complete"
        );

        result
    }

    /// Run task with loop driver.
    ///
    /// Executes codex.submit() in a loop until condition is met.
    /// Uses continue-on-error: if iteration fails, logs and continues.
    ///
    /// # Arguments
    ///
    /// * `codex` - Codex instance to submit queries to
    /// * `original_query` - Original user query (enhanced for iterations > 0)
    /// * `sink` - Optional LogFileSink for event logging
    ///
    /// # Returns
    ///
    /// Loop execution result with iteration count and stop reason.
    pub async fn run_with_loop(
        &mut self,
        codex: &Codex,
        original_query: &str,
        sink: Option<&LogFileSink>,
    ) -> LoopResult {
        info!(
            condition = %self.condition.display(),
            context_enabled = self.context.is_some(),
            "Starting loop execution"
        );

        while self.should_continue() {
            // 1. Build prompt (use context if enabled)
            let query = self.build_prompt_with_context(original_query);
            let input = vec![UserInput::Text { text: query, text_elements: vec![] }];

            if let Some(s) = sink {
                s.log(&format!("Iteration {}: Starting...", self.iteration));
            }

            info!(
                iteration = self.iteration,
                elapsed_secs = self.start_time.elapsed().as_secs(),
                "Starting iteration"
            );

            // 2. Submit via Codex API
            if let Err(e) = codex
                .submit(Op::UserInput {
                    items: input,
                    final_output_json_schema: None,
                    ultrathink_enabled: false,
                })
                .await
            {
                warn!(
                    iteration = self.iteration,
                    error = %e,
                    "Iteration failed to submit, continuing to next iteration..."
                );
                if let Some(s) = sink {
                    s.log(&format!(
                        "Iteration {} failed to submit: {e}",
                        self.iteration
                    ));
                }
                self.iterations_failed += 1;
                self.iteration += 1;
                continue; // Continue-on-error
            }

            // 3. Wait for turn completion, collecting events for summarization
            let (success, collected_items) = self.wait_for_turn_complete(codex, sink).await;

            // 4. Process iteration context if enabled (with collected items for LLM summary)
            if self.context.is_some() {
                self.process_iteration_context(collected_items, success, sink)
                    .await;
            }

            // 5. Update counters
            if success {
                info!(
                    iteration = self.iteration,
                    elapsed_secs = self.start_time.elapsed().as_secs(),
                    "Iteration succeeded"
                );
            } else {
                warn!(
                    iteration = self.iteration,
                    "Iteration task aborted, continuing to next iteration..."
                );
                self.iterations_failed += 1;
            }

            self.iteration += 1;

            // 6. Trigger progress callback
            if let Some(ref callback) = self.progress_callback {
                callback(LoopProgress {
                    iteration: self.iteration,
                    succeeded: self.iteration - self.iterations_failed,
                    failed: self.iterations_failed,
                    elapsed_seconds: self.start_time.elapsed().as_secs() as i64,
                });
            }
        }

        let result = LoopResult {
            iterations_attempted: self.iteration,
            iterations_succeeded: self.iteration - self.iterations_failed,
            iterations_failed: self.iterations_failed,
            stop_reason: self.determine_stop_reason(),
            elapsed_seconds: self.start_time.elapsed().as_secs() as i64,
        };

        info!(
            attempted = result.iterations_attempted,
            succeeded = result.iterations_succeeded,
            failed = result.iterations_failed,
            elapsed_secs = result.elapsed_seconds,
            reason = ?result.stop_reason,
            "Loop execution complete"
        );

        result
    }

    /// Process iteration context (collect changes, generate summary, commit).
    ///
    /// Uses LLM-based summarization when model_client is available and
    /// collected_items is not empty. Falls back to file-based summary otherwise.
    async fn process_iteration_context(
        &mut self,
        collected_items: Vec<ResponseItem>,
        success: bool,
        sink: Option<&LogFileSink>,
    ) {
        // Lazy client creation from summarizer_ctx if needed
        if self.model_client.is_none() && self.cached_summarization_client.is_none() {
            if let Some(ref ctx) = self.summarizer_ctx {
                info!("Creating summarization client lazily");
                self.cached_summarization_client =
                    Some(Arc::new(summarizer::create_summarization_client(ctx)));
            }
        }

        // Extract needed data from context first to avoid borrow conflicts
        let (initial_prompt, cwd, model_client) = {
            let ctx = match self.context.as_ref() {
                Some(c) => c,
                None => return,
            };
            let cwd = match &self.cwd {
                Some(c) => c.clone(),
                None => return,
            };
            // Use model_client if set, otherwise use cached client from summarizer_ctx
            let client = self
                .model_client
                .clone()
                .or_else(|| self.cached_summarization_client.clone());
            (ctx.initial_prompt.clone(), cwd, client)
        };

        // 4a. Get changed files
        let changed_files = git_ops::get_uncommitted_changes(&cwd)
            .await
            .unwrap_or_default();

        // 4b. Generate summary - use LLM if items available, otherwise file-based
        let summary = if let Some(client) = &model_client {
            if !collected_items.is_empty() {
                // Use LLM-based summarization with collected conversation items
                match summarizer::summarize_iteration(&collected_items, client.clone()).await {
                    Ok(llm_summary) => {
                        info!(
                            iteration = self.iteration,
                            "Generated LLM summary for iteration"
                        );
                        llm_summary
                    }
                    Err(e) => {
                        warn!(error = %e, "LLM summary failed, using file-based fallback");
                        Self::generate_file_based_summary_static(
                            self.iteration,
                            &changed_files,
                            success,
                        )
                    }
                }
            } else {
                // No collected items, use file-based summary
                Self::generate_file_based_summary_static(self.iteration, &changed_files, success)
            }
        } else {
            // No model client, use file-based summary
            Self::generate_file_based_summary_static(self.iteration, &changed_files, success)
        };

        if let Some(s) = sink {
            s.log(&format!(
                "Iteration {} summary: {}",
                self.iteration, &summary
            ));
        }

        // 4c. Commit if there are changes
        let commit_id = if !changed_files.is_empty() {
            // Generate commit message
            let commit_msg = if let Some(client) = &model_client {
                // Try LLM-based commit message
                match summarizer::generate_commit_message(
                    self.iteration,
                    &initial_prompt,
                    &changed_files,
                    &summary,
                    client.clone(),
                )
                .await
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!(error = %e, "Failed to generate commit message, using fallback");
                        Self::generate_fallback_commit_message_static(
                            self.iteration,
                            &changed_files,
                        )
                    }
                }
            } else {
                // Use fallback commit message
                Self::generate_fallback_commit_message_static(self.iteration, &changed_files)
            };

            // Execute commit
            match git_ops::commit_if_needed(&cwd, &commit_msg).await {
                Ok(id) => {
                    if let Some(s) = sink {
                        if let Some(ref commit) = id {
                            let short_id = if commit.len() >= 7 {
                                &commit[..7]
                            } else {
                                commit.as_str()
                            };
                            s.log(&format!("Committed: {short_id}"));
                        }
                    }
                    id
                }
                Err(e) => {
                    warn!(error = %e, "Failed to commit");
                    None
                }
            }
        } else {
            None
        };

        // 4d. Record this iteration
        if let Some(ctx) = self.context.as_mut() {
            ctx.add_iteration(IterationRecord::new(
                self.iteration,
                commit_id,
                changed_files,
                summary,
                success,
            ));
        }
    }

    /// Wait for TurnComplete or TurnAborted event, collecting response items.
    ///
    /// Returns (success, collected_items) where:
    /// - success is true if TurnComplete was received, false if aborted or error
    /// - collected_items contains ResponseItems for LLM summarization
    async fn wait_for_turn_complete(
        &self,
        codex: &Codex,
        sink: Option<&LogFileSink>,
    ) -> (bool, Vec<ResponseItem>) {
        let mut collected_items: Vec<ResponseItem> = Vec::new();

        loop {
            // Check cancellation
            if self.cancellation_token.is_cancelled() {
                if let Some(s) = sink {
                    s.log("Cancelled by user");
                }
                return (false, collected_items);
            }

            // Get next event from Codex
            match codex.next_event().await {
                Ok(event) => {
                    // Collect relevant events for summarization
                    if let Some(item) = event_to_response_item(&event.msg) {
                        collected_items.push(item);
                    }

                    // Check for completion events
                    match &event.msg {
                        EventMsg::TurnComplete(_) => {
                            if let Some(s) = sink {
                                s.log("TurnComplete received");
                            }
                            return (true, collected_items);
                        }
                        EventMsg::TurnAborted(aborted) => {
                            if let Some(s) = sink {
                                s.log(&format!("TurnAborted: {:?}", aborted.reason));
                            }
                            return (false, collected_items);
                        }
                        // Continue processing other events
                        _ => continue,
                    }
                }
                Err(e) => {
                    if let Some(s) = sink {
                        s.log(&format!("Error receiving event: {e}"));
                    }
                    warn!(error = %e, "Error receiving event");
                    return (false, collected_items);
                }
            }
        }
    }

    /// Determine why loop stopped.
    fn determine_stop_reason(&self) -> LoopStopReason {
        if self.cancellation_token.is_cancelled() {
            return LoopStopReason::Cancelled;
        }

        match &self.condition {
            LoopCondition::Iters { count } => {
                if self.iteration >= *count {
                    LoopStopReason::Completed
                } else {
                    LoopStopReason::TaskAborted
                }
            }
            LoopCondition::Duration { seconds } => {
                if (self.start_time.elapsed().as_secs() as i64) >= *seconds {
                    LoopStopReason::DurationElapsed
                } else {
                    LoopStopReason::TaskAborted
                }
            }
        }
    }
}

impl std::fmt::Debug for LoopDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopDriver")
            .field("condition", &self.condition)
            .field("iteration", &self.iteration)
            .field("iterations_failed", &self.iterations_failed)
            .field("elapsed_secs", &self.start_time.elapsed().as_secs())
            .finish()
    }
}

/// Convert EventMsg to ResponseItem for summarization.
///
/// Only converts events that are useful for iteration summary:
/// - AgentMessage → assistant message
/// - ExecCommandBegin → function call
/// - ExecCommandEnd → function call output
fn event_to_response_item(msg: &EventMsg) -> Option<ResponseItem> {
    match msg {
        EventMsg::AgentMessage(AgentMessageEvent { message }) => Some(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: message.clone(),
            }],
            end_turn: None,
        }),
        EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id, command, ..
        }) => Some(ResponseItem::FunctionCall {
            id: None,
            call_id: call_id.clone(),
            name: "shell".to_string(),
            arguments: serde_json::to_string(command).unwrap_or_default(),
        }),
        EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id,
            aggregated_output,
            ..
        }) => Some(ResponseItem::FunctionCallOutput {
            call_id: call_id.clone(),
            output: codex_protocol::models::FunctionCallOutputPayload {
                content: aggregated_output.clone(),
                ..Default::default()
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn should_continue_iters() {
        let token = CancellationToken::new();
        let mut driver = LoopDriver::new(LoopCondition::Iters { count: 3 }, token);

        assert!(driver.should_continue()); // iteration 0 < 3
        driver.iteration = 2;
        assert!(driver.should_continue()); // iteration 2 < 3
        driver.iteration = 3;
        assert!(!driver.should_continue()); // iteration 3 >= 3
    }

    #[test]
    fn should_continue_cancelled() {
        let token = CancellationToken::new();
        let driver = LoopDriver::new(LoopCondition::Iters { count: 100 }, token.clone());

        assert!(driver.should_continue());
        token.cancel();
        assert!(!driver.should_continue());
    }

    #[test]
    fn build_query_iterations() {
        let token = CancellationToken::new();
        let mut driver = LoopDriver::new(LoopCondition::Iters { count: 5 }, token);

        let original = "Fix the bug";

        // Iteration 0: unchanged
        assert_eq!(driver.build_query(original), original);

        // Iteration 1+: enhanced
        driver.iteration = 1;
        let enhanced = driver.build_query(original);
        assert!(enhanced.contains(original));
        assert!(enhanced.contains("git log"));
    }

    #[test]
    fn loop_result_tracks_failures() {
        let result = LoopResult {
            iterations_attempted: 5,
            iterations_succeeded: 3,
            iterations_failed: 2,
            stop_reason: LoopStopReason::Completed,
            elapsed_seconds: 100,
        };

        assert_eq!(result.iterations_attempted, 5);
        assert_eq!(result.iterations_succeeded, 3);
        assert_eq!(result.iterations_failed, 2);
    }

    #[test]
    fn mark_iteration_complete() {
        let token = CancellationToken::new();
        let mut driver = LoopDriver::new(LoopCondition::Iters { count: 5 }, token);

        // First iteration succeeds
        let progress = driver.mark_iteration_complete(true);
        assert_eq!(progress.iteration, 1);
        assert_eq!(progress.succeeded, 1);
        assert_eq!(progress.failed, 0);

        // Second iteration fails
        let progress = driver.mark_iteration_complete(false);
        assert_eq!(progress.iteration, 2);
        assert_eq!(progress.succeeded, 1);
        assert_eq!(progress.failed, 1);

        // Third iteration succeeds
        let progress = driver.mark_iteration_complete(true);
        assert_eq!(progress.iteration, 3);
        assert_eq!(progress.succeeded, 2);
        assert_eq!(progress.failed, 1);
    }

    #[test]
    fn finish_returns_correct_result() {
        let token = CancellationToken::new();
        let mut driver = LoopDriver::new(LoopCondition::Iters { count: 3 }, token);

        driver.mark_iteration_complete(true);
        driver.mark_iteration_complete(false);
        driver.mark_iteration_complete(true);

        let result = driver.finish();
        assert_eq!(result.iterations_attempted, 3);
        assert_eq!(result.iterations_succeeded, 2);
        assert_eq!(result.iterations_failed, 1);
        assert_eq!(result.stop_reason, LoopStopReason::Completed);
    }
}

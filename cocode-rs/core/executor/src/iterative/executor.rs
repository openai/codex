//! Iterative agent executor that drives a single agent session through multiple iterations.
//!
//! Implements codex-rs loop_driver style iteration with:
//! - Context passing between iterations
//! - Git operations (uncommitted changes, auto-commit)
//! - Prompt enhancement with history injection
//! - LLM-based or file-based summarization

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::iterative::condition::IterationCondition;
use crate::iterative::context::IterationContext;
use crate::iterative::context::IterationRecord;
use crate::iterative::git_ops;
use crate::iterative::prompt_builder::IterativePromptBuilder;
use crate::iterative::summarizer::CommitMessageFn;
use crate::iterative::summarizer::SummarizeFn;
use crate::iterative::summarizer::generate_commit_message;

/// Input for each iteration callback.
#[derive(Debug, Clone)]
pub struct IterationInput {
    /// Current iteration number (0-based).
    pub iteration: i32,
    /// The enhanced prompt for this iteration.
    pub prompt: String,
    /// Full iteration context.
    pub context: IterationContext,
    /// Working directory for git operations.
    pub cwd: PathBuf,
}

/// Output from each iteration callback.
#[derive(Debug, Clone)]
pub struct IterationOutput {
    /// Result text from the iteration.
    pub result: String,
    /// Whether the iteration succeeded.
    pub success: bool,
}

/// Callback type for executing an agent for one iteration.
///
/// The callback receives an IterationInput with full context and returns IterationOutput.
pub type IterationExecuteFn = Arc<
    dyn Fn(
            IterationInput,
        )
            -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<IterationOutput>> + Send>>
        + Send
        + Sync,
>;

/// Simple callback type for backward compatibility.
///
/// Receives iteration number and prompt, returns result string.
pub type SimpleIterationExecuteFn = Arc<
    dyn Fn(
            i32,    // iteration
            String, // prompt
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
        + Send
        + Sync,
>;

/// Progress information for callback.
#[derive(Debug, Clone)]
pub struct IterationProgress {
    /// Current iteration number (after completion).
    pub iteration: i32,
    /// Number of iterations that succeeded.
    pub succeeded: i32,
    /// Number of iterations that failed.
    pub failed: i32,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: i64,
}

/// Configuration for context passing mode.
#[derive(Debug, Clone)]
pub struct ContextPassingConfig {
    /// Working directory for git operations.
    pub cwd: PathBuf,
    /// Original user prompt.
    pub initial_prompt: String,
    /// Plan content (optional).
    pub plan_content: Option<String>,
    /// Enable automatic git commits.
    pub auto_commit: bool,
    /// Enable complexity assessment prompt injection.
    pub enable_complexity_assessment: bool,
}

impl Default for ContextPassingConfig {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_default(),
            initial_prompt: String::new(),
            plan_content: None,
            auto_commit: true,
            enable_complexity_assessment: true,
        }
    }
}

/// Executor that runs an agent prompt iteratively based on a condition.
///
/// Supports two modes:
/// - Basic mode: Simple iteration with optional callback
/// - Context passing mode: Full codex-rs style with git integration
pub struct IterativeExecutor {
    /// The condition controlling iteration.
    pub condition: IterationCondition,

    /// Maximum number of iterations allowed.
    pub max_iterations: i32,

    /// Optional callback for executing each iteration (full context).
    execute_fn: Option<IterationExecuteFn>,

    /// Optional callback for simple execution (backward compat).
    simple_execute_fn: Option<SimpleIterationExecuteFn>,

    /// Context passing configuration (optional).
    context_config: Option<ContextPassingConfig>,

    /// Iteration context (built during execution).
    context: Option<IterationContext>,

    /// Optional progress callback.
    progress_callback: Option<Box<dyn Fn(IterationProgress) + Send + Sync>>,

    /// Optional LLM summarization callback.
    summarize_fn: Option<SummarizeFn>,

    /// Optional LLM commit message callback.
    commit_msg_fn: Option<CommitMessageFn>,
}

impl IterativeExecutor {
    /// Create a new iterative executor with the given condition.
    pub fn new(condition: IterationCondition) -> Self {
        let max_iterations = match &condition {
            IterationCondition::Count { max } => *max,
            IterationCondition::Duration { .. } => 100,
            IterationCondition::Until { .. } => 50,
        };
        Self {
            condition,
            max_iterations,
            execute_fn: None,
            simple_execute_fn: None,
            context_config: None,
            context: None,
            progress_callback: None,
            summarize_fn: None,
            commit_msg_fn: None,
        }
    }

    /// Set the execution callback for each iteration (full context).
    pub fn with_execute_fn(mut self, f: IterationExecuteFn) -> Self {
        self.execute_fn = Some(f);
        self
    }

    /// Set simple execution callback (backward compatibility).
    pub fn with_simple_execute_fn(mut self, f: SimpleIterationExecuteFn) -> Self {
        self.simple_execute_fn = Some(f);
        self
    }

    /// Enable context passing mode with full git integration.
    pub fn with_context_passing(mut self, config: ContextPassingConfig) -> Self {
        self.context_config = Some(config);
        self
    }

    /// Set progress callback for real-time iteration updates.
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(IterationProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Set LLM summarization callback.
    pub fn with_summarize_fn(mut self, f: SummarizeFn) -> Self {
        self.summarize_fn = Some(f);
        self
    }

    /// Set LLM commit message generation callback.
    pub fn with_commit_msg_fn(mut self, f: CommitMessageFn) -> Self {
        self.commit_msg_fn = Some(f);
        self
    }

    /// Check if context passing is enabled.
    pub fn context_passing_enabled(&self) -> bool {
        self.context_config.is_some()
    }

    /// Get current context (if enabled).
    pub fn get_context(&self) -> Option<&IterationContext> {
        self.context.as_ref()
    }

    /// Execute the prompt iteratively according to the configured condition.
    ///
    /// Returns a record of each iteration including its result and duration.
    pub async fn execute(&mut self, prompt: &str) -> anyhow::Result<Vec<IterationRecord>> {
        tracing::info!(
            condition = ?self.condition,
            max_iterations = self.max_iterations,
            prompt_len = prompt.len(),
            context_passing = self.context_config.is_some(),
            "Starting iterative execution"
        );

        // Initialize context if context passing enabled
        if let Some(config) = &self.context_config {
            let base_commit = git_ops::get_head_commit(&config.cwd)
                .await
                .unwrap_or_else(|_| "unknown".to_string());

            let plan_content = config
                .plan_content
                .clone()
                .or_else(|| git_ops::read_plan_file_if_exists(&config.cwd));

            self.context = Some(IterationContext::with_context_passing(
                base_commit,
                config.initial_prompt.clone(),
                plan_content,
                self.max_iterations,
            ));
        }

        let mut records = Vec::new();
        let start = tokio::time::Instant::now();
        let mut iterations_failed = 0;

        for i in 0..self.max_iterations {
            let iter_start = tokio::time::Instant::now();

            // Check duration condition before executing.
            if let IterationCondition::Duration { max_secs } = &self.condition {
                let elapsed = start.elapsed().as_secs() as i64;
                if elapsed >= *max_secs {
                    tracing::info!(elapsed_secs = elapsed, "Duration limit reached");
                    break;
                }
            }

            // Build prompt for this iteration
            let enhanced_prompt = self.build_prompt(prompt, i);

            // Execute iteration
            let (result, success) = self.execute_iteration(i, enhanced_prompt.clone()).await;

            // Process context (git operations) if enabled
            let (changed_files, commit_id, summary) = if self.context_config.is_some() {
                self.process_iteration_context(i, prompt, success).await
            } else {
                (Vec::new(), None, String::new())
            };

            let duration_ms = iter_start.elapsed().as_millis() as i64;

            // Create iteration record
            let record = IterationRecord::with_git_info(
                i,
                result.clone(),
                duration_ms,
                commit_id,
                changed_files,
                summary,
                success,
            );

            // Add to context if enabled
            if let Some(ctx) = &mut self.context {
                ctx.add_iteration(record.clone());
            }

            records.push(record);

            // Track failures
            if !success {
                iterations_failed += 1;
                tracing::warn!(
                    iteration = i,
                    "Iteration failed, continuing to next iteration..."
                );
            } else {
                tracing::info!(
                    iteration = i,
                    elapsed_secs = start.elapsed().as_secs(),
                    "Iteration succeeded"
                );
            }

            // Trigger progress callback
            if let Some(ref callback) = self.progress_callback {
                callback(IterationProgress {
                    iteration: i + 1,
                    succeeded: (i + 1) - iterations_failed,
                    failed: iterations_failed,
                    elapsed_ms: start.elapsed().as_millis() as i64,
                });
            }

            // Check count condition.
            if let IterationCondition::Count { max } = &self.condition {
                if i + 1 >= *max {
                    break;
                }
            }

            // Check "Until" condition if configured
            if let IterationCondition::Until { check } = &self.condition {
                if result.contains(check) {
                    tracing::info!(
                        iteration = i,
                        check = %check,
                        "Until condition satisfied"
                    );
                    break;
                }
            }
        }

        tracing::info!(
            iterations = records.len(),
            succeeded = records.len() as i32 - iterations_failed,
            failed = iterations_failed,
            total_ms = start.elapsed().as_millis() as i64,
            "Iterative execution complete"
        );

        Ok(records)
    }

    /// Build enhanced prompt for the given iteration.
    fn build_prompt(&self, original: &str, iteration: i32) -> String {
        if let Some(config) = &self.context_config {
            let mut builder = IterativePromptBuilder::new(&config.initial_prompt);
            if !config.enable_complexity_assessment {
                builder = builder.without_complexity_assessment();
            }

            if let Some(ctx) = &self.context {
                builder.build_with_context(iteration, ctx)
            } else {
                builder.build(iteration)
            }
        } else {
            // Basic mode - use original prompt, optionally enhanced
            IterativePromptBuilder::new(original)
                .without_complexity_assessment()
                .build(iteration)
        }
    }

    /// Execute a single iteration.
    async fn execute_iteration(&self, iteration: i32, prompt: String) -> (String, bool) {
        // Try full execute_fn first
        if let Some(execute_fn) = &self.execute_fn {
            let cwd = self
                .context_config
                .as_ref()
                .map(|c| c.cwd.clone())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

            let context = self
                .context
                .clone()
                .unwrap_or_else(|| IterationContext::new(iteration, self.max_iterations));

            let input = IterationInput {
                iteration,
                prompt,
                context,
                cwd,
            };

            match execute_fn(input).await {
                Ok(output) => return (output.result, output.success),
                Err(e) => {
                    tracing::error!(iteration, error = %e, "Iteration failed");
                    return (format!("Iteration {iteration} failed: {e}"), false);
                }
            }
        }

        // Fall back to simple execute_fn
        if let Some(simple_fn) = &self.simple_execute_fn {
            match simple_fn(iteration, prompt).await {
                Ok(result) => return (result, true),
                Err(e) => {
                    tracing::error!(iteration, error = %e, "Iteration failed");
                    return (format!("Iteration {iteration} failed: {e}"), false);
                }
            }
        }

        // No execute function - return stub
        (format!("Iteration {iteration} completed"), true)
    }

    /// Process iteration context: git operations and summarization.
    async fn process_iteration_context(
        &self,
        iteration: i32,
        task: &str,
        success: bool,
    ) -> (Vec<String>, Option<String>, String) {
        let config = match &self.context_config {
            Some(c) => c,
            None => return (Vec::new(), None, String::new()),
        };

        // Get changed files
        let changed_files = git_ops::get_uncommitted_changes(&config.cwd)
            .await
            .unwrap_or_default();

        // Generate summary
        let summary = crate::iterative::summarizer::generate_summary(
            iteration,
            &changed_files,
            task,
            success,
            self.summarize_fn.as_ref(),
        )
        .await;

        // Commit if auto-commit enabled and there are changes
        let commit_id = if config.auto_commit && !changed_files.is_empty() {
            let commit_msg = generate_commit_message(
                iteration,
                task,
                &changed_files,
                &summary,
                self.commit_msg_fn.as_ref(),
            )
            .await;

            match git_ops::commit_if_needed(&config.cwd, &commit_msg).await {
                Ok(id) => {
                    if let Some(ref commit) = id {
                        let short_id = if commit.len() >= 7 {
                            &commit[..7]
                        } else {
                            commit.as_str()
                        };
                        tracing::info!(iteration, commit = short_id, "Committed changes");
                    }
                    id
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to commit");
                    None
                }
            }
        } else {
            None
        };

        (changed_files, commit_id, summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_count_executor() {
        let executor = IterativeExecutor::new(IterationCondition::Count { max: 5 });
        assert_eq!(executor.max_iterations, 5);
        assert!(!executor.context_passing_enabled());
    }

    #[test]
    fn test_new_duration_executor() {
        let executor = IterativeExecutor::new(IterationCondition::Duration { max_secs: 60 });
        assert_eq!(executor.max_iterations, 100);
    }

    #[test]
    fn test_new_until_executor() {
        let executor = IterativeExecutor::new(IterationCondition::Until {
            check: "tests pass".to_string(),
        });
        assert_eq!(executor.max_iterations, 50);
    }

    #[test]
    fn test_context_passing_config() {
        let config = ContextPassingConfig {
            cwd: PathBuf::from("/tmp"),
            initial_prompt: "Fix bugs".to_string(),
            plan_content: Some("Plan content".to_string()),
            auto_commit: true,
            enable_complexity_assessment: false,
        };
        let executor = IterativeExecutor::new(IterationCondition::Count { max: 3 })
            .with_context_passing(config);
        assert!(executor.context_passing_enabled());
    }

    #[tokio::test]
    async fn test_execute_count_basic() {
        let mut executor = IterativeExecutor::new(IterationCondition::Count { max: 3 });
        let records = executor.execute("test prompt").await.expect("execute");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].iteration, 0);
        assert_eq!(records[1].iteration, 1);
        assert_eq!(records[2].iteration, 2);
        // All should succeed with stub
        assert!(records.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn test_execute_with_simple_callback() {
        let callback: SimpleIterationExecuteFn =
            Arc::new(|i, _prompt| Box::pin(async move { Ok(format!("Result for iteration {i}")) }));

        let mut executor = IterativeExecutor::new(IterationCondition::Count { max: 2 })
            .with_simple_execute_fn(callback);

        let records = executor.execute("test").await.expect("execute");
        assert_eq!(records.len(), 2);
        assert!(records[0].result.contains("iteration 0"));
        assert!(records[1].result.contains("iteration 1"));
    }

    #[tokio::test]
    async fn test_execute_with_full_callback() {
        let callback: IterationExecuteFn = Arc::new(|input| {
            Box::pin(async move {
                Ok(IterationOutput {
                    result: format!(
                        "Iteration {} with {} context",
                        input.iteration, input.context.total_iterations
                    ),
                    success: true,
                })
            })
        });

        let mut executor =
            IterativeExecutor::new(IterationCondition::Count { max: 2 }).with_execute_fn(callback);

        let records = executor.execute("test").await.expect("execute");
        assert_eq!(records.len(), 2);
        assert!(records[0].result.contains("Iteration 0"));
    }

    #[tokio::test]
    async fn test_execute_until_condition() {
        let callback: SimpleIterationExecuteFn = Arc::new(|i, _prompt| {
            Box::pin(async move {
                if i == 2 {
                    Ok("tests pass".to_string())
                } else {
                    Ok("still working".to_string())
                }
            })
        });

        let mut executor = IterativeExecutor::new(IterationCondition::Until {
            check: "tests pass".to_string(),
        })
        .with_simple_execute_fn(callback);

        let records = executor.execute("test").await.expect("execute");
        assert_eq!(records.len(), 3); // 0, 1, 2 - stops after finding "tests pass"
        assert!(records[2].result.contains("tests pass"));
    }

    #[tokio::test]
    async fn test_progress_callback() {
        use std::sync::atomic::AtomicI32;
        use std::sync::atomic::Ordering;

        let progress_count = Arc::new(AtomicI32::new(0));
        let progress_count_clone = progress_count.clone();

        let mut executor = IterativeExecutor::new(IterationCondition::Count { max: 3 })
            .with_progress_callback(move |_progress| {
                progress_count_clone.fetch_add(1, Ordering::SeqCst);
            });

        let _ = executor.execute("test").await;
        assert_eq!(progress_count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_iteration_input_fields() {
        let input = IterationInput {
            iteration: 5,
            prompt: "Test".to_string(),
            context: IterationContext::new(5, 10),
            cwd: PathBuf::from("/tmp"),
        };
        assert_eq!(input.iteration, 5);
        assert_eq!(input.prompt, "Test");
        assert_eq!(input.context.iteration, 5);
    }

    #[test]
    fn test_iteration_output_fields() {
        let output = IterationOutput {
            result: "Done".to_string(),
            success: true,
        };
        assert_eq!(output.result, "Done");
        assert!(output.success);
    }
}

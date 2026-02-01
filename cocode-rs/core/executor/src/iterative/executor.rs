use std::pin::Pin;
use std::sync::Arc;

use crate::iterative::condition::IterationCondition;
use crate::iterative::context::IterationRecord;

/// Callback type for executing an agent for one iteration.
///
/// The callback receives:
/// - `iteration`: The current iteration number (0-based)
/// - `prompt`: The task prompt for the iteration
///
/// Returns the iteration result as a string.
pub type IterationExecuteFn = Arc<
    dyn Fn(
            i32,    // iteration
            String, // prompt
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
        + Send
        + Sync,
>;

/// Executor that runs an agent prompt iteratively based on a condition.
pub struct IterativeExecutor {
    /// The condition controlling iteration.
    pub condition: IterationCondition,

    /// Maximum number of iterations allowed.
    pub max_iterations: i32,

    /// Optional callback for executing each iteration.
    execute_fn: Option<IterationExecuteFn>,
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
        }
    }

    /// Set the execution callback for each iteration.
    pub fn with_execute_fn(mut self, f: IterationExecuteFn) -> Self {
        self.execute_fn = Some(f);
        self
    }

    /// Execute the prompt iteratively according to the configured condition.
    ///
    /// Returns a record of each iteration including its result and duration.
    pub async fn execute(&self, prompt: &str) -> anyhow::Result<Vec<IterationRecord>> {
        tracing::info!(
            condition = ?self.condition,
            max_iterations = self.max_iterations,
            prompt_len = prompt.len(),
            "Starting iterative execution"
        );

        let mut records = Vec::new();
        let start = tokio::time::Instant::now();

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

            // Execute actual agent call using callback, or use stub
            let result = if let Some(execute_fn) = &self.execute_fn {
                match execute_fn(i, prompt.to_string()).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(iteration = i, error = %e, "Iteration failed");
                        format!("Iteration {i} failed: {e}")
                    }
                }
            } else {
                // No execute function - return stub
                format!("Iteration {i} result for: {prompt}")
            };

            let duration_ms = iter_start.elapsed().as_millis() as i64;

            records.push(IterationRecord {
                iteration: i,
                result: result.clone(),
                duration_ms,
            });

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
            total_ms = start.elapsed().as_millis() as i64,
            "Iterative execution complete"
        );

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_count_executor() {
        let executor = IterativeExecutor::new(IterationCondition::Count { max: 5 });
        assert_eq!(executor.max_iterations, 5);
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

    #[tokio::test]
    async fn test_execute_count() {
        let executor = IterativeExecutor::new(IterationCondition::Count { max: 3 });
        let records = executor.execute("test prompt").await.expect("execute");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].iteration, 0);
        assert_eq!(records[1].iteration, 1);
        assert_eq!(records[2].iteration, 2);
    }

    #[tokio::test]
    async fn test_execute_results_contain_prompt() {
        let executor = IterativeExecutor::new(IterationCondition::Count { max: 1 });
        let records = executor.execute("find bugs").await.expect("execute");
        assert_eq!(records.len(), 1);
        assert!(records[0].result.contains("find bugs"));
    }
}

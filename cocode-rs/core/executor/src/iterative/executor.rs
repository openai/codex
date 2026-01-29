use crate::iterative::condition::IterationCondition;
use crate::iterative::context::IterationRecord;

/// Executor that runs an agent prompt iteratively based on a condition.
pub struct IterativeExecutor {
    /// The condition controlling iteration.
    pub condition: IterationCondition,

    /// Maximum number of iterations allowed.
    pub max_iterations: i32,
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
        }
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

            // TODO: Execute actual agent call here.
            let result = format!("Iteration {i} result for: {prompt}");
            let duration_ms = iter_start.elapsed().as_millis() as i64;

            records.push(IterationRecord {
                iteration: i,
                result,
                duration_ms,
            });

            // Check count condition.
            if let IterationCondition::Count { max } = &self.condition {
                if i + 1 >= *max {
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

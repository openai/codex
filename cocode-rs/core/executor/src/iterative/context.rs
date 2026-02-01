use serde::Deserialize;
use serde::Serialize;

/// Context available during each iteration of an iterative execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationContext {
    /// Current iteration number (0-based).
    pub iteration: i32,

    /// Total number of planned iterations (may be approximate for
    /// duration/until conditions).
    pub total_iterations: i32,

    /// Results from all previous iterations.
    pub previous_results: Vec<String>,
}

/// Record of a single completed iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationRecord {
    /// Iteration number (0-based).
    pub iteration: i32,

    /// The result text produced by this iteration.
    pub result: String,

    /// Wall-clock duration of this iteration in milliseconds.
    pub duration_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iteration_context() {
        let ctx = IterationContext {
            iteration: 2,
            total_iterations: 5,
            previous_results: vec!["result-0".to_string(), "result-1".to_string()],
        };
        assert_eq!(ctx.iteration, 2);
        assert_eq!(ctx.total_iterations, 5);
        assert_eq!(ctx.previous_results.len(), 2);
    }

    #[test]
    fn test_iteration_record() {
        let record = IterationRecord {
            iteration: 0,
            result: "compiled successfully".to_string(),
            duration_ms: 1500,
        };
        assert_eq!(record.iteration, 0);
        assert_eq!(record.result, "compiled successfully");
        assert_eq!(record.duration_ms, 1500);
    }

    #[test]
    fn test_iteration_record_serde() {
        let record = IterationRecord {
            iteration: 3,
            result: "test passed".to_string(),
            duration_ms: 250,
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let back: IterationRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.iteration, record.iteration);
        assert_eq!(back.result, record.result);
        assert_eq!(back.duration_ms, record.duration_ms);
    }

    #[test]
    fn test_iteration_context_serde() {
        let ctx = IterationContext {
            iteration: 1,
            total_iterations: 10,
            previous_results: vec!["done".to_string()],
        };
        let json = serde_json::to_string(&ctx).expect("serialize");
        let back: IterationContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.iteration, 1);
        assert_eq!(back.total_iterations, 10);
        assert_eq!(back.previous_results, vec!["done"]);
    }
}

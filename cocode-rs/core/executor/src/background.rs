use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Represents a background execution task.
///
/// Background executions run asynchronously and write their output to a file
/// that can be polled or awaited by the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundExecution {
    /// Unique task identifier.
    pub task_id: String,

    /// Path to the file where output will be written.
    pub output_file: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_background_execution() {
        let exec = BackgroundExecution {
            task_id: "task-001".to_string(),
            output_file: PathBuf::from("/tmp/task-001-output.json"),
        };
        assert_eq!(exec.task_id, "task-001");
        assert_eq!(exec.output_file, PathBuf::from("/tmp/task-001-output.json"));
    }

    #[test]
    fn test_background_execution_serde() {
        let exec = BackgroundExecution {
            task_id: "task-002".to_string(),
            output_file: PathBuf::from("/tmp/output.json"),
        };
        let json = serde_json::to_string(&exec).expect("serialize");
        let back: BackgroundExecution = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.task_id, "task-002");
        assert_eq!(back.output_file, PathBuf::from("/tmp/output.json"));
    }
}

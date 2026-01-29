use crate::iterative::context::IterationRecord;

/// Summarises a series of iteration records into a human-readable report.
pub struct Summarizer;

impl Summarizer {
    /// Produce a summary string from a set of iteration records.
    ///
    /// The summary includes the total iteration count, aggregate duration, and
    /// the result of each iteration.
    pub fn summarize_iterations(records: &[IterationRecord]) -> String {
        if records.is_empty() {
            return "No iterations executed.".to_string();
        }

        let total_ms: i64 = records.iter().map(|r| r.duration_ms).sum();
        let count = records.len();

        let mut lines = vec![format!(
            "Completed {count} iteration(s) in {total_ms}ms total."
        )];

        for record in records {
            lines.push(format!(
                "  [{iter}] ({dur}ms): {result}",
                iter = record.iteration,
                dur = record.duration_ms,
                result = record.result
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_empty() {
        let summary = Summarizer::summarize_iterations(&[]);
        assert_eq!(summary, "No iterations executed.");
    }

    #[test]
    fn test_summarize_single() {
        let records = vec![IterationRecord {
            iteration: 0,
            result: "success".to_string(),
            duration_ms: 100,
        }];
        let summary = Summarizer::summarize_iterations(&records);
        assert!(summary.contains("1 iteration(s)"));
        assert!(summary.contains("100ms total"));
        assert!(summary.contains("[0] (100ms): success"));
    }

    #[test]
    fn test_summarize_multiple() {
        let records = vec![
            IterationRecord {
                iteration: 0,
                result: "compiled".to_string(),
                duration_ms: 200,
            },
            IterationRecord {
                iteration: 1,
                result: "tests passed".to_string(),
                duration_ms: 300,
            },
            IterationRecord {
                iteration: 2,
                result: "deployed".to_string(),
                duration_ms: 150,
            },
        ];
        let summary = Summarizer::summarize_iterations(&records);
        assert!(summary.contains("3 iteration(s)"));
        assert!(summary.contains("650ms total"));
        assert!(summary.contains("[0] (200ms): compiled"));
        assert!(summary.contains("[1] (300ms): tests passed"));
        assert!(summary.contains("[2] (150ms): deployed"));
    }
}

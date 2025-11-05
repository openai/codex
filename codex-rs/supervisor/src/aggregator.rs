use crate::types::AggregatedResult;
use crate::types::MergeStrategy;
use crate::types::TaskResult;

/// Aggregate results from multiple tasks according to the merge strategy
pub fn aggregate_results(results: Vec<TaskResult>, strategy: MergeStrategy) -> AggregatedResult {
    match strategy {
        MergeStrategy::Concatenate => concatenate_results(results),
        MergeStrategy::Voting => voting_results(results),
        MergeStrategy::FirstSuccess => first_success_results(results),
        MergeStrategy::HighestScore => highest_score_results(results),
    }
}

fn concatenate_results(results: Vec<TaskResult>) -> AggregatedResult {
    let summary = results
        .iter()
        .map(|r| r.output.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    AggregatedResult {
        summary,
        individual_results: results,
    }
}

fn voting_results(results: Vec<TaskResult>) -> AggregatedResult {
    // Simple voting: count successful results
    let successful = results.iter().filter(|r| r.success).count();
    let total = results.len();

    let summary = format!(
        "Voting complete: {successful}/{total} tasks succeeded.\n\n{}",
        results
            .iter()
            .map(|r| r.output.clone())
            .collect::<Vec<_>>()
            .join("\n")
    );

    AggregatedResult {
        summary,
        individual_results: results,
    }
}

fn first_success_results(results: Vec<TaskResult>) -> AggregatedResult {
    let first_success = results.iter().find(|r| r.success);

    let summary = if let Some(result) = first_success {
        format!("First successful result:\n{}", result.output)
    } else {
        "No successful results found.".to_string()
    };

    AggregatedResult {
        summary,
        individual_results: results,
    }
}

fn highest_score_results(results: Vec<TaskResult>) -> AggregatedResult {
    let best = results.iter().max_by(|a, b| {
        let a_score = a.score.unwrap_or(0.0);
        let b_score = b.score.unwrap_or(0.0);
        a_score
            .partial_cmp(&b_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let summary = if let Some(result) = best {
        format!(
            "Best result (score: {:.2}):\n{}",
            result.score.unwrap_or(0.0),
            result.output
        )
    } else {
        "No results to aggregate.".to_string()
    };

    AggregatedResult {
        summary,
        individual_results: results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_test_results() -> Vec<TaskResult> {
        vec![
            TaskResult {
                step_id: "step-1".to_string(),
                agent_name: "Agent1".to_string(),
                success: true,
                output: "Result 1".to_string(),
                score: Some(0.8),
            },
            TaskResult {
                step_id: "step-2".to_string(),
                agent_name: "Agent2".to_string(),
                success: true,
                output: "Result 2".to_string(),
                score: Some(0.9),
            },
            TaskResult {
                step_id: "step-3".to_string(),
                agent_name: "Agent3".to_string(),
                success: false,
                output: "Failed".to_string(),
                score: Some(0.3),
            },
        ]
    }

    #[test]
    fn test_concatenate_results() {
        let results = create_test_results();
        let aggregated = aggregate_results(results.clone(), MergeStrategy::Concatenate);

        assert!(aggregated.summary.contains("Result 1"));
        assert!(aggregated.summary.contains("Result 2"));
        assert_eq!(aggregated.individual_results.len(), 3);
    }

    #[test]
    fn test_voting_results() {
        let results = create_test_results();
        let aggregated = aggregate_results(results, MergeStrategy::Voting);

        assert!(aggregated.summary.contains("2/3 tasks succeeded"));
    }

    #[test]
    fn test_first_success_results() {
        let results = create_test_results();
        let aggregated = aggregate_results(results, MergeStrategy::FirstSuccess);

        assert!(aggregated.summary.contains("First successful result"));
        assert!(aggregated.summary.contains("Result 1"));
    }

    #[test]
    fn test_highest_score_results() {
        let results = create_test_results();
        let aggregated = aggregate_results(results, MergeStrategy::HighestScore);

        assert!(aggregated.summary.contains("0.90"));
        assert!(aggregated.summary.contains("Result 2"));
    }
}

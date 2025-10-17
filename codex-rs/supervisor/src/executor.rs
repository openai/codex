use crate::types::Assignment;
use crate::types::CoordinationStrategy;
use crate::types::TaskResult;
use anyhow::Result;

/// Execute assignments according to the coordination strategy
pub async fn execute_plan(
    assignments: Vec<Assignment>,
    strategy: CoordinationStrategy,
) -> Result<Vec<TaskResult>> {
    match strategy {
        CoordinationStrategy::Sequential => execute_sequential(assignments).await,
        CoordinationStrategy::Parallel => execute_parallel(assignments).await,
        CoordinationStrategy::Hybrid => execute_hybrid(assignments).await,
    }
}

async fn execute_sequential(assignments: Vec<Assignment>) -> Result<Vec<TaskResult>> {
    let mut results = Vec::new();

    for assignment in assignments {
        let result = execute_single_task(assignment).await?;
        results.push(result);
    }

    Ok(results)
}

async fn execute_parallel(assignments: Vec<Assignment>) -> Result<Vec<TaskResult>> {
    let tasks: Vec<_> = assignments
        .into_iter()
        .map(|assignment| tokio::spawn(execute_single_task(assignment)))
        .collect();

    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await??);
    }

    Ok(results)
}

async fn execute_hybrid(assignments: Vec<Assignment>) -> Result<Vec<TaskResult>> {
    // Simple hybrid: first half sequential, second half parallel
    let mid = assignments.len() / 2;
    let (sequential, parallel) = assignments.split_at(mid);

    let mut results = Vec::new();

    // Execute first half sequentially
    for assignment in sequential {
        results.push(execute_single_task(assignment.clone()).await?);
    }

    // Execute second half in parallel
    let tasks: Vec<_> = parallel
        .iter()
        .map(|assignment| tokio::spawn(execute_single_task(assignment.clone())))
        .collect();

    for task in tasks {
        results.push(task.await??);
    }

    Ok(results)
}

async fn execute_single_task(assignment: Assignment) -> Result<TaskResult> {
    // Mock execution - in the future, this would delegate to actual agents
    // For now, simulate some work
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    Ok(TaskResult {
        step_id: assignment.step_id,
        agent_name: assignment.agent_name.clone(),
        success: true,
        output: format!(
            "Completed: {} by {}",
            assignment.description, assignment.agent_name
        ),
        score: Some(0.9),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_execute_sequential() {
        let assignments = vec![
            Assignment {
                step_id: "step-1".to_string(),
                agent_name: "Agent1".to_string(),
                description: "Task 1".to_string(),
            },
            Assignment {
                step_id: "step-2".to_string(),
                agent_name: "Agent2".to_string(),
                description: "Task 2".to_string(),
            },
        ];

        let results = execute_plan(assignments, CoordinationStrategy::Sequential)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].step_id, "step-1");
        assert_eq!(results[1].step_id, "step-2");
        assert!(results[0].success);
        assert!(results[1].success);
    }

    #[tokio::test]
    async fn test_execute_parallel() {
        let assignments = vec![
            Assignment {
                step_id: "step-1".to_string(),
                agent_name: "Agent1".to_string(),
                description: "Task 1".to_string(),
            },
            Assignment {
                step_id: "step-2".to_string(),
                agent_name: "Agent2".to_string(),
                description: "Task 2".to_string(),
            },
        ];

        let results = execute_plan(assignments, CoordinationStrategy::Parallel)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.step_id == "step-1"));
        assert!(results.iter().any(|r| r.step_id == "step-2"));
    }

    #[tokio::test]
    async fn test_execute_hybrid() {
        let assignments = vec![
            Assignment {
                step_id: "step-1".to_string(),
                agent_name: "Agent1".to_string(),
                description: "Task 1".to_string(),
            },
            Assignment {
                step_id: "step-2".to_string(),
                agent_name: "Agent2".to_string(),
                description: "Task 2".to_string(),
            },
            Assignment {
                step_id: "step-3".to_string(),
                agent_name: "Agent3".to_string(),
                description: "Task 3".to_string(),
            },
        ];

        let results = execute_plan(assignments, CoordinationStrategy::Hybrid)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));
    }
}

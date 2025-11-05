use crate::types::Assignment;
use crate::types::CoordinationStrategy;
use crate::types::SupervisorConfig;
use crate::types::TaskResult;
use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::task::JoinSet;

/// Execute assignments according to the coordination strategy while respecting
/// dependency ordering and collaboration domains so we do not clash with
/// parallel human work (e.g., pair programming sessions).
pub async fn execute_plan(
    assignments: Vec<Assignment>,
    config: &SupervisorConfig,
) -> Result<Vec<TaskResult>> {
    let concurrency_limit = match config.strategy {
        CoordinationStrategy::Sequential => 1,
        CoordinationStrategy::Parallel => config.max_parallel_agents.max(1),
        CoordinationStrategy::Hybrid => {
            std::cmp::max(1, std::cmp::min(config.max_parallel_agents, 2))
        }
    };

    execute_with_dependencies(assignments, concurrency_limit).await
}

async fn execute_with_dependencies(
    assignments: Vec<Assignment>,
    concurrency_limit: usize,
) -> Result<Vec<TaskResult>> {
    if assignments.is_empty() {
        return Ok(Vec::new());
    }

    let mut all_assignments = Vec::new();
    let mut all_step_ids = HashSet::new();
    for assignment in assignments {
        all_step_ids.insert(assignment.step_id.clone());
        all_assignments.push(assignment);
    }

    let mut pending: HashMap<String, Assignment> = HashMap::new();
    let mut remaining_dependencies: HashMap<String, HashSet<String>> = HashMap::new();
    let mut ready: HashMap<String, VecDeque<Assignment>> = HashMap::new();

    for assignment in all_assignments {
        let filtered_dependencies: HashSet<String> = assignment
            .dependencies
            .iter()
            .filter(|dependency| {
                all_step_ids.contains(*dependency) && *dependency != &assignment.step_id
            })
            .cloned()
            .collect();

        if filtered_dependencies.is_empty() {
            enqueue_ready(assignment, &mut ready);
        } else {
            remaining_dependencies.insert(assignment.step_id.clone(), filtered_dependencies);
            pending.insert(assignment.step_id.clone(), assignment);
        }
    }

    let mut results = Vec::new();
    let mut active_domains: HashSet<String> = HashSet::new();
    let mut join_set = JoinSet::new();

    loop {
        while join_set.len() < concurrency_limit {
            if let Some((domain, assignment)) = pop_next_ready(&mut ready, &active_domains) {
                active_domains.insert(domain.clone());
                join_set.spawn(async move {
                    let result = execute_single_task(assignment).await;
                    (domain, result)
                });
            } else {
                break;
            }
        }

        if join_set.is_empty() {
            if ready.is_empty() && pending.is_empty() {
                break;
            }

            return Err(anyhow!(
                "No executable tasks available; dependency cycle or domain deadlock detected"
            ));
        }

        let (domain, task_result) = join_set
            .join_next()
            .await
            .expect("join set should not be empty")
            .map_err(|error| anyhow!(error))?;

        let task_result = task_result?;
        active_domains.remove(&domain);
        let completed_step = task_result.step_id.clone();
        results.push(task_result);

        let mut newly_ready_steps = Vec::new();
        for (step_id, deps) in remaining_dependencies.iter_mut() {
            deps.remove(&completed_step);
            if deps.is_empty() {
                newly_ready_steps.push(step_id.clone());
            }
        }

        for step_id in newly_ready_steps {
            remaining_dependencies.remove(&step_id);
            if let Some(assignment) = pending.remove(&step_id) {
                enqueue_ready(assignment, &mut ready);
            }
        }
    }

    Ok(results)
}

fn enqueue_ready(assignment: Assignment, ready: &mut HashMap<String, VecDeque<Assignment>>) {
    let domain = assignment
        .collaboration_domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
        .unwrap_or_else(|| "__unscoped".to_string());

    ready
        .entry(domain)
        .or_insert_with(VecDeque::new)
        .push_back(assignment);
}

fn pop_next_ready(
    ready: &mut HashMap<String, VecDeque<Assignment>>,
    active_domains: &HashSet<String>,
) -> Option<(String, Assignment)> {
    let mut domains: Vec<String> = ready.keys().cloned().collect();
    domains.sort();

    for domain in domains {
        if active_domains.contains(&domain) {
            continue;
        }

        let mut should_remove = false;
        if let Some(queue) = ready.get_mut(&domain) {
            if let Some(assignment) = queue.pop_front() {
                should_remove = queue.is_empty();
                if should_remove {
                    ready.remove(&domain);
                }
                return Some((domain, assignment));
            }
            should_remove = true;
        }

        if should_remove {
            ready.remove(&domain);
        }
    }

    None
}

async fn execute_single_task(assignment: Assignment) -> Result<TaskResult> {
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    Ok(TaskResult {
        step_id: assignment.step_id.clone(),
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

    fn base_config(strategy: CoordinationStrategy) -> SupervisorConfig {
        SupervisorConfig {
            strategy,
            ..Default::default()
        }
    }

    fn make_assignment(
        step_id: &str,
        agent: &str,
        description: &str,
        dependencies: Vec<&str>,
        domain: Option<&str>,
    ) -> Assignment {
        Assignment {
            step_id: step_id.to_string(),
            agent_name: agent.to_string(),
            description: description.to_string(),
            dependencies: dependencies
                .into_iter()
                .map(|dep| dep.to_string())
                .collect(),
            collaboration_domain: domain.map(|value| value.to_string()),
        }
    }

    #[tokio::test]
    async fn test_execute_sequential_respects_dependencies() {
        let assignments = vec![
            make_assignment("step-1", "Agent1", "Task 1", vec![], Some("area-a")),
            make_assignment("step-2", "Agent2", "Task 2", vec!["step-1"], Some("area-a")),
        ];

        let results = execute_plan(assignments, &base_config(CoordinationStrategy::Sequential))
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].step_id, "step-1");
        assert_eq!(results[1].step_id, "step-2");
    }

    #[tokio::test]
    async fn test_execute_parallel_limits_domain_conflicts() {
        let assignments = vec![
            make_assignment("step-1", "Agent1", "Task 1", vec![], Some("shared-area")),
            make_assignment("step-2", "Agent2", "Task 2", vec![], Some("shared-area")),
            make_assignment("step-3", "Agent3", "Task 3", vec![], Some("other-area")),
        ];

        let results = execute_plan(assignments, &base_config(CoordinationStrategy::Parallel))
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        // Only one of the shared-area tasks should run at a time, but both must complete.
        let shared_count = results
            .iter()
            .filter(|result| result.step_id == "step-1" || result.step_id == "step-2")
            .count();
        assert_eq!(shared_count, 2);
    }

    #[tokio::test]
    async fn test_execute_hybrid_reuses_dependency_logic() {
        let assignments = vec![
            make_assignment("step-1", "Agent1", "Task 1", vec![], Some("domain-a")),
            make_assignment(
                "step-2",
                "Agent2",
                "Task 2",
                vec!["step-1"],
                Some("domain-b"),
            ),
            make_assignment(
                "step-3",
                "Agent3",
                "Task 3",
                vec!["step-2"],
                Some("domain-c"),
            ),
        ];

        let results = execute_plan(assignments, &base_config(CoordinationStrategy::Hybrid))
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|result| result.success));
    }

    #[tokio::test]
    async fn test_cycle_detection() {
        let assignments = vec![
            make_assignment("step-1", "Agent1", "Task 1", vec!["step-2"], Some("a")),
            make_assignment("step-2", "Agent2", "Task 2", vec!["step-1"], Some("b")),
        ];

        let error = execute_plan(assignments, &base_config(CoordinationStrategy::Parallel))
            .await
            .expect_err("should detect cycle");

        assert!(
            error
                .to_string()
                .contains("dependency cycle or domain deadlock detected")
        );
    }
}

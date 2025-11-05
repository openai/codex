use crate::types::Assignment;
use crate::types::Plan;
use anyhow::Result;

/// Assign steps to agents based on hints and available agents
pub fn assign_tasks(plan: &Plan, agents_hint: Option<Vec<String>>) -> Result<Vec<Assignment>> {
    let mut assignments = Vec::new();

    for (index, step) in plan.steps.iter().enumerate() {
        let agent_name = if let Some(ref agents) = agents_hint {
            if agents.is_empty() {
                step.agent_hint
                    .clone()
                    .unwrap_or_else(|| "default".to_string())
            } else if let Some(ref hint) = step.agent_hint {
                let hint_lower = hint.to_lowercase();
                agents
                    .iter()
                    .find(|agent| agent.to_lowercase().contains(&hint_lower))
                    .cloned()
                    .unwrap_or_else(|| agents[index % agents.len()].clone())
            } else {
                agents[index % agents.len()].clone()
            }
        } else {
            // No agents specified, use step hint or default
            step.agent_hint
                .clone()
                .unwrap_or_else(|| "default".to_string())
        };

        assignments.push(Assignment {
            step_id: step.id.clone(),
            agent_name,
            description: step.description.clone(),
            dependencies: step.dependencies.clone(),
            collaboration_domain: step.collaboration_domain.clone(),
        });
    }

    Ok(assignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Step;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_assign_tasks_with_hints() {
        let plan = Plan {
            goal: "Test goal".to_string(),
            steps: vec![
                Step {
                    id: "step-1".to_string(),
                    description: "Backend work".to_string(),
                    agent_hint: Some("Backend".to_string()),
                    dependencies: vec![],
                    collaboration_domain: Some("backend".to_string()),
                },
                Step {
                    id: "step-2".to_string(),
                    description: "Frontend work".to_string(),
                    agent_hint: Some("Frontend".to_string()),
                    dependencies: vec![],
                    collaboration_domain: Some("frontend".to_string()),
                },
            ],
        };

        let agents = Some(vec!["Backend".to_string(), "Frontend".to_string()]);
        let assignments = assign_tasks(&plan, agents).unwrap();

        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].agent_name, "Backend");
        assert_eq!(assignments[1].agent_name, "Frontend");
        assert!(
            assignments
                .iter()
                .all(|assignment| assignment.dependencies.is_empty())
        );
        assert_eq!(
            assignments[0].collaboration_domain.as_deref(),
            Some("backend")
        );
        assert_eq!(
            assignments[1].collaboration_domain.as_deref(),
            Some("frontend")
        );
    }

    #[test]
    fn test_assign_tasks_without_agents() {
        let plan = Plan {
            goal: "Test goal".to_string(),
            steps: vec![Step {
                id: "step-1".to_string(),
                description: "Some work".to_string(),
                agent_hint: Some("Security".to_string()),
                dependencies: vec![],
                collaboration_domain: Some("security".to_string()),
            }],
        };

        let assignments = assign_tasks(&plan, None).unwrap();

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_name, "Security");
        assert!(assignments[0].dependencies.is_empty());
        assert_eq!(
            assignments[0].collaboration_domain.as_deref(),
            Some("security")
        );
    }

    #[test]
    fn test_assign_tasks_fallback() {
        let plan = Plan {
            goal: "Test goal".to_string(),
            steps: vec![Step {
                id: "step-1".to_string(),
                description: "Some work".to_string(),
                agent_hint: None,
                dependencies: vec![],
                collaboration_domain: None,
            }],
        };

        let agents = Some(vec!["Agent1".to_string()]);
        let assignments = assign_tasks(&plan, agents).unwrap();

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_name, "Agent1");
        assert!(assignments[0].dependencies.is_empty());
        assert!(assignments[0].collaboration_domain.is_none());
    }

    #[test]
    fn test_assign_tasks_round_robin_without_hints() {
        let plan = Plan {
            goal: "Round robin".to_string(),
            steps: vec![
                Step {
                    id: "step-1".to_string(),
                    description: "Task 1".to_string(),
                    agent_hint: None,
                    dependencies: vec![],
                    collaboration_domain: None,
                },
                Step {
                    id: "step-2".to_string(),
                    description: "Task 2".to_string(),
                    agent_hint: None,
                    dependencies: vec![],
                    collaboration_domain: None,
                },
                Step {
                    id: "step-3".to_string(),
                    description: "Task 3".to_string(),
                    agent_hint: None,
                    dependencies: vec![],
                    collaboration_domain: None,
                },
            ],
        };

        let agents = Some(vec!["AgentA".to_string(), "AgentB".to_string()]);
        let assignments = assign_tasks(&plan, agents).unwrap();

        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[0].agent_name, "AgentA");
        assert_eq!(assignments[1].agent_name, "AgentB");
        assert_eq!(assignments[2].agent_name, "AgentA");
    }
}

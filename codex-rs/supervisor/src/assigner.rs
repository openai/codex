use crate::types::Assignment;
use crate::types::Plan;
use anyhow::Result;

/// Assign steps to agents based on hints and available agents
pub fn assign_tasks(plan: &Plan, agents_hint: Option<Vec<String>>) -> Result<Vec<Assignment>> {
    let mut assignments = Vec::new();

    for step in &plan.steps {
        let agent_name = if let Some(ref agents) = agents_hint {
            // If agents are specified, try to match based on step hint
            if let Some(ref hint) = step.agent_hint {
                agents
                    .iter()
                    .find(|a| a.to_lowercase().contains(&hint.to_lowercase()))
                    .cloned()
                    .unwrap_or_else(|| agents[0].clone())
            } else {
                // No hint, use first agent
                agents[0].clone()
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
                },
                Step {
                    id: "step-2".to_string(),
                    description: "Frontend work".to_string(),
                    agent_hint: Some("Frontend".to_string()),
                    dependencies: vec![],
                },
            ],
        };

        let agents = Some(vec!["Backend".to_string(), "Frontend".to_string()]);
        let assignments = assign_tasks(&plan, agents).unwrap();

        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].agent_name, "Backend");
        assert_eq!(assignments[1].agent_name, "Frontend");
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
            }],
        };

        let assignments = assign_tasks(&plan, None).unwrap();

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_name, "Security");
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
            }],
        };

        let agents = Some(vec!["Agent1".to_string()]);
        let assignments = assign_tasks(&plan, agents).unwrap();

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_name, "Agent1");
    }
}

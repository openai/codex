use crate::types::Plan;
use crate::types::Step;
use anyhow::Result;

/// Generate a plan from a natural language goal
pub fn analyze_goal(goal: &str) -> Result<Plan> {
    // For now, this is a simple heuristic-based planner
    // In the future, this would call an LLM to generate a detailed plan

    let steps = if goal.to_lowercase().contains("secure auth") {
        vec![
            Step {
                id: "step-1".to_string(),
                description: "Design authentication architecture".to_string(),
                agent_hint: Some("Security".to_string()),
                dependencies: vec![],
            },
            Step {
                id: "step-2".to_string(),
                description: "Implement backend authentication API".to_string(),
                agent_hint: Some("Backend".to_string()),
                dependencies: vec!["step-1".to_string()],
            },
            Step {
                id: "step-3".to_string(),
                description: "Create frontend authentication UI".to_string(),
                agent_hint: Some("Frontend".to_string()),
                dependencies: vec!["step-1".to_string()],
            },
            Step {
                id: "step-4".to_string(),
                description: "Integrate frontend with backend auth".to_string(),
                agent_hint: Some("Frontend".to_string()),
                dependencies: vec!["step-2".to_string(), "step-3".to_string()],
            },
        ]
    } else {
        // Generic plan for unknown goals
        vec![
            Step {
                id: "step-1".to_string(),
                description: format!("Analyze requirements for: {goal}"),
                agent_hint: None,
                dependencies: vec![],
            },
            Step {
                id: "step-2".to_string(),
                description: format!("Implement solution for: {goal}"),
                agent_hint: None,
                dependencies: vec!["step-1".to_string()],
            },
            Step {
                id: "step-3".to_string(),
                description: format!("Test and validate: {goal}"),
                agent_hint: None,
                dependencies: vec!["step-2".to_string()],
            },
        ]
    };

    Ok(Plan {
        goal: goal.to_string(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_analyze_goal_secure_auth() {
        let goal = "Implement secure auth";
        let plan = analyze_goal(goal).unwrap();

        assert_eq!(plan.goal, goal);
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[0].agent_hint.as_deref(), Some("Security"));
        assert_eq!(plan.steps[1].agent_hint.as_deref(), Some("Backend"));
        assert_eq!(plan.steps[2].agent_hint.as_deref(), Some("Frontend"));
    }

    #[test]
    fn test_analyze_goal_generic() {
        let goal = "Build a new feature";
        let plan = analyze_goal(goal).unwrap();

        assert_eq!(plan.goal, goal);
        assert_eq!(plan.steps.len(), 3);
        assert!(plan.steps[0].description.contains("Analyze requirements"));
    }

    #[test]
    fn test_plan_has_dependencies() {
        let plan = analyze_goal("Implement secure auth").unwrap();

        // First step has no dependencies
        assert!(plan.steps[0].dependencies.is_empty());

        // Later steps have dependencies
        assert!(!plan.steps[1].dependencies.is_empty());
        assert!(!plan.steps[2].dependencies.is_empty());
    }
}

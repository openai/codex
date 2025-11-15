//! Plan executor tests

#[cfg(test)]
mod tests {
    use crate::plan::{PlanBlock, PlanState, ExecutionMode, Budget};
    use chrono::Utc;

    #[test]
    fn test_Plan_can_execute() {
        #[allow(non_snake_case)]
        let mut Plan = create_test_Plan();
        
        // Should not be executable in Drafting state
        Plan.state = PlanState::Drafting;
        assert!(!Plan.state.can_execute());
        
        // Should be executable in Approved state
        Plan.state = PlanState::Approved {
            approved_by: "test-user".to_string(),
            approved_at: Utc::now(),
        };
        assert!(Plan.state.can_execute());
    }

    #[test]
    fn test_state_transitions_for_execution() {
        let state = PlanState::Approved {
            approved_by: "test-user".to_string(),
            approved_at: Utc::now(),
        };

        // Approved -> Executing
        let state = state.start_execution("exec-1".to_string()).unwrap();
        assert!(state.is_executing());

        // Executing -> Completed
        let state = state.complete_execution().unwrap();
        assert!(state.is_completed());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_execution_failure_transition() {
        let state = PlanState::Approved {
            approved_by: "test-user".to_string(),
            approved_at: Utc::now(),
        };

        // Approved -> Executing
        let state = state.start_execution("exec-2".to_string()).unwrap();

        // Executing -> Failed
        let state = state.fail_execution("Test error".to_string()).unwrap();
        assert!(state.is_failed());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_invalid_execution_transition() {
        // Can't execute from Drafting
        let state = PlanState::Drafting;
        assert!(state.start_execution("exec-3".to_string()).is_err());

        // Can't complete from Approved
        let state = PlanState::Approved {
            approved_by: "test-user".to_string(),
            approved_at: Utc::now(),
        };
        assert!(state.complete_execution().is_err());
    }

    fn create_test_Plan() -> PlanBlock {
        PlanBlock {
            id: "test-bp-1".to_string(),
            title: "Test Plan".to_string(),
            goal: "Test goal".to_string(),
            assumptions: vec![],
            clarifying_questions: vec![],
            approach: "Test approach".to_string(),
            mode: ExecutionMode::Single,
            work_items: vec![],
            risks: vec![],
            eval: Default::default(),
            budget: Budget::default(),
            rollback: "git reset".to_string(),
            artifacts: vec![],
            research: None,
            state: PlanState::Drafting,
            need_approval: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: Some("test-user".to_string()),
        }
    }
}







//! Plan manager
//!
//! High-level API for creating, updating, approving, and exporting plan.

use super::persist::PlanPersister;
use super::policy::{ApprovalRole, PolicyEnforcer, PrivilegedOperation};
use super::schema::{PlanBlock, ResearchBlock, Risk, WorkItem};
use super::state::{PlanState, StateTransitionError};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use thiserror::Error;

/// Errors from Plan manager operations
#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("Plan not found: {id}")]
    NotFound { id: String },

    #[error("Plan cannot be modified in current state: {state}")]
    CannotModify { state: String },

    #[error("State transition error: {0}")]
    StateTransition(#[from] StateTransitionError),

    #[error("Persistence error: {0}")]
    Persistence(#[from] std::io::Error),

    #[error("Policy violation: {0}")]
    Policy(#[from] super::policy::PolicyError),
}

/// Plan manager for high-level operations
pub struct PlanManager {
    /// In-memory Plan store
    plans: Arc<RwLock<HashMap<String, PlanBlock>>>,

    /// Persister for saving Plans
    persister: PlanPersister,

    /// Policy enforcer
    policy_enforcer: PolicyEnforcer,
}

impl PlanManager {
    /// Create a new Plan manager
    pub fn new() -> Result<Self> {
        Ok(Self {
            plans: Arc::new(RwLock::new(HashMap::new())),
            persister: PlanPersister::new()?,
            policy_enforcer: PolicyEnforcer::default(),
        })
    }

    /// Create a new Plan manager with custom persister and policy
    pub fn with_config(persister: PlanPersister, policy_enforcer: PolicyEnforcer) -> Self {
        Self {
            plans: Arc::new(RwLock::new(HashMap::new())),
            persister,
            policy_enforcer,
        }
    }

    /// Create a new Plan
    pub fn create_Plan(
        &self,
        goal: String,
        title: String,
        created_by: Option<String>,
    ) -> Result<String> {
        let mut bp = PlanBlock::new(goal, title);
        bp.created_by = created_by;
        bp.state = PlanState::Inactive.start_drafting()?;

        let id = bp.id.clone();

        // Store in memory
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(id.clone(), bp.clone());
        }

        // Persist to disk
        self.persister
            .save_json(&bp)
            .context("Failed to persist Plan")?;

        Ok(id)
    }

    /// Get a Plan by ID
    pub fn get_Plan(&self, id: &str) -> Result<PlanBlock> {
        // Try memory first
        {
            let plans = self.plans.read().unwrap();
            if let Some(bp) = plans.get(id) {
                return Ok(bp.clone());
            }
        }

        // Try loading from disk
        let bp = self
            .persister
            .load_json(id)
            .map_err(|_| ManagerError::NotFound { id: id.to_string() })?;

        // Cache in memory
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(id.to_string(), bp.clone());
        }

        Ok(bp)
    }

    /// Update a Plan (creates new version if scope changes)
    pub fn update_Plan(&self, id: &str, update_fn: impl FnOnce(&mut PlanBlock)) -> Result<String> {
        let mut bp = self.get_Plan(id)?;

        // Check if Plan can be modified
        if !bp.state.can_modify() {
            return Err(ManagerError::CannotModify {
                state: bp.state.name().to_string(),
            }
            .into());
        }

        // Apply updates
        update_fn(&mut bp);
        bp.touch();

        // Save
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;

        Ok(bp.id.clone())
    }

    /// Submit Plan for approval
    pub fn submit_for_approval(&self, id: &str) -> Result<()> {
        let mut bp = self.get_Plan(id)?;
        bp.state = bp.state.submit_for_approval()?;
        bp.touch();

        // Save
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Approve a Plan
    pub fn approve_Plan(
        &self,
        id: &str,
        approver: String,
        approver_role: ApprovalRole,
    ) -> Result<()> {
        // Check policy
        self.policy_enforcer
            .enforce(PrivilegedOperation::ShellExec, Some(approver_role), None)?;

        let mut bp = self.get_Plan(id)?;
        bp.state = bp.state.approve(approver)?;
        bp.touch();

        // Save
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Reject a Plan
    pub fn reject_Plan(&self, id: &str, reason: String, rejector: Option<String>) -> Result<()> {
        let mut bp = self.get_Plan(id)?;
        bp.state = bp.state.reject(reason, rejector)?;
        bp.touch();

        // Save
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Supersede a Plan with a new version
    pub fn supersede_Plan(&self, id: &str, new_id: String) -> Result<()> {
        let mut bp = self.get_Plan(id)?;
        bp.state = bp.state.supersede(new_id)?;
        bp.touch();

        // Save
        {
            let mut plans = self.plans.write().unwrap();
            plans.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Export Plan to markdown and JSON
    pub fn export_Plan(&self, id: &str) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
        let bp = self.get_Plan(id)?;
        self.persister.export(&bp).context("Failed to export Plan")
    }

    /// Add work item to Plan
    pub fn add_work_item(&self, id: &str, work_item: WorkItem) -> Result<()> {
        self.update_Plan(id, |bp| {
            bp.add_work_item(work_item);
        })?;
        Ok(())
    }

    /// Add risk to Plan
    pub fn add_risk(&self, id: &str, risk: Risk) -> Result<()> {
        self.update_Plan(id, |bp| {
            bp.add_risk(risk);
        })?;
        Ok(())
    }

    /// Add research results to Plan
    pub fn add_research(&self, id: &str, research: ResearchBlock) -> Result<()> {
        self.update_Plan(id, |bp| {
            bp.set_research(research);
        })?;
        Ok(())
    }

    /// List all Plan IDs
    pub fn list_Plans(&self) -> Result<Vec<String>> {
        self.persister.list_plans().context("Failed to list Plans")
    }

    /// Delete a Plan (soft delete - marks as superseded)
    pub fn delete_Plan(&self, id: &str) -> Result<()> {
        self.supersede_Plan(id, "deleted".to_string())
    }
}

impl Default for PlanManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default PlanManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (PlanManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let markdown_dir = temp_dir.path().join("markdown");
        let json_dir = temp_dir.path().join("json");

        let persister = PlanPersister::with_dirs(markdown_dir, json_dir).unwrap();
        let manager = PlanManager::with_config(persister, PolicyEnforcer::default());

        (manager, temp_dir)
    }

    #[test]
    fn test_create_and_get_Plan() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_Plan(
                "Test goal".to_string(),
                "test-bp".to_string(),
                Some("user1".to_string()),
            )
            .unwrap();

        let bp = manager.get_Plan(&id).unwrap();
        assert_eq!(bp.goal, "Test goal");
        assert!(matches!(bp.state, PlanState::Drafting));
    }

    #[test]
    fn test_approval_flow() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_Plan("Test".to_string(), "test".to_string(), None)
            .unwrap();

        // Submit for approval
        manager.submit_for_approval(&id).unwrap();
        let bp = manager.get_Plan(&id).unwrap();
        assert!(matches!(bp.state, PlanState::Pending { .. }));

        // Approve
        manager
            .approve_Plan(&id, "reviewer".to_string(), ApprovalRole::Maintainer)
            .unwrap();
        let bp = manager.get_Plan(&id).unwrap();
        assert!(matches!(bp.state, PlanState::Approved { .. }));
        assert!(bp.can_execute());
    }

    #[test]
    fn test_rejection_flow() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_Plan("Test".to_string(), "test".to_string(), None)
            .unwrap();

        manager.submit_for_approval(&id).unwrap();

        // Reject
        manager
            .reject_Plan(&id, "Not ready".to_string(), Some("reviewer".to_string()))
            .unwrap();

        let bp = manager.get_Plan(&id).unwrap();
        assert!(matches!(bp.state, PlanState::Rejected { .. }));
    }

    #[test]
    fn test_cannot_modify_approved() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_Plan("Test".to_string(), "test".to_string(), None)
            .unwrap();

        manager.submit_for_approval(&id).unwrap();
        manager
            .approve_Plan(&id, "reviewer".to_string(), ApprovalRole::Maintainer)
            .unwrap();

        // Try to update
        let result = manager.update_Plan(&id, |bp| {
            bp.goal = "Modified goal".to_string();
        });

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().downcast::<ManagerError>().unwrap(),
            ManagerError::CannotModify { .. }
        ));
    }

    #[test]
    fn test_add_work_item() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_Plan("Test".to_string(), "test".to_string(), None)
            .unwrap();

        let work_item = WorkItem {
            name: "Task 1".to_string(),
            files_touched: vec!["file.rs".to_string()],
            diff_contract: "patch".to_string(),
            tests: vec!["test_file".to_string()],
        };

        manager.add_work_item(&id, work_item).unwrap();

        let bp = manager.get_Plan(&id).unwrap();
        assert_eq!(bp.work_items.len(), 1);
        assert_eq!(bp.work_items[0].name, "Task 1");
    }

    #[test]
    fn test_list_Plans() {
        let (manager, _temp) = create_test_manager();

        let id1 = manager
            .create_Plan("Test 1".to_string(), "test-1".to_string(), None)
            .unwrap();
        let id2 = manager
            .create_Plan("Test 2".to_string(), "test-2".to_string(), None)
            .unwrap();

        let ids = manager.list_Plans().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }
}

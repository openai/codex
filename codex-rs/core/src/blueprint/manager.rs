//! Blueprint manager
//!
//! High-level API for creating, updating, approving, and exporting blueprints.

use super::persist::BlueprintPersister;
use super::policy::{ApprovalRole, PolicyEnforcer, PrivilegedOperation};
use super::schema::{BlueprintBlock, ResearchBlock, Risk, WorkItem};
use super::state::{BlueprintState, StateTransitionError};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Errors from blueprint manager operations
#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("Blueprint not found: {id}")]
    NotFound { id: String },

    #[error("Blueprint cannot be modified in current state: {state}")]
    CannotModify { state: String },

    #[error("State transition error: {0}")]
    StateTransition(#[from] StateTransitionError),

    #[error("Persistence error: {0}")]
    Persistence(#[from] std::io::Error),

    #[error("Policy violation: {0}")]
    Policy(#[from] super::policy::PolicyError),
}

/// Blueprint manager for high-level operations
pub struct BlueprintManager {
    /// In-memory blueprint store
    blueprints: Arc<RwLock<HashMap<String, BlueprintBlock>>>,

    /// Persister for saving blueprints
    persister: BlueprintPersister,

    /// Policy enforcer
    policy_enforcer: PolicyEnforcer,
}

impl BlueprintManager {
    /// Create a new blueprint manager
    pub fn new() -> Result<Self> {
        Ok(Self {
            blueprints: Arc::new(RwLock::new(HashMap::new())),
            persister: BlueprintPersister::new()?,
            policy_enforcer: PolicyEnforcer::default(),
        })
    }

    /// Create a new blueprint manager with custom persister and policy
    pub fn with_config(persister: BlueprintPersister, policy_enforcer: PolicyEnforcer) -> Self {
        Self {
            blueprints: Arc::new(RwLock::new(HashMap::new())),
            persister,
            policy_enforcer,
        }
    }

    /// Create a new blueprint
    pub fn create_blueprint(
        &self,
        goal: String,
        title: String,
        created_by: Option<String>,
    ) -> Result<String> {
        let mut bp = BlueprintBlock::new(goal, title);
        bp.created_by = created_by;
        bp.state = BlueprintState::Inactive.start_drafting()?;

        let id = bp.id.clone();

        // Store in memory
        {
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(id.clone(), bp.clone());
        }

        // Persist to disk
        self.persister
            .save_json(&bp)
            .context("Failed to persist blueprint")?;

        Ok(id)
    }

    /// Get a blueprint by ID
    pub fn get_blueprint(&self, id: &str) -> Result<BlueprintBlock> {
        // Try memory first
        {
            let blueprints = self.blueprints.read().unwrap();
            if let Some(bp) = blueprints.get(id) {
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
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(id.to_string(), bp.clone());
        }

        Ok(bp)
    }

    /// Update a blueprint (creates new version if scope changes)
    pub fn update_blueprint(
        &self,
        id: &str,
        update_fn: impl FnOnce(&mut BlueprintBlock),
    ) -> Result<String> {
        let mut bp = self.get_blueprint(id)?;

        // Check if blueprint can be modified
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
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;

        Ok(bp.id.clone())
    }

    /// Submit blueprint for approval
    pub fn submit_for_approval(&self, id: &str) -> Result<()> {
        let mut bp = self.get_blueprint(id)?;
        bp.state = bp.state.submit_for_approval()?;
        bp.touch();

        // Save
        {
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Approve a blueprint
    pub fn approve_blueprint(
        &self,
        id: &str,
        approver: String,
        approver_role: ApprovalRole,
    ) -> Result<()> {
        // Check policy
        self.policy_enforcer
            .enforce(PrivilegedOperation::ShellExec, Some(approver_role), None)?;

        let mut bp = self.get_blueprint(id)?;
        bp.state = bp.state.approve(approver)?;
        bp.touch();

        // Save
        {
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Reject a blueprint
    pub fn reject_blueprint(
        &self,
        id: &str,
        reason: String,
        rejector: Option<String>,
    ) -> Result<()> {
        let mut bp = self.get_blueprint(id)?;
        bp.state = bp.state.reject(reason, rejector)?;
        bp.touch();

        // Save
        {
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Supersede a blueprint with a new version
    pub fn supersede_blueprint(&self, id: &str, new_id: String) -> Result<()> {
        let mut bp = self.get_blueprint(id)?;
        bp.state = bp.state.supersede(new_id)?;
        bp.touch();

        // Save
        {
            let mut blueprints = self.blueprints.write().unwrap();
            blueprints.insert(bp.id.clone(), bp.clone());
        }

        self.persister.save_json(&bp)?;
        Ok(())
    }

    /// Export blueprint to markdown and JSON
    pub fn export_blueprint(&self, id: &str) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
        let bp = self.get_blueprint(id)?;
        self.persister
            .export(&bp)
            .context("Failed to export blueprint")
    }

    /// Add work item to blueprint
    pub fn add_work_item(&self, id: &str, work_item: WorkItem) -> Result<()> {
        self.update_blueprint(id, |bp| {
            bp.add_work_item(work_item);
        })?;
        Ok(())
    }

    /// Add risk to blueprint
    pub fn add_risk(&self, id: &str, risk: Risk) -> Result<()> {
        self.update_blueprint(id, |bp| {
            bp.add_risk(risk);
        })?;
        Ok(())
    }

    /// Add research results to blueprint
    pub fn add_research(&self, id: &str, research: ResearchBlock) -> Result<()> {
        self.update_blueprint(id, |bp| {
            bp.set_research(research);
        })?;
        Ok(())
    }

    /// List all blueprint IDs
    pub fn list_blueprints(&self) -> Result<Vec<String>> {
        self.persister
            .list_blueprints()
            .context("Failed to list blueprints")
    }

    /// Delete a blueprint (soft delete - marks as superseded)
    pub fn delete_blueprint(&self, id: &str) -> Result<()> {
        self.supersede_blueprint(id, "deleted".to_string())
    }
}

impl Default for BlueprintManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default BlueprintManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (BlueprintManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let markdown_dir = temp_dir.path().join("markdown");
        let json_dir = temp_dir.path().join("json");

        let persister = BlueprintPersister::with_dirs(markdown_dir, json_dir).unwrap();
        let manager = BlueprintManager::with_config(persister, PolicyEnforcer::default());

        (manager, temp_dir)
    }

    #[test]
    fn test_create_and_get_blueprint() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_blueprint(
                "Test goal".to_string(),
                "test-bp".to_string(),
                Some("user1".to_string()),
            )
            .unwrap();

        let bp = manager.get_blueprint(&id).unwrap();
        assert_eq!(bp.goal, "Test goal");
        assert!(matches!(bp.state, BlueprintState::Drafting));
    }

    #[test]
    fn test_approval_flow() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_blueprint("Test".to_string(), "test".to_string(), None)
            .unwrap();

        // Submit for approval
        manager.submit_for_approval(&id).unwrap();
        let bp = manager.get_blueprint(&id).unwrap();
        assert!(matches!(bp.state, BlueprintState::Pending { .. }));

        // Approve
        manager
            .approve_blueprint(&id, "reviewer".to_string(), ApprovalRole::Maintainer)
            .unwrap();
        let bp = manager.get_blueprint(&id).unwrap();
        assert!(matches!(bp.state, BlueprintState::Approved { .. }));
        assert!(bp.can_execute());
    }

    #[test]
    fn test_rejection_flow() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_blueprint("Test".to_string(), "test".to_string(), None)
            .unwrap();

        manager.submit_for_approval(&id).unwrap();

        // Reject
        manager
            .reject_blueprint(&id, "Not ready".to_string(), Some("reviewer".to_string()))
            .unwrap();

        let bp = manager.get_blueprint(&id).unwrap();
        assert!(matches!(bp.state, BlueprintState::Rejected { .. }));
    }

    #[test]
    fn test_cannot_modify_approved() {
        let (manager, _temp) = create_test_manager();

        let id = manager
            .create_blueprint("Test".to_string(), "test".to_string(), None)
            .unwrap();

        manager.submit_for_approval(&id).unwrap();
        manager
            .approve_blueprint(&id, "reviewer".to_string(), ApprovalRole::Maintainer)
            .unwrap();

        // Try to update
        let result = manager.update_blueprint(&id, |bp| {
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
            .create_blueprint("Test".to_string(), "test".to_string(), None)
            .unwrap();

        let work_item = WorkItem {
            name: "Task 1".to_string(),
            files_touched: vec!["file.rs".to_string()],
            diff_contract: "patch".to_string(),
            tests: vec!["test_file".to_string()],
        };

        manager.add_work_item(&id, work_item).unwrap();

        let bp = manager.get_blueprint(&id).unwrap();
        assert_eq!(bp.work_items.len(), 1);
        assert_eq!(bp.work_items[0].name, "Task 1");
    }

    #[test]
    fn test_list_blueprints() {
        let (manager, _temp) = create_test_manager();

        let id1 = manager
            .create_blueprint("Test 1".to_string(), "test-1".to_string(), None)
            .unwrap();
        let id2 = manager
            .create_blueprint("Test 2".to_string(), "test-2".to_string(), None)
            .unwrap();

        let ids = manager.list_blueprints().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }
}

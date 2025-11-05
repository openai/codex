//! Blueprint state machine
//!
//! Defines the finite state machine for blueprint lifecycle management.
//! State transitions are strictly controlled to ensure safety and proper approvals.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Blueprint state machine states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BlueprintState {
    /// Blueprint is inactive/not started
    Inactive,
    
    /// Blueprint is being drafted
    Drafting,
    
    /// Blueprint is pending approval
    Pending {
        /// Timestamp when moved to pending
        pending_since: DateTime<Utc>,
    },
    
    /// Blueprint has been approved for execution
    Approved {
        /// Who approved it
        approved_by: String,
        /// When it was approved
        approved_at: DateTime<Utc>,
    },
    
    /// Blueprint was rejected
    Rejected {
        /// Reason for rejection
        reason: String,
        /// Who rejected it
        rejected_by: Option<String>,
        /// When it was rejected
        rejected_at: DateTime<Utc>,
    },
    
    /// Blueprint was superseded by a new version
    Superseded {
        /// ID of the new blueprint
        new_id: String,
        /// When it was superseded
        superseded_at: DateTime<Utc>,
    },
    
    /// Blueprint is currently executing
    Executing {
        /// Execution ID
        execution_id: String,
        /// When execution started
        started_at: DateTime<Utc>,
    },
    
    /// Blueprint execution completed successfully
    Completed {
        /// Execution ID
        execution_id: String,
        /// When execution completed
        completed_at: DateTime<Utc>,
    },
    
    /// Blueprint execution failed
    Failed {
        /// Execution ID
        execution_id: String,
        /// Error message
        error: String,
        /// When execution failed
        failed_at: DateTime<Utc>,
    },
}

impl Default for BlueprintState {
    fn default() -> Self {
        Self::Inactive
    }
}

/// Errors that can occur during state transitions
#[derive(Debug, Error)]
pub enum StateTransitionError {
    #[error("Invalid state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    
    #[error("Blueprint must be in {required} state, but is in {actual}")]
    InvalidState { required: String, actual: String },
    
    #[error("Approval required but not provided")]
    ApprovalRequired,
    
    #[error("Rejection reason required but not provided")]
    ReasonRequired,
}

impl BlueprintState {
    /// Check if state allows execution
    pub fn can_execute(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }
    
    /// Check if state is executing
    pub fn is_executing(&self) -> bool {
        matches!(self, Self::Executing { .. })
    }
    
    /// Check if state is completed
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }
    
    /// Check if state is failed
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
    
    /// Check if state allows modifications
    pub fn can_modify(&self) -> bool {
        matches!(self, Self::Inactive | Self::Drafting)
    }
    
    /// Check if state is terminal (no further transitions)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Rejected { .. } | Self::Superseded { .. } | Self::Completed { .. } | Self::Failed { .. }
        )
    }
    
    /// Get human-readable state name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Drafting => "drafting",
            Self::Pending { .. } => "pending",
            Self::Approved { .. } => "approved",
            Self::Rejected { .. } => "rejected",
            Self::Superseded { .. } => "superseded",
            Self::Executing { .. } => "executing",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
        }
    }
    
    /// Transition to Drafting state
    pub fn start_drafting(self) -> Result<Self, StateTransitionError> {
        match self {
            Self::Inactive => Ok(Self::Drafting),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "drafting".to_string(),
            }),
        }
    }
    
    /// Transition to Pending state
    pub fn submit_for_approval(self) -> Result<Self, StateTransitionError> {
        match self {
            Self::Drafting => Ok(Self::Pending {
                pending_since: Utc::now(),
            }),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "pending".to_string(),
            }),
        }
    }
    
    /// Transition to Approved state
    pub fn approve(self, approver: String) -> Result<Self, StateTransitionError> {
        match self {
            Self::Pending { .. } => Ok(Self::Approved {
                approved_by: approver,
                approved_at: Utc::now(),
            }),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "approved".to_string(),
            }),
        }
    }
    
    /// Transition to Rejected state
    pub fn reject(
        self,
        reason: String,
        rejector: Option<String>,
    ) -> Result<Self, StateTransitionError> {
        if reason.is_empty() {
            return Err(StateTransitionError::ReasonRequired);
        }
        
        match self {
            Self::Pending { .. } | Self::Drafting => Ok(Self::Rejected {
                reason,
                rejected_by: rejector,
                rejected_at: Utc::now(),
            }),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "rejected".to_string(),
            }),
        }
    }
    
    /// Transition to Superseded state
    pub fn supersede(self, new_id: String) -> Result<Self, StateTransitionError> {
        // Can supersede from any non-terminal state
        if self.is_terminal() {
            return Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "superseded".to_string(),
            });
        }
        
        Ok(Self::Superseded {
            new_id,
            superseded_at: Utc::now(),
        })
    }
    
    /// Return to Drafting state (e.g., after rejection)
    pub fn back_to_drafting(self) -> Result<Self, StateTransitionError> {
        match self {
            Self::Rejected { .. } => Ok(Self::Drafting),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "drafting".to_string(),
            }),
        }
    }
    
    /// Transition to Executing state
    pub fn start_execution(self, execution_id: String) -> Result<Self, StateTransitionError> {
        match self {
            Self::Approved { .. } => Ok(Self::Executing {
                execution_id,
                started_at: Utc::now(),
            }),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "executing".to_string(),
            }),
        }
    }
    
    /// Transition to Completed state
    pub fn complete_execution(self) -> Result<Self, StateTransitionError> {
        match self {
            Self::Executing { execution_id, .. } => Ok(Self::Completed {
                execution_id,
                completed_at: Utc::now(),
            }),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "completed".to_string(),
            }),
        }
    }
    
    /// Transition to Failed state
    pub fn fail_execution(self, error: String) -> Result<Self, StateTransitionError> {
        match self {
            Self::Executing { execution_id, .. } => Ok(Self::Failed {
                execution_id,
                error,
                failed_at: Utc::now(),
            }),
            _ => Err(StateTransitionError::InvalidTransition {
                from: self.name().to_string(),
                to: "failed".to_string(),
            }),
        }
    }
}

impl std::fmt::Display for BlueprintState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_state_transitions() {
        // Inactive -> Drafting
        let state = BlueprintState::Inactive;
        let state = state.start_drafting().unwrap();
        assert!(matches!(state, BlueprintState::Drafting));
        
        // Drafting -> Pending
        let state = state.submit_for_approval().unwrap();
        assert!(matches!(state, BlueprintState::Pending { .. }));
        
        // Pending -> Approved
        let state = state.approve("user1".to_string()).unwrap();
        assert!(matches!(state, BlueprintState::Approved { .. }));
        assert!(state.can_execute());
    }
    
    #[test]
    fn test_rejection_flow() {
        let state = BlueprintState::Drafting;
        let state = state.submit_for_approval().unwrap();
        
        // Pending -> Rejected
        let state = state.reject(
            "Not ready yet".to_string(),
            Some("reviewer".to_string()),
        ).unwrap();
        assert!(matches!(state, BlueprintState::Rejected { .. }));
        assert!(state.is_terminal());
        
        // Rejected -> Drafting (rework)
        let state = state.back_to_drafting().unwrap();
        assert!(matches!(state, BlueprintState::Drafting));
    }
    
    #[test]
    fn test_supersede() {
        let state = BlueprintState::Drafting;
        let state = state.supersede("new-bp-id".to_string()).unwrap();
        assert!(matches!(state, BlueprintState::Superseded { .. }));
        assert!(state.is_terminal());
    }
    
    #[test]
    fn test_invalid_transitions() {
        // Can't approve from Drafting
        let state = BlueprintState::Drafting;
        assert!(state.approve("user".to_string()).is_err());
        
        // Can't modify approved blueprint
        let state = BlueprintState::Approved {
            approved_by: "user".to_string(),
            approved_at: Utc::now(),
        };
        assert!(!state.can_modify());
    }
    
    #[test]
    fn test_rejection_requires_reason() {
        let state = BlueprintState::Pending {
            pending_since: Utc::now(),
        };
        
        // Empty reason should fail
        assert!(state.clone().reject("".to_string(), None).is_err());
        
        // Valid reason should succeed
        assert!(state.reject("Valid reason".to_string(), None).is_ok());
    }
}


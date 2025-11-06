//! Blueprint Mode implementation
//!
//! Provides read-only planning phase with approval gates, budget enforcement,
//! and multiple execution strategies (single, orchestrated, competition).

pub mod budget;
pub mod execution_log;
pub mod executor;
pub mod manager;
pub mod persist;
pub mod policy;
pub mod research_integration;
pub mod schema;
pub mod state;

pub use budget::{BudgetError, BudgetTracker, BudgetUsage, format_usage};
pub use executor::{BlueprintExecutor, ExecutionEvent, ExecutionResult, TestResult};
pub use manager::{BlueprintManager, ManagerError};
pub use persist::BlueprintPersister;
pub use policy::{
    ApprovalRole, BlueprintPolicy, PermissionTier, PolicyEnforcer, PolicyError, PrivilegedOperation,
};
pub use research_integration::{ResearchApprovalDialog, ResearchIntegration, ResearchRequest};
pub use schema::{
    BlueprintBlock, Budget, EvalCriteria, ExecutionMode, ResearchBlock, ResearchSource, Risk,
    WorkItem,
};
pub use state::{BlueprintState, StateTransitionError};

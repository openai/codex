//! plan mode implementation
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
pub use executor::{ExecutionEvent, ExecutionResult, PlanExecutor, TestResult};
pub use manager::{ManagerError, PlanManager};
pub use persist::PlanPersister;
pub use policy::{
    ApprovalRole, PermissionTier, PlanPolicy, PolicyEnforcer, PolicyError, PrivilegedOperation,
};
pub use research_integration::{ResearchApprovalDialog, ResearchIntegration, ResearchRequest};
pub use schema::{
    Budget, EvalCriteria, ExecutionMode, PlanBlock, ResearchBlock, ResearchSource, Risk, WorkItem,
};
pub use state::{PlanState, StateTransitionError};

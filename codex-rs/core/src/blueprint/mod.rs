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

pub use budget::BudgetError;
pub use budget::BudgetTracker;
pub use budget::BudgetUsage;
pub use budget::format_usage;
pub use executor::BlueprintExecutor;
pub use executor::ExecutionEvent;
pub use executor::ExecutionResult;
pub use executor::TestResult;
pub use manager::BlueprintManager;
pub use manager::ManagerError;
pub use persist::BlueprintPersister;
pub use policy::ApprovalRole;
pub use policy::BlueprintPolicy;
pub use policy::PermissionTier;
pub use policy::PolicyEnforcer;
pub use policy::PolicyError;
pub use policy::PrivilegedOperation;
pub use research_integration::ResearchApprovalDialog;
pub use research_integration::ResearchIntegration;
pub use research_integration::ResearchRequest;
pub use schema::BlueprintBlock;
pub use schema::Budget;
pub use schema::EvalCriteria;
pub use schema::ExecutionMode;
pub use schema::ResearchBlock;
pub use schema::ResearchSource;
pub use schema::Risk;
pub use schema::WorkItem;
pub use state::BlueprintState;
pub use state::StateTransitionError;

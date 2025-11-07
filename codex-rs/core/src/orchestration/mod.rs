// Auto-orchestration and task analysis
pub mod auto_orchestrator;
pub mod task_analyzer;

// Blueprint orchestration
pub mod plan_orchestrator;

// Collaboration and conflict resolution
pub mod collaboration_store;
pub mod conflict_resolver;

// Error handling
pub mod error_handler;

// Parallel execution and worktree management (v1.3.0)
pub mod parallel_execution;
pub mod resource_manager;
pub mod worktree_manager;

// Re-export common types
pub use auto_orchestrator::{AutoOrchestrator, OrchestratedResult};
pub use plan_orchestrator::PlanOrchestrator;
pub use collaboration_store::CollaborationStore;
pub use conflict_resolver::{ConflictResolver, MergeStrategy};
pub use plan_orchestrator::PlanOrchestrator;
pub use resource_manager::{ResourceCapacity, ResourceGuard, ResourceManager, SystemStats};
pub use task_analyzer::{TaskAnalysis, TaskAnalyzer};

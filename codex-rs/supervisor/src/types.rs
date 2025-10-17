use serde::Deserialize;
use serde::Serialize;

/// Management style for coordinating agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagementStyle {
    /// Single agent makes all decisions
    Autocratic,
    /// Agents collaborate on decisions
    Democratic,
    /// Agents work independently
    LaissezFaire,
}

impl Default for ManagementStyle {
    fn default() -> Self {
        Self::Democratic
    }
}

/// Strategy for coordinating task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoordinationStrategy {
    /// Execute tasks one after another
    Sequential,
    /// Execute all tasks at once
    Parallel,
    /// Mix of sequential and parallel
    Hybrid,
}

impl Default for CoordinationStrategy {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// Strategy for merging results from multiple agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    /// Concatenate all results
    Concatenate,
    /// Use voting mechanism
    Voting,
    /// Take first successful result
    FirstSuccess,
    /// Take result with highest score
    HighestScore,
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::Concatenate
    }
}

/// A single step in the execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub description: String,
    pub agent_hint: Option<String>,
    pub dependencies: Vec<String>,
}

/// Execution plan generated from a goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub goal: String,
    pub steps: Vec<Step>,
}

/// Assignment of a step to a specific agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub step_id: String,
    pub agent_name: String,
    pub description: String,
}

/// Result from executing a single task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub step_id: String,
    pub agent_name: String,
    pub success: bool,
    pub output: String,
    pub score: Option<f64>,
}

/// Aggregated results from multiple tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResult {
    pub summary: String,
    pub individual_results: Vec<TaskResult>,
}

/// Final result from supervisor coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorResult {
    pub goal: String,
    pub plan: Plan,
    pub assignments: Vec<Assignment>,
    pub results: AggregatedResult,
}

/// Configuration for the supervisor
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub strategy: CoordinationStrategy,
    pub style: ManagementStyle,
    pub merge_strategy: MergeStrategy,
    pub max_parallel_agents: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            strategy: CoordinationStrategy::default(),
            style: ManagementStyle::default(),
            merge_strategy: MergeStrategy::default(),
            max_parallel_agents: 5,
        }
    }
}

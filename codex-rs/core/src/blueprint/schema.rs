//! Blueprint schema definitions
//!
//! Defines the core data structures for Blueprint Mode, including the
//! BlueprintBlock which represents a complete planning artifact.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Execution mode for blueprint execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Single agent, no orchestration
    Single,
    /// Orchestrated control with central planner + sub-agents
    Orchestrated,
    /// Worktree competition with multiple variants
    Competition,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Orchestrated
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Single => write!(f, "single"),
            Self::Orchestrated => write!(f, "orchestrated"),
            Self::Competition => write!(f, "competition"),
        }
    }
}

/// A single work item in the blueprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    /// Work item name
    pub name: String,
    /// Files that will be touched
    pub files_touched: Vec<String>,
    /// Diff contract (patch, full, etc.)
    pub diff_contract: String,
    /// Required tests
    pub tests: Vec<String>,
}

/// A risk item with mitigation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    /// Risk description
    pub item: String,
    /// Mitigation strategy
    pub mitigation: String,
}

/// Evaluation criteria for the blueprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCriteria {
    /// Tests that must pass
    pub tests: Vec<String>,
    /// Performance metrics and thresholds
    pub metrics: HashMap<String, String>,
}

impl Default for EvalCriteria {
    fn default() -> Self {
        Self {
            tests: Vec::new(),
            metrics: HashMap::new(),
        }
    }
}

/// Budget constraints for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Token budget per step
    pub max_step: Option<u64>,
    /// Session-wide token cap
    pub session_cap: Option<u64>,
    /// Time estimate in minutes
    pub estimate_min: Option<u64>,
    /// Time cap in minutes
    pub cap_min: Option<u64>,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_step: Some(20000),
            session_cap: Some(100000),
            estimate_min: Some(10),
            cap_min: Some(30),
        }
    }
}

/// Research source from DeepResearch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    /// Source title
    pub title: String,
    /// Source URL
    pub url: String,
    /// Publication/access date
    pub date: String,
    /// Key finding from this source
    pub key_finding: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// Research block appended to blueprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchBlock {
    /// Research query
    pub query: String,
    /// Search depth (1-3)
    pub depth: u8,
    /// Research strategy used
    pub strategy: String,
    /// Collected sources
    pub sources: Vec<ResearchSource>,
    /// Synthesized summary
    pub synthesis: String,
    /// Overall confidence (0.0-1.0)
    pub confidence: f64,
    /// Whether approval was required
    pub needs_approval: bool,
    /// Timestamp of research
    pub timestamp: DateTime<Utc>,
}

/// Complete blueprint block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintBlock {
    /// Unique blueprint ID (timestamp-based)
    pub id: String,

    /// Blueprint title
    pub title: String,

    /// High-level goal
    pub goal: String,

    /// Assumptions made
    pub assumptions: Vec<String>,

    /// Clarifying questions
    pub clarifying_questions: Vec<String>,

    /// Approach description
    pub approach: String,

    /// Execution mode
    pub mode: ExecutionMode,

    /// Work items to complete
    pub work_items: Vec<WorkItem>,

    /// Identified risks
    pub risks: Vec<Risk>,

    /// Evaluation criteria
    pub eval: EvalCriteria,

    /// Budget constraints
    pub budget: Budget,

    /// Rollback plan
    pub rollback: String,

    /// Artifact paths (generated files)
    pub artifacts: Vec<String>,

    /// Optional research results
    pub research: Option<ResearchBlock>,

    /// Current state
    pub state: super::state::BlueprintState,

    /// Whether approval is required
    pub need_approval: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,

    /// User who created the blueprint
    pub created_by: Option<String>,
}

impl BlueprintBlock {
    /// Create a new blueprint from a goal
    pub fn new(goal: String, title: String) -> Self {
        let now = Utc::now();
        let id = format!(
            "{}_{}",
            now.format("%Y-%m-%dT%H:%M:%SZ"),
            title.to_lowercase().replace(' ', "-")
        );

        Self {
            id,
            title,
            goal,
            assumptions: Vec::new(),
            clarifying_questions: Vec::new(),
            approach: String::new(),
            mode: ExecutionMode::default(),
            work_items: Vec::new(),
            risks: Vec::new(),
            eval: EvalCriteria::default(),
            budget: Budget::default(),
            rollback: String::new(),
            artifacts: Vec::new(),
            research: None,
            state: super::state::BlueprintState::Drafting,
            need_approval: true,
            created_at: now,
            updated_at: now,
            created_by: None,
        }
    }

    /// Update the timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if blueprint can be executed
    pub fn can_execute(&self) -> bool {
        matches!(self.state, super::state::BlueprintState::Approved { .. })
    }

    /// Add a work item
    pub fn add_work_item(&mut self, item: WorkItem) {
        self.work_items.push(item);
        self.touch();
    }

    /// Add a risk
    pub fn add_risk(&mut self, risk: Risk) {
        self.risks.push(risk);
        self.touch();
    }

    /// Set research results
    pub fn set_research(&mut self, research: ResearchBlock) {
        self.research = Some(research);
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blueprint_creation() {
        let bp = BlueprintBlock::new("Add telemetry".to_string(), "feat-telemetry".to_string());

        assert!(bp.id.contains("feat-telemetry"));
        assert_eq!(bp.goal, "Add telemetry");
        assert!(matches!(
            bp.state,
            super::super::state::BlueprintState::Drafting
        ));
        assert!(!bp.can_execute());
    }

    #[test]
    fn test_execution_mode_display() {
        assert_eq!(ExecutionMode::Single.to_string(), "single");
        assert_eq!(ExecutionMode::Orchestrated.to_string(), "orchestrated");
        assert_eq!(ExecutionMode::Competition.to_string(), "competition");
    }

    #[test]
    fn test_budget_defaults() {
        let budget = Budget::default();
        assert_eq!(budget.max_step, Some(20000));
        assert_eq!(budget.session_cap, Some(100000));
    }
}

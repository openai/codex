//! DeepResearch integration for plan mode
//!
//! Integrates deep research capabilities with Plan planning phase.

use super::policy::ApprovalRole;
use super::policy::PolicyEnforcer;
use super::policy::PrivilegedOperation;
use super::schema::ResearchBlock;
use super::schema::ResearchSource;
use anyhow::Context;
use anyhow::Result;
use codex_deep_research::DeepResearcher;
use codex_deep_research::DeepResearcherConfig;
use codex_deep_research::ResearchStrategy;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;
use tracing::info;

/// Research request for Plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRequest {
    /// Research query
    pub query: String,

    /// Search depth (1-3)
    pub depth: u8,

    /// Research strategy
    pub strategy: ResearchStrategy,

    /// User requesting research
    pub requester: Option<String>,

    /// User role
    pub requester_role: Option<ApprovalRole>,
}

/// Research approval dialog data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchApprovalDialog {
    /// Query being researched
    pub query: String,

    /// Search depth
    pub depth: u8,

    /// Estimated domains to query
    pub domains: Vec<String>,

    /// Token budget estimate
    pub token_budget: u64,

    /// Time budget estimate in seconds
    pub time_budget_secs: u64,

    /// Data retention policy
    pub data_retention: String,
}

impl ResearchApprovalDialog {
    /// Create approval dialog for a research request
    pub fn from_request(request: &ResearchRequest) -> Self {
        // Estimate domains based on strategy
        let domains = match request.strategy {
            ResearchStrategy::Focused => {
                vec!["duckduckgo.com".to_string(), "github.com".to_string()]
            }
            ResearchStrategy::Comprehensive => vec![
                "duckduckgo.com".to_string(),
                "github.com".to_string(),
                "stackoverflow.com".to_string(),
                "docs.rs".to_string(),
            ],
            ResearchStrategy::Exploratory => vec![
                "duckduckgo.com".to_string(),
                "github.com".to_string(),
                "reddit.com".to_string(),
                "medium.com".to_string(),
                "dev.to".to_string(),
            ],
        };

        // Estimate budgets
        let token_budget = match request.depth {
            1 => 10_000,
            2 => 25_000,
            _ => 50_000,
        };

        let time_budget_secs = match request.strategy {
            ResearchStrategy::Focused => 60,
            ResearchStrategy::Comprehensive => 180,
            ResearchStrategy::Exploratory => 300,
        };

        Self {
            query: request.query.clone(),
            depth: request.depth,
            domains,
            token_budget,
            time_budget_secs,
            data_retention: "Research results stored locally for 30 days, then auto-deleted"
                .to_string(),
        }
    }
}

/// Research integration manager
pub struct ResearchIntegration {
    policy_enforcer: Arc<PolicyEnforcer>,
}

impl ResearchIntegration {
    /// Create a new research integration
    pub fn new(policy_enforcer: Arc<PolicyEnforcer>) -> Self {
        Self { policy_enforcer }
    }

    /// Check if research requires approval
    pub fn requires_approval(&self) -> bool {
        self.policy_enforcer
            .requires_approval(PrivilegedOperation::Network)
    }

    /// Create approval dialog for a research request
    pub fn create_approval_dialog(&self, request: &ResearchRequest) -> ResearchApprovalDialog {
        ResearchApprovalDialog::from_request(request)
    }

    /// Execute research (after approval if required)
    pub async fn execute_research(
        &self,
        request: &ResearchRequest,
        approved: bool,
    ) -> Result<ResearchBlock> {
        // Check policy
        if self.requires_approval() {
            if !approved {
                anyhow::bail!("Research requires approval but was not approved");
            }

            // Verify role if provided
            if let (Some(role), Some(_requester)) = (request.requester_role, &request.requester) {
                self.policy_enforcer
                    .enforce(PrivilegedOperation::Network, Some(role), None)?;
            } else {
                anyhow::bail!("Research approval requires user role information");
            }
        }

        info!("Executing research: {}", request.query);
        debug!("Depth: {}, Strategy: {:?}", request.depth, request.strategy);

        // Configure researcher
        let config = DeepResearcherConfig {
            max_depth: request.depth,
            max_sources: match request.strategy {
                ResearchStrategy::Focused => 5,
                ResearchStrategy::Comprehensive => 10,
                ResearchStrategy::Exploratory => 15,
            },
            strategy: request.strategy,
        };

        // Create provider (use web search provider as default)
        let provider = Arc::new(codex_deep_research::WebSearchProvider::new(3, 30));

        let researcher = DeepResearcher::new(config.clone(), provider);

        // Conduct research
        let report = researcher
            .research(&request.query)
            .await
            .context("Research failed")?;

        // Filter sources by cross-source agreement (≥2 credible sources)
        let filtered_sources = self.filter_by_agreement(&report.sources);

        // Calculate overall confidence from findings
        let overall_confidence = if report.findings.is_empty() {
            0.5
        } else {
            report.findings.iter().map(|f| f.confidence).sum::<f64>() / report.findings.len() as f64
        };

        // Convert to ResearchBlock
        let research_block = ResearchBlock {
            query: request.query.clone(),
            depth: request.depth,
            strategy: format!("{:?}", request.strategy).to_lowercase(),
            sources: filtered_sources
                .iter()
                .map(|s| ResearchSource {
                    title: s.title.clone(),
                    url: s.url.clone(),
                    date: "Unknown".to_string(), // Source doesn't have date field
                    key_finding: s.snippet.chars().take(200).collect(),
                    confidence: s.relevance_score,
                })
                .collect(),
            synthesis: report.summary,
            confidence: overall_confidence,
            needs_approval: self.requires_approval(),
            timestamp: chrono::Utc::now(),
        };

        Ok(research_block)
    }

    /// Filter sources by cross-source agreement
    fn filter_by_agreement(
        &self,
        sources: &[codex_deep_research::types::Source],
    ) -> Vec<codex_deep_research::types::Source> {
        // For now, require relevance_score ≥ 0.7
        // TODO: Implement actual cross-source agreement checking
        sources
            .iter()
            .filter(|s| s.relevance_score >= 0.7)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::policy::PlanPolicy;

    #[test]
    fn test_approval_dialog_creation() {
        let request = ResearchRequest {
            query: "Rust async patterns".to_string(),
            depth: 2,
            strategy: ResearchStrategy::Focused,
            requester: Some("user1".to_string()),
            requester_role: Some(ApprovalRole::Maintainer),
        };

        let dialog = ResearchApprovalDialog::from_request(&request);

        assert_eq!(dialog.query, "Rust async patterns");
        assert_eq!(dialog.depth, 2);
        assert!(!dialog.domains.is_empty());
        assert!(dialog.token_budget > 0);
    }

    #[test]
    fn test_budget_estimates() {
        let request1 = ResearchRequest {
            query: "test".to_string(),
            depth: 1,
            strategy: ResearchStrategy::Focused,
            requester: None,
            requester_role: None,
        };

        let dialog1 = ResearchApprovalDialog::from_request(&request1);
        assert_eq!(dialog1.token_budget, 10_000);

        let request3 = ResearchRequest {
            query: "test".to_string(),
            depth: 3,
            strategy: ResearchStrategy::Comprehensive,
            requester: None,
            requester_role: None,
        };

        let dialog3 = ResearchApprovalDialog::from_request(&request3);
        assert_eq!(dialog3.token_budget, 50_000);
        assert!(dialog3.time_budget_secs > dialog1.time_budget_secs);
    }

    #[test]
    fn test_requires_approval() {
        let policy = PlanPolicy::default();
        let enforcer = Arc::new(PolicyEnforcer::new(policy));
        let integration = ResearchIntegration::new(enforcer);

        assert!(integration.requires_approval());
    }
}

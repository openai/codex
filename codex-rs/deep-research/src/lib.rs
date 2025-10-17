pub mod contradiction;
mod gemini_search_provider;
mod mcp_search_provider;
mod pipeline;
pub mod planner;
pub mod provider;
mod strategies;
pub mod types;
mod url_decoder;
mod web_search_provider;

use anyhow::Result;
use provider::ResearchProvider;
use std::sync::Arc;
use types::ResearchReport;

pub use contradiction::Contradiction;
pub use contradiction::ContradictionChecker;
pub use contradiction::ContradictionReport;
pub use gemini_search_provider::GeminiSearchProvider;
pub use mcp_search_provider::McpSearchProvider;
pub use mcp_search_provider::SearchBackend;
pub use planner::ResearchPlan;
pub use planner::ResearchPlanner;
pub use planner::StopConditions;
pub use provider::MockProvider;
pub use types::DeepResearcherConfig;
pub use types::ResearchStrategy;
pub use web_search_provider::WebSearchProvider;

/// Main deep researcher for conducting comprehensive research
pub struct DeepResearcher {
    config: DeepResearcherConfig,
    provider: Arc<dyn ResearchProvider>,
}

impl DeepResearcher {
    /// Create a new deep researcher with the given configuration and provider
    pub fn new(config: DeepResearcherConfig, provider: Arc<dyn ResearchProvider>) -> Self {
        Self { config, provider }
    }

    /// Conduct research on the given query
    pub async fn research(&self, query: &str) -> Result<ResearchReport> {
        pipeline::conduct_research(Arc::clone(&self.provider), &self.config, query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_deep_researcher() {
        let config = DeepResearcherConfig::default();
        let provider = Arc::new(MockProvider);
        let researcher = DeepResearcher::new(config, provider);

        let report = researcher.research("Rust async patterns").await.unwrap();

        assert_eq!(report.query, "Rust async patterns");
        assert!(!report.sources.is_empty());
        assert!(!report.summary.is_empty());
    }

    #[tokio::test]
    async fn test_deep_researcher_with_custom_config() {
        let config = DeepResearcherConfig {
            max_depth: 2,
            max_sources: 5,
            strategy: ResearchStrategy::Focused,
        };
        let provider = Arc::new(MockProvider);
        let researcher = DeepResearcher::new(config, provider);

        let report = researcher.research("test query").await.unwrap();

        assert_eq!(report.strategy, ResearchStrategy::Focused);
        assert!(report.depth_reached <= 2);
    }
}

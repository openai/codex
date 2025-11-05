use crate::contradiction::ContradictionChecker;
use crate::provider::ResearchProvider;
use crate::strategies::apply_strategy;
use crate::strategies::extract_findings;
use crate::strategies::generate_summary;
use crate::types::ConfidenceLevel;
use crate::types::DeepResearcherConfig;
use crate::types::ResearchReport;
use anyhow::Result;
use std::sync::Arc;

/// Execute the research pipeline
pub async fn conduct_research(
    provider: Arc<dyn ResearchProvider>,
    config: &DeepResearcherConfig,
    query: &str,
) -> Result<ResearchReport> {
    let mut all_sources = Vec::new();
    let mut depth = 0;

    // Multi-depth exploration
    // Note: In the current mock implementation, we only do one iteration.
    // A real implementation would analyze results and generate follow-up queries
    // for deeper exploration across multiple iterations.
    #[allow(clippy::never_loop)]
    while depth < config.max_depth {
        depth += 1;
        let current_query = query.to_string();

        // Search for sources
        let sources = provider.search(&current_query, config.max_sources).await?;

        if sources.is_empty() {
            break;
        }

        all_sources.extend(sources);

        // For mock implementation, we don't do follow-up queries
        // In a real implementation, we would analyze results and generate new queries
        break;
    }

    // Apply strategy to filter and prioritize sources
    let filtered_sources = apply_strategy(all_sources, config.strategy, config.max_sources);

    // Retrieve detailed content from sources
    let mut contents = Vec::new();
    for source in &filtered_sources {
        match provider.retrieve(&source.url).await {
            Ok(content) => contents.push(content),
            Err(_) => contents.push(String::new()),
        }
    }

    // Extract findings from sources
    let findings = extract_findings(&filtered_sources, &contents);

    // Generate summary
    let summary = generate_summary(&findings, query, config.strategy);

    // Check for contradictions
    let contradiction_report = ContradictionChecker::check_contradictions(&findings);

    // Calculate domain diversity
    let diversity_score = ContradictionChecker::calculate_diversity_score(&filtered_sources);

    // Determine confidence level
    let confidence_level = if contradiction_report.contradiction_count > findings.len() / 3 {
        ConfidenceLevel::Low
    } else if diversity_score > 0.6 && contradiction_report.contradiction_count < 2 {
        ConfidenceLevel::High
    } else {
        ConfidenceLevel::Medium
    };

    Ok(ResearchReport {
        query: query.to_string(),
        strategy: config.strategy,
        sources: filtered_sources,
        findings,
        summary,
        depth_reached: depth,
        contradictions: Some(contradiction_report),
        diversity_score,
        confidence_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;
    use crate::types::ResearchStrategy;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_conduct_research() {
        let provider = Arc::new(MockProvider);
        let config = DeepResearcherConfig {
            max_depth: 2,
            max_sources: 5,
            strategy: ResearchStrategy::Comprehensive,
        };

        let report = conduct_research(provider, &config, "Rust async patterns")
            .await
            .unwrap();

        assert_eq!(report.query, "Rust async patterns");
        assert!(report.depth_reached > 0);
        assert!(!report.sources.is_empty());
        assert_eq!(report.findings.len(), report.sources.len());
        assert!(!report.summary.is_empty());
    }

    #[tokio::test]
    async fn test_conduct_research_with_strategies() {
        let strategies = vec![
            ResearchStrategy::Comprehensive,
            ResearchStrategy::Focused,
            ResearchStrategy::Exploratory,
        ];

        for strategy in strategies {
            let provider = Arc::new(MockProvider);
            let config = DeepResearcherConfig {
                max_depth: 1,
                max_sources: 3,
                strategy,
            };

            let report = conduct_research(provider, &config, "test query")
                .await
                .unwrap();

            assert_eq!(report.strategy, strategy);
            assert!(report.sources.len() <= 3);
        }
    }

    #[tokio::test]
    async fn test_conduct_research_respects_max_sources() {
        let provider = Arc::new(MockProvider);
        let config = DeepResearcherConfig {
            max_depth: 1,
            max_sources: 2,
            strategy: ResearchStrategy::Comprehensive,
        };

        let report = conduct_research(provider, &config, "test").await.unwrap();

        assert!(report.sources.len() <= 2);
    }
}

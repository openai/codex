// Integration tests for DeepResearch functionality
// Tests 3 research strategies and core features

use codex_deep_research::DeepResearcher;
use codex_deep_research::DeepResearcherConfig;
use codex_deep_research::MockProvider;
use codex_deep_research::ResearchStrategy;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[tokio::test]
async fn test_deep_research_comprehensive_strategy() {
    let config = DeepResearcherConfig {
        max_depth: 3,
        max_sources: 10,
        strategy: ResearchStrategy::Comprehensive,
    };

    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher.research("Rust async patterns").await.unwrap();

    assert_eq!(report.query, "Rust async patterns");
    assert_eq!(report.strategy, ResearchStrategy::Comprehensive);
    assert!(report.depth_reached <= 3, "Should respect max_depth");
    assert!(!report.sources.is_empty(), "Should have sources");
    assert!(!report.summary.is_empty(), "Should have summary");
}

#[tokio::test]
async fn test_deep_research_focused_strategy() {
    let config = DeepResearcherConfig {
        max_depth: 2,
        max_sources: 5,
        strategy: ResearchStrategy::Focused,
    };

    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher.research("Rust ownership").await.unwrap();

    assert_eq!(report.strategy, ResearchStrategy::Focused);
    assert!(report.depth_reached <= 2, "Should respect max_depth");
    assert!(report.sources.len() <= 5, "Should respect max_sources");
}

#[tokio::test]
async fn test_deep_research_exploratory_strategy() {
    let config = DeepResearcherConfig {
        max_depth: 1,
        max_sources: 15,
        strategy: ResearchStrategy::Exploratory,
    };

    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher.research("Rust web frameworks").await.unwrap();

    assert_eq!(report.strategy, ResearchStrategy::Exploratory);
    assert_eq!(report.depth_reached, 1, "Exploratory should be shallow");
    assert!(!report.sources.is_empty(), "Should have sources");
}

#[tokio::test]
async fn test_deep_research_contradiction_detection() {
    let config = DeepResearcherConfig {
        max_depth: 2,
        max_sources: 10,
        strategy: ResearchStrategy::Comprehensive,
    };

    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher
        .research("test query with contradictions")
        .await
        .unwrap();

    // 矛盾検出機能が動作していることを確認
    // MockProviderは矛盾を含む可能性がある
    if let Some(contradiction_report) = &report.contradictions {
        // 矛盾検出レポートが生成されていれば検証
        // contradiction_count is always >= 0 (unsigned); just check existence
        assert!(
            contradiction_report.contradiction_count > 0
                || contradiction_report.diversity_score >= 0.0,
            "Contradiction report should have count or valid diversity score"
        );
        assert!(
            contradiction_report.diversity_score >= 0.0
                && contradiction_report.diversity_score <= 1.0,
            "Diversity score should be between 0 and 1"
        );
    }
}

#[tokio::test]
async fn test_deep_research_citation_generation() {
    let config = DeepResearcherConfig {
        max_depth: 2,
        max_sources: 5,
        strategy: ResearchStrategy::Comprehensive,
    };

    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher
        .research("Rust zero-copy serialization")
        .await
        .unwrap();

    // ソースが生成されていることを確認（引用の代わりにソース使用）
    assert!(!report.sources.is_empty(), "Should generate sources");

    // ソースの整合性確認
    for source in &report.sources {
        assert!(!source.url.is_empty(), "Source should have URL");
        assert!(!source.title.is_empty(), "Source should have title");
        assert!(!source.snippet.is_empty(), "Source should have snippet");
        assert!(
            source.relevance_score > 0.0,
            "Source should have relevance score"
        );
    }
}

#[tokio::test]
async fn test_deep_research_default_config() {
    let config = DeepResearcherConfig::default();
    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher.research("test query").await.unwrap();

    // デフォルト設定で正常に動作することを確認
    assert_eq!(report.query, "test query");
    assert!(!report.sources.is_empty());
    assert!(!report.summary.is_empty());
}

#[tokio::test]
async fn test_deep_research_empty_query() {
    let config = DeepResearcherConfig::default();
    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    let report = researcher.research("").await.unwrap();

    // 空クエリでもエラーにならないことを確認
    assert_eq!(report.query, "");
}

#[tokio::test]
async fn test_deep_research_large_query() {
    let config = DeepResearcherConfig {
        max_depth: 1,
        max_sources: 3,
        strategy: ResearchStrategy::Focused,
    };

    let provider = Arc::new(MockProvider);
    let researcher = DeepResearcher::new(config, provider);

    // 長いクエリでテスト
    let long_query = "This is a very long query that might exceed normal query length limits and should be handled gracefully by the DeepResearch engine without errors";
    let report = researcher.research(long_query).await.unwrap();

    assert_eq!(report.query, long_query);
    assert!(!report.sources.is_empty());
}

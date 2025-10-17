use crate::types::Source;
use anyhow::Result;
use async_trait::async_trait;

/// Trait for research providers that can search and retrieve information
#[async_trait]
pub trait ResearchProvider: Send + Sync {
    /// Search for sources related to the query
    async fn search(&self, query: &str, max_results: u8) -> Result<Vec<Source>>;

    /// Retrieve detailed content from a source URL
    async fn retrieve(&self, url: &str) -> Result<String>;
}

/// Mock provider for testing
pub struct MockProvider;

#[async_trait]
impl ResearchProvider for MockProvider {
    async fn search(&self, query: &str, max_results: u8) -> Result<Vec<Source>> {
        let count = max_results.min(5) as usize;
        let sources = (0..count)
            .map(|i| Source {
                url: format!("https://example.com/source-{i}"),
                title: format!("Source {i} for: {query}"),
                snippet: format!("This is a snippet about {query} from source {i}"),
                relevance_score: 0.9 - (i as f64 * 0.1),
            })
            .collect();

        Ok(sources)
    }

    async fn retrieve(&self, url: &str) -> Result<String> {
        Ok(format!("Detailed content from {url}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_mock_provider_search() {
        let provider = MockProvider;
        let sources = provider.search("test query", 3).await.unwrap();

        assert_eq!(sources.len(), 3);
        assert!(sources[0].title.contains("test query"));
        assert!(sources[0].relevance_score > sources[1].relevance_score);
    }

    #[tokio::test]
    async fn test_mock_provider_retrieve() {
        let provider = MockProvider;
        let content = provider.retrieve("https://example.com/test").await.unwrap();

        assert!(content.contains("example.com/test"));
    }
}

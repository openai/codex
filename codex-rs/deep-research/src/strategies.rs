use crate::types::Finding;
use crate::types::ResearchStrategy;
use crate::types::Source;

/// Apply research strategy to prioritize and filter sources
pub fn apply_strategy(
    sources: Vec<Source>,
    strategy: ResearchStrategy,
    max_sources: u8,
) -> Vec<Source> {
    let mut filtered = sources;

    match strategy {
        ResearchStrategy::Comprehensive => {
            // Take all sources up to max, prioritize by relevance
            filtered.sort_by(|a, b| {
                b.relevance_score
                    .partial_cmp(&a.relevance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        ResearchStrategy::Focused => {
            // Only take highest relevance sources
            filtered.retain(|s| s.relevance_score >= 0.7);
            filtered.sort_by(|a, b| {
                b.relevance_score
                    .partial_cmp(&a.relevance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        ResearchStrategy::Exploratory => {
            // Mix of high and medium relevance for diversity
            filtered.sort_by(|a, b| {
                b.relevance_score
                    .partial_cmp(&a.relevance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    filtered.truncate(max_sources as usize);
    filtered
}

/// Extract findings from sources
pub fn extract_findings(sources: &[Source], contents: &[String]) -> Vec<Finding> {
    sources
        .iter()
        .zip(contents.iter())
        .map(|(source, content)| Finding {
            content: format!("Finding from {}: {}", source.title, content),
            sources: vec![source.url.clone()],
            confidence: source.relevance_score,
        })
        .collect()
}

/// Generate summary from findings
pub fn generate_summary(findings: &[Finding], query: &str, strategy: ResearchStrategy) -> String {
    let strategy_name = match strategy {
        ResearchStrategy::Comprehensive => "comprehensive",
        ResearchStrategy::Focused => "focused",
        ResearchStrategy::Exploratory => "exploratory",
    };

    format!(
        "Research Summary for \"{query}\" ({strategy_name} strategy):\n\n\
        Found {} relevant findings.\n\n\
        Key findings:\n{}",
        findings.len(),
        findings
            .iter()
            .take(3)
            .map(|f| format!("- {} (confidence: {:.2})", f.content, f.confidence))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_test_sources() -> Vec<Source> {
        vec![
            Source {
                url: "https://example.com/1".to_string(),
                title: "High relevance".to_string(),
                snippet: "snippet 1".to_string(),
                relevance_score: 0.95,
            },
            Source {
                url: "https://example.com/2".to_string(),
                title: "Medium relevance".to_string(),
                snippet: "snippet 2".to_string(),
                relevance_score: 0.75,
            },
            Source {
                url: "https://example.com/3".to_string(),
                title: "Low relevance".to_string(),
                snippet: "snippet 3".to_string(),
                relevance_score: 0.50,
            },
        ]
    }

    #[test]
    fn test_apply_strategy_comprehensive() {
        let sources = create_test_sources();
        let filtered = apply_strategy(sources, ResearchStrategy::Comprehensive, 5);

        assert_eq!(filtered.len(), 3);
        assert!(filtered[0].relevance_score >= filtered[1].relevance_score);
    }

    #[test]
    fn test_apply_strategy_focused() {
        let sources = create_test_sources();
        let filtered = apply_strategy(sources, ResearchStrategy::Focused, 5);

        // Focused filters out low relevance (< 0.7)
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|s| s.relevance_score >= 0.7));
    }

    #[test]
    fn test_extract_findings() {
        let sources = create_test_sources();
        let contents = vec![
            "Content 1".to_string(),
            "Content 2".to_string(),
            "Content 3".to_string(),
        ];

        let findings = extract_findings(&sources, &contents);

        assert_eq!(findings.len(), 3);
        assert!(findings[0].content.contains("Content 1"));
        assert_eq!(findings[0].sources[0], "https://example.com/1");
    }

    #[test]
    fn test_generate_summary() {
        let findings = vec![
            Finding {
                content: "Finding 1".to_string(),
                sources: vec!["url1".to_string()],
                confidence: 0.9,
            },
            Finding {
                content: "Finding 2".to_string(),
                sources: vec!["url2".to_string()],
                confidence: 0.8,
            },
        ];

        let summary = generate_summary(&findings, "test query", ResearchStrategy::Comprehensive);

        assert!(summary.contains("test query"));
        assert!(summary.contains("comprehensive"));
        assert!(summary.contains("2 relevant findings"));
    }
}

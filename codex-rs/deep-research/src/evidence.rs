/// Evidence-based research support with citation management
///
/// Provides standardized Evidence structure for tracking sources,
/// quotes, confidence scores, and reproducibility logging.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Standard evidence structure for research citations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Title of the source
    pub title: String,
    /// URL of the source
    pub url: String,
    /// Publication date (ISO 8601 format, optional)
    pub published: Option<String>,
    /// Relevant quote or excerpt
    pub quote: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

impl Evidence {
    /// Create a new evidence entry
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        quote: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            published: None,
            quote: quote.into(),
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Set publication date
    pub fn with_published_date(mut self, date: impl Into<String>) -> Self {
        self.published = Some(date.into());
        self
    }

    /// Check if this evidence is from a recent source (within days)
    pub fn is_recent(&self, _days: u32) -> bool {
        if let Some(ref published) = self.published {
            // Simple date comparison using string matching
            // For now, just check if it's from 2024 or 2025
            published.contains("2024") || published.contains("2025")
        } else {
            false
        }
    }
}

/// Research log for reproducibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchLog {
    /// Original search query
    pub query: String,
    /// Timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// All evidence sources found
    pub sources: Vec<Evidence>,
    /// Random seed used (if applicable)
    pub seed: Option<u64>,
    /// Search strategy used
    pub strategy: String,
    /// Depth level reached
    pub depth: u8,
    /// Total sources evaluated
    pub total_sources: usize,
    /// Deduplication count
    pub duplicates_removed: usize,
}

impl ResearchLog {
    /// Create a new research log
    pub fn new(query: impl Into<String>, strategy: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            query: query.into(),
            timestamp,
            sources: Vec::new(),
            seed: None,
            strategy: strategy.into(),
            depth: 1,
            total_sources: 0,
            duplicates_removed: 0,
        }
    }

    /// Add evidence to the log
    pub fn add_evidence(&mut self, evidence: Evidence) {
        self.sources.push(evidence);
    }

    /// Set random seed for reproducibility
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set depth level
    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Save research log to `.codex/research/` directory
    pub fn save_to_codex_dir(&self, repo_root: &Path) -> Result<PathBuf> {
        let research_dir = repo_root.join(".codex").join("research");
        std::fs::create_dir_all(&research_dir)?;

        // Generate filename: {timestamp}_{query_hash}.json
        let query_hash = Self::hash_query(&self.query);
        let filename = format!("{}_{:x}.json", self.timestamp, query_hash);
        let file_path = research_dir.join(filename);

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&file_path, json)?;

        Ok(file_path)
    }

    /// Load research log from file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let log: ResearchLog = serde_json::from_str(&json)?;
        Ok(log)
    }

    /// Hash query string for filename generation
    fn hash_query(query: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        hasher.finish()
    }

    /// Get evidence sorted by confidence (descending)
    pub fn evidence_by_confidence(&self) -> Vec<&Evidence> {
        let mut sorted: Vec<&Evidence> = self.sources.iter().collect();
        sorted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    /// Get evidence sorted by date (most recent first)
    pub fn evidence_by_date(&self) -> Vec<&Evidence> {
        let mut dated: Vec<&Evidence> = self.sources.iter()
            .filter(|e| e.published.is_some())
            .collect();
        dated.sort_by(|a, b| {
            b.published.as_ref()
                .cmp(&a.published.as_ref())
        });
        dated
    }

    /// Filter evidence by minimum confidence threshold
    pub fn filter_by_confidence(&self, min_confidence: f64) -> Vec<&Evidence> {
        self.sources
            .iter()
            .filter(|e| e.confidence >= min_confidence)
            .collect()
    }
}

/// Citation builder for inline references
pub struct CitationBuilder {
    evidence_list: Vec<Evidence>,
    citation_map: std::collections::HashMap<String, usize>,
}

impl CitationBuilder {
    /// Create a new citation builder
    pub fn new() -> Self {
        Self {
            evidence_list: Vec::new(),
            citation_map: std::collections::HashMap::new(),
        }
    }

    /// Add evidence and get citation number
    pub fn add_evidence(&mut self, evidence: Evidence) -> usize {
        let key = format!("{}:{}", evidence.url, evidence.quote);
        
        if let Some(&existing_id) = self.citation_map.get(&key) {
            return existing_id;
        }

        let citation_id = self.evidence_list.len() + 1;
        self.evidence_list.push(evidence);
        self.citation_map.insert(key, citation_id);
        citation_id
    }

    /// Generate inline citation marker
    pub fn cite(&mut self, evidence: Evidence) -> String {
        let citation_id = self.add_evidence(evidence);
        format!("[^{}]", citation_id)
    }

    /// Generate citation list (markdown format)
    pub fn generate_citations(&self) -> String {
        let mut output = String::from("\n\n## References\n\n");
        
        for (idx, evidence) in self.evidence_list.iter().enumerate() {
            let citation_num = idx + 1;
            let published = evidence.published.as_deref().unwrap_or("n.d.");
            output.push_str(&format!(
                "[^{}]: {} ({}). [{}]({})\n",
                citation_num,
                evidence.title,
                published,
                evidence.url.chars().take(50).collect::<String>(),
                evidence.url
            ));
            if !evidence.quote.is_empty() {
                output.push_str(&format!("   > \"{}\"\n", evidence.quote));
            }
        }

        output
    }

    /// Get all evidence
    pub fn get_all_evidence(&self) -> &[Evidence] {
        &self.evidence_list
    }
}

impl Default for CitationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_creation() {
        let evidence = Evidence::new(
            "Rust Async Book",
            "https://rust-lang.github.io/async-book/",
            "Async Rust is fundamentally different from sync Rust",
            0.95,
        ).with_published_date("2024");

        assert_eq!(evidence.title, "Rust Async Book");
        assert_eq!(evidence.confidence, 0.95);
        assert_eq!(evidence.published.as_deref(), Some("2024"));
    }

    #[test]
    fn test_research_log_save_and_load() {
        let mut log = ResearchLog::new("test query", "comprehensive");
        log.add_evidence(Evidence::new(
            "Test Source",
            "https://example.com",
            "Test quote",
            0.8,
        ));

        let temp_dir = tempfile::tempdir().unwrap();
        let saved_path = log.save_to_codex_dir(temp_dir.path()).unwrap();

        let loaded = ResearchLog::load_from_file(&saved_path).unwrap();
        assert_eq!(loaded.query, "test query");
        assert_eq!(loaded.sources.len(), 1);
    }

    #[test]
    fn test_citation_builder() {
        let mut builder = CitationBuilder::new();
        
        let evidence1 = Evidence::new(
            "Source 1",
            "https://example.com/1",
            "Quote 1",
            0.9,
        );
        let evidence2 = Evidence::new(
            "Source 2",
            "https://example.com/2",
            "Quote 2",
            0.8,
        );

        let citation1 = builder.cite(evidence1.clone());
        let citation2 = builder.cite(evidence2);
        let citation1_again = builder.cite(evidence1); // Should reuse same ID

        assert_eq!(citation1, "[^1]");
        assert_eq!(citation2, "[^2]");
        assert_eq!(citation1_again, "[^1]"); // Deduplication

        let citations_md = builder.generate_citations();
        assert!(citations_md.contains("[^1]:"));
        assert!(citations_md.contains("[^2]:"));
    }

    #[test]
    fn test_evidence_sorting() {
        let mut log = ResearchLog::new("test", "focused");
        
        log.add_evidence(Evidence::new("Low", "https://a.com", "text", 0.5));
        log.add_evidence(Evidence::new("High", "https://b.com", "text", 0.9));
        log.add_evidence(Evidence::new("Med", "https://c.com", "text", 0.7));

        let by_conf = log.evidence_by_confidence();
        assert_eq!(by_conf[0].title, "High");
        assert_eq!(by_conf[1].title, "Med");
        assert_eq!(by_conf[2].title, "Low");

        let filtered = log.filter_by_confidence(0.7);
        assert_eq!(filtered.len(), 2);
    }
}


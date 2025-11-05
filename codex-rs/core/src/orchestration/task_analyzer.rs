//! Task complexity analysis engine for autonomous orchestration.
//!
//! Analyzes user input to determine if sub-agent orchestration would benefit the task.

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;

/// Task analysis result containing complexity metrics and recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAnalysis {
    /// Complexity score (0.0 = simple, 1.0 = extremely complex)
    pub complexity_score: f64,

    /// Keywords detected in the input
    pub detected_keywords: Vec<String>,

    /// Recommended agents for this task
    pub recommended_agents: Vec<String>,

    /// Decomposed subtasks
    pub subtasks: Vec<String>,

    /// Original user input
    pub original_input: String,
}

impl TaskAnalysis {
    /// Determine if orchestration should be triggered based on complexity.
    pub fn should_orchestrate(&self, threshold: f64) -> bool {
        self.complexity_score > threshold
    }

    /// Get a human-readable summary of the analysis.
    pub fn summary(&self) -> String {
        format!(
            "Complexity: {:.2} | Agents: {} | Subtasks: {}",
            self.complexity_score,
            self.recommended_agents.join(", "),
            self.subtasks.len()
        )
    }
}

/// Task analyzer that evaluates complexity and recommends orchestration strategy.
pub struct TaskAnalyzer {
    _complexity_threshold: f64, // Prefixed with _ to suppress unused warning
}

impl TaskAnalyzer {
    /// Create a new task analyzer with the given complexity threshold.
    pub fn new(_complexity_threshold: f64) -> Self {
        Self {
            _complexity_threshold,
        }
    }

    /// Analyze user input and return task analysis.
    pub fn analyze(&self, user_input: &str) -> TaskAnalysis {
        let complexity_score = self.calculate_complexity(user_input);
        let detected_keywords = self.extract_keywords(user_input);
        let recommended_agents = self.recommend_agents(user_input, &detected_keywords);
        let subtasks = self.decompose_into_subtasks(user_input, &detected_keywords);

        TaskAnalysis {
            complexity_score,
            detected_keywords,
            recommended_agents,
            subtasks,
            original_input: user_input.to_string(),
        }
    }

    /// Calculate complexity score based on multiple factors.
    fn calculate_complexity(&self, input: &str) -> f64 {
        let mut score = 0.0;

        // Factor 1: Word count (longer = more complex)
        let word_count = input.split_whitespace().count();
        let word_score = (word_count as f64 / 50.0).min(0.3);
        score += word_score;

        // Factor 2: Sentence count (multiple sentences = more complex)
        let sentence_count = input.matches(['.', '!', '?']).count().max(1);
        let sentence_score = ((sentence_count - 1) as f64 * 0.15).min(0.2);
        score += sentence_score;

        // Factor 3: Action keywords (more actions = more complex)
        let action_keywords = [
            "implement",
            "create",
            "build",
            "develop",
            "write",
            "add",
            "refactor",
            "migrate",
            "update",
            "fix",
            "review",
            "test",
            "deploy",
            "setup",
            "configure",
            "optimize",
            "analyze",
        ];
        let action_count = action_keywords
            .iter()
            .filter(|&kw| input.to_lowercase().contains(kw))
            .count();
        let action_score = (action_count as f64 * 0.1).min(0.3);
        score += action_score;

        // Factor 4: Domain keywords (multiple domains = more complex)
        let domain_keywords = [
            ("auth", "security", "login", "password", "oauth", "jwt"),
            ("test", "testing", "spec", "unit", "integration", "e2e"),
            ("database", "db", "sql", "migration", "schema", "storage"),
            ("api", "rest", "graphql", "endpoint", "route", "http"),
            ("frontend", "ui", "component", "react", "vue", "angular"),
            (
                "backend",
                "server",
                "service",
                "microservice",
                "grpc",
                "lambda",
            ),
            ("docs", "documentation", "readme", "guide", "wiki", "manual"),
            ("deploy", "deployment", "ci", "cd", "devops", "infra"),
        ];

        let mut detected_domains = HashSet::new();
        let lower_input = input.to_lowercase();
        for domain_group in &domain_keywords {
            let keywords_slice = &[
                domain_group.0,
                domain_group.1,
                domain_group.2,
                domain_group.3,
                domain_group.4,
                domain_group.5,
            ];
            for keyword in keywords_slice {
                if lower_input.contains(keyword) {
                    detected_domains.insert(domain_group);
                    break;
                }
            }
        }
        let domain_score = (detected_domains.len() as f64 * 0.15).min(0.4);
        score += domain_score;

        // Factor 5: Conjunction words (and, with, plus = multiple requirements)
        let conjunction_count = ["and", "with", "plus", "also", "including"]
            .iter()
            .filter(|&conj| lower_input.contains(conj))
            .count();
        let conjunction_score = (conjunction_count as f64 * 0.1).min(0.2);
        score += conjunction_score;

        score.min(1.0)
    }

    /// Extract relevant keywords from the input.
    fn extract_keywords(&self, input: &str) -> Vec<String> {
        let keywords = [
            "implement",
            "create",
            "build",
            "develop",
            "write",
            "add",
            "refactor",
            "migrate",
            "update",
            "fix",
            "review",
            "test",
            "security",
            "auth",
            "authentication",
            "oauth",
            "jwt",
            "database",
            "api",
            "frontend",
            "backend",
            "deploy",
            "documentation",
            "docs",
            "readme",
        ];

        let lower_input = input.to_lowercase();
        keywords
            .iter()
            .filter(|&kw| lower_input.contains(kw))
            .map(|&kw| kw.to_string())
            .collect()
    }

    /// Recommend agents based on detected keywords.
    fn recommend_agents(&self, _input: &str, keywords: &[String]) -> Vec<String> {
        let mut agents = HashSet::new();

        // Security-related
        if keywords
            .iter()
            .any(|k| ["security", "auth", "authentication", "oauth", "jwt"].contains(&k.as_str()))
        {
            agents.insert("sec-audit".to_string());
        }

        // Testing-related
        if keywords
            .iter()
            .any(|k| ["test", "review"].contains(&k.as_str()))
        {
            agents.insert("test-gen".to_string());
        }

        // Code review
        if keywords
            .iter()
            .any(|k| ["refactor", "migrate", "update", "fix", "review"].contains(&k.as_str()))
        {
            agents.insert("code-reviewer".to_string());
        }

        // Research-related
        if keywords
            .iter()
            .any(|k| ["documentation", "docs", "readme", "research"].contains(&k.as_str()))
        {
            agents.insert("researcher".to_string());
        }

        // Default to code-reviewer if no specific agents identified
        if agents.is_empty() {
            agents.insert("code-reviewer".to_string());
        }

        agents.into_iter().collect()
    }

    /// Decompose input into subtasks.
    fn decompose_into_subtasks(&self, input: &str, keywords: &[String]) -> Vec<String> {
        let mut subtasks = Vec::new();

        // Split by common separators
        let parts: Vec<&str> = input
            .split(&[',', ';', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() > 1 {
            // Multiple explicit parts
            subtasks.extend(parts.iter().map(|&s| s.to_string()));
        } else {
            // Infer subtasks from keywords
            if keywords
                .iter()
                .any(|k| k.contains("implement") || k.contains("create"))
            {
                subtasks.push("Implement core functionality".to_string());
            }
            if keywords.iter().any(|k| k.contains("test")) {
                subtasks.push("Write comprehensive tests".to_string());
            }
            if keywords
                .iter()
                .any(|k| k.contains("security") || k.contains("auth"))
            {
                subtasks.push("Security review and validation".to_string());
            }
            if keywords
                .iter()
                .any(|k| k.contains("docs") || k.contains("documentation"))
            {
                subtasks.push("Update documentation".to_string());
            }

            // If no specific subtasks identified, use the whole input
            if subtasks.is_empty() {
                subtasks.push(input.to_string());
            }
        }

        subtasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_task_low_complexity() {
        let analyzer = TaskAnalyzer::new(0.7);
        let analysis = analyzer.analyze("Fix typo in README");

        assert!(analysis.complexity_score < 0.5);
        assert!(!analysis.should_orchestrate(0.7));
    }

    #[test]
    fn test_complex_task_high_complexity() {
        let analyzer = TaskAnalyzer::new(0.7);
        let analysis = analyzer.analyze(
            "Implement user authentication with JWT, write unit tests, \
             perform security review, and update documentation",
        );

        assert!(analysis.complexity_score > 0.7);
        assert!(analysis.should_orchestrate(0.7));
        assert!(
            analysis
                .recommended_agents
                .contains(&"sec-audit".to_string())
        );
        assert!(
            analysis
                .recommended_agents
                .contains(&"test-gen".to_string())
        );
    }

    #[test]
    fn test_keyword_extraction() {
        let analyzer = TaskAnalyzer::new(0.7);
        let analysis = analyzer.analyze("Implement OAuth authentication and write tests");

        assert!(
            analysis
                .detected_keywords
                .contains(&"implement".to_string())
        );
        assert!(analysis.detected_keywords.contains(&"auth".to_string()));
        assert!(analysis.detected_keywords.contains(&"test".to_string()));
    }

    #[test]
    fn test_agent_recommendation() {
        let analyzer = TaskAnalyzer::new(0.7);

        // Security task
        let security_analysis = analyzer.analyze("Review security vulnerabilities");
        assert!(
            security_analysis
                .recommended_agents
                .contains(&"sec-audit".to_string())
        );

        // Testing task
        let test_analysis = analyzer.analyze("Write unit tests for the API");
        assert!(
            test_analysis
                .recommended_agents
                .contains(&"test-gen".to_string())
        );
    }

    #[test]
    fn test_subtask_decomposition() {
        let analyzer = TaskAnalyzer::new(0.7);
        let analysis = analyzer.analyze("Implement feature, write tests, update docs");

        assert!(analysis.subtasks.len() >= 3);
    }
}

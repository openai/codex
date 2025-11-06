//! AI client for code analysis using local models

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::analyzer::{Issue, IssueSeverity, Suggestion, SuggestionType, Impact};
use crate::config::LocalAIConfig;

/// AI client for code analysis
pub struct AIClient {
    config: LocalAIConfig,
    client: reqwest::Client,
}

impl AIClient {
    /// Create a new AI client
    pub fn new(config: LocalAIConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        Ok(Self { config, client })
    }

    /// Analyze code using AI
    pub async fn analyze_code(
        &self,
        file_path: &Path,
        content: &str,
        language: &str,
    ) -> Result<AIAnalysisResult> {
        info!("Analyzing {:?} with AI model: {}", file_path, self.config.review_model);

        let prompt = self.create_analysis_prompt(file_path, content, language);

        let response = self.call_ollama(&self.config.review_model, &prompt).await?;

        let result = self.parse_analysis_response(&response)?;

        Ok(result)
    }

    /// Generate code improvements using AI
    pub async fn suggest_improvements(
        &self,
        file_path: &Path,
        content: &str,
        issues: &[Issue],
    ) -> Result<Vec<Suggestion>> {
        info!("Generating improvement suggestions for {:?}", file_path);

        let prompt = self.create_improvement_prompt(file_path, content, issues);

        let response = self.call_ollama(&self.config.coding_model, &prompt).await?;

        let suggestions = self.parse_suggestions_response(&response)?;

        Ok(suggestions)
    }

    /// Quick check using fast model
    pub async fn quick_check(
        &self,
        content: &str,
        check_type: QuickCheckType,
    ) -> Result<bool> {
        debug!("Performing quick check: {:?}", check_type);

        let prompt = match check_type {
            QuickCheckType::HasBugs => format!(
                "Does this code have any obvious bugs? Answer with just YES or NO.\n\n{}",
                content
            ),
            QuickCheckType::NeedsRefactor => format!(
                "Does this code need refactoring? Answer with just YES or NO.\n\n{}",
                content
            ),
            QuickCheckType::IsTested => format!(
                "Does this code appear to have tests? Answer with just YES or NO.\n\n{}",
                content
            ),
        };

        let response = self.call_ollama(&self.config.quick_check_model, &prompt).await?;

        Ok(response.to_lowercase().contains("yes"))
    }

    /// Call Ollama API
    async fn call_ollama(&self, model: &str, prompt: &str) -> Result<String> {
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: 0.3, // Lower temperature for more focused analysis
                top_p: 0.9,
                num_predict: 2048,
            },
        };

        let url = format!("{}/api/generate", self.config.ollama_base_url);

        debug!("Calling Ollama API: {}", url);

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to call Ollama API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error {}: {}", status, error_text);
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(ollama_response.response)
    }

    /// Create analysis prompt
    fn create_analysis_prompt(&self, file_path: &Path, content: &str, language: &str) -> String {
        format!(
            r#"You are a code review expert. Analyze the following {} code from file {:?}.

Identify:
1. Potential bugs and security issues
2. Code style violations
3. Performance problems
4. Maintainability concerns

Provide your analysis in this JSON format:
{{
  "issues": [
    {{
      "severity": "error|warning|info",
      "category": "bug|style|performance|security|complexity",
      "description": "Description of the issue",
      "line": line_number,
      "suggestion": "How to fix it"
    }}
  ],
  "overall_quality": "poor|fair|good|excellent",
  "key_concerns": ["concern1", "concern2"]
}}

Code:
```{}
{}
```

Analysis:"#,
            language,
            file_path,
            language,
            content
        )
    }

    /// Create improvement prompt
    fn create_improvement_prompt(&self, file_path: &Path, content: &str, issues: &[Issue]) -> String {
        let issues_summary = issues
            .iter()
            .map(|i| format!("- {:?}: {}", i.category, i.description))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"You are a code improvement expert. Given the code and identified issues, suggest specific improvements.

File: {:?}

Known Issues:
{}

Code:
```
{}
```

Provide improvement suggestions in this JSON format:
{{
  "suggestions": [
    {{
      "type": "refactor|add_test|add_comment|extract_function|remove_duplication|optimize",
      "description": "What to improve",
      "confidence": 0.0-1.0,
      "impact": "low|medium|high",
      "rationale": "Why this improvement helps"
    }}
  ]
}}

Suggestions:"#,
            file_path,
            issues_summary,
            content
        )
    }

    /// Parse analysis response
    fn parse_analysis_response(&self, response: &str) -> Result<AIAnalysisResult> {
        // Try to extract JSON from response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        match serde_json::from_str::<AIAnalysisResult>(json_str) {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("Failed to parse AI response as JSON: {}. Using fallback parsing.", e);
                // Fallback: basic text analysis
                Ok(AIAnalysisResult {
                    issues: vec![],
                    overall_quality: "unknown".to_string(),
                    key_concerns: vec![],
                })
            }
        }
    }

    /// Parse suggestions response
    fn parse_suggestions_response(&self, response: &str) -> Result<Vec<Suggestion>> {
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        #[derive(Deserialize)]
        struct SuggestionsResponse {
            suggestions: Vec<AISuggestion>,
        }

        match serde_json::from_str::<SuggestionsResponse>(json_str) {
            Ok(response) => {
                Ok(response.suggestions.iter().map(|s| s.to_suggestion()).collect())
            }
            Err(e) => {
                warn!("Failed to parse suggestions: {}", e);
                Ok(vec![])
            }
        }
    }
}

/// Quick check types
#[derive(Debug, Clone, Copy)]
pub enum QuickCheckType {
    HasBugs,
    NeedsRefactor,
    IsTested,
}

/// AI analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIAnalysisResult {
    pub issues: Vec<AIIssue>,
    pub overall_quality: String,
    pub key_concerns: Vec<String>,
}

/// AI-detected issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIIssue {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub line: Option<usize>,
    pub suggestion: Option<String>,
}

impl AIIssue {
    /// Convert to Issue
    pub fn to_issue(&self) -> Issue {
        let severity = match self.severity.to_lowercase().as_str() {
            "critical" => IssueSeverity::Critical,
            "error" => IssueSeverity::Error,
            "warning" => IssueSeverity::Warning,
            _ => IssueSeverity::Info,
        };

        let category = match self.category.to_lowercase().as_str() {
            "bug" => crate::analyzer::IssueCategory::Bug,
            "style" => crate::analyzer::IssueCategory::Style,
            "performance" => crate::analyzer::IssueCategory::Performance,
            "security" => crate::analyzer::IssueCategory::Security,
            "complexity" => crate::analyzer::IssueCategory::Complexity,
            "documentation" => crate::analyzer::IssueCategory::Documentation,
            "testing" => crate::analyzer::IssueCategory::Testing,
            _ => crate::analyzer::IssueCategory::Maintainability,
        };

        Issue {
            severity,
            category,
            description: self.description.clone(),
            line: self.line,
            column: None,
            snippet: None,
            suggested_fix: self.suggestion.clone(),
        }
    }
}

/// AI suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AISuggestion {
    #[serde(rename = "type")]
    suggestion_type: String,
    description: String,
    confidence: f32,
    impact: String,
    rationale: String,
}

impl AISuggestion {
    fn to_suggestion(&self) -> Suggestion {
        let suggestion_type = match self.suggestion_type.to_lowercase().as_str() {
            "refactor" => SuggestionType::Refactor,
            "add_test" => SuggestionType::AddTest,
            "add_comment" => SuggestionType::AddComment,
            "extract_function" => SuggestionType::ExtractFunction,
            "remove_duplication" => SuggestionType::RemoveDuplication,
            "optimize" => SuggestionType::OptimizeImports,
            _ => SuggestionType::SimplifyLogic,
        };

        let impact = match self.impact.to_lowercase().as_str() {
            "high" => Impact::High,
            "medium" => Impact::Medium,
            _ => Impact::Low,
        };

        Suggestion {
            suggestion_type,
            description: self.description.clone(),
            confidence: self.confidence,
            impact,
            code_changes: None,
            rationale: self.rationale.clone(),
        }
    }
}

/// Ollama request
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

/// Ollama options
#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    top_p: f32,
    num_predict: usize,
}

/// Ollama response
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_issue_conversion() {
        let ai_issue = AIIssue {
            severity: "error".to_string(),
            category: "bug".to_string(),
            description: "Null pointer dereference".to_string(),
            line: Some(42),
            suggestion: Some("Check for null".to_string()),
        };

        let issue = ai_issue.to_issue();
        assert_eq!(issue.severity, IssueSeverity::Error);
        assert_eq!(issue.line, Some(42));
    }

    #[test]
    fn test_create_analysis_prompt() {
        let config = LocalAIConfig::default();
        let client = AIClient::new(config).unwrap();

        let prompt = client.create_analysis_prompt(
            Path::new("test.rs"),
            "fn main() {}",
            "rust",
        );

        assert!(prompt.contains("code review expert"));
        assert!(prompt.contains("fn main() {}"));
    }
}

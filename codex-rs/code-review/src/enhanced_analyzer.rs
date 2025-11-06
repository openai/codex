//! Enhanced code analyzer integrating AI, linters, and AST analysis

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

use crate::ai_client::{AIClient, QuickCheckType};
use crate::analyzer::{AnalysisResult, CodeMetrics, Issue, Suggestion};
use crate::ast_analyzer::{AstAnalyzer, AstAnalysisResult};
use crate::config::{AnalysisConfig, LocalAIConfig};
use crate::linters::LinterRegistry;

/// Enhanced code analyzer with AI and linter integration
pub struct EnhancedAnalyzer {
    /// Basic analysis config
    config: AnalysisConfig,
    /// AI client (optional)
    ai_client: Option<Arc<AIClient>>,
    /// Linter registry
    linters: Arc<LinterRegistry>,
    /// AST analyzer for Rust
    ast_analyzer: Option<AstAnalyzer>,
    /// Enable AI analysis
    use_ai: bool,
    /// Enable linters
    use_linters: bool,
    /// Enable AST analysis
    use_ast: bool,
}

impl EnhancedAnalyzer {
    /// Create a new enhanced analyzer
    pub fn new(config: AnalysisConfig) -> Self {
        let max_complexity = config.max_complexity;
        Self {
            config,
            ai_client: None,
            linters: Arc::new(LinterRegistry::default()),
            ast_analyzer: Some(AstAnalyzer::new(
                max_complexity,
                100, // max function length
            )),
            use_ai: false,
            use_linters: true,
            use_ast: true,
        }
    }

    /// Enable AI analysis
    pub fn with_ai(mut self, ai_config: LocalAIConfig) -> Result<Self> {
        self.ai_client = Some(Arc::new(AIClient::new(ai_config)?));
        self.use_ai = true;
        Ok(self)
    }

    /// Configure linter usage
    pub fn with_linters(mut self, enabled: bool) -> Self {
        self.use_linters = enabled;
        self
    }

    /// Configure AST analysis
    pub fn with_ast(mut self, enabled: bool) -> Self {
        self.use_ast = enabled;
        self
    }

    /// Set custom linter registry
    pub fn with_linter_registry(mut self, registry: LinterRegistry) -> Self {
        self.linters = Arc::new(registry);
        self
    }

    /// Comprehensive file analysis
    pub async fn analyze_file(&self, file_path: &Path) -> Result<EnhancedAnalysisResult> {
        let start = std::time::Instant::now();
        info!("Enhanced analysis of {:?}", file_path);

        // Read file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context("Failed to read file")?;

        // Detect language
        let language = self.detect_language(file_path);

        // Collect all issues from different sources
        let mut all_issues = Vec::new();
        let mut all_suggestions = Vec::new();

        // 1. AST Analysis (for Rust)
        let ast_result = if self.use_ast && language == "rust" {
            match self.analyze_with_ast(&content).await {
                Ok(result) => {
                    all_issues.extend(result.issues.clone());
                    Some(result)
                }
                Err(e) => {
                    debug!("AST analysis failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // 2. Linter Analysis
        if self.use_linters {
            match self.linters.lint_file(file_path).await {
                Ok(issues) => {
                    info!("Linters found {} issues", issues.len());
                    all_issues.extend(issues);
                }
                Err(e) => {
                    debug!("Linter analysis failed: {}", e);
                }
            }
        }

        // 3. AI Analysis
        let ai_result = if self.use_ai && self.ai_client.is_some() {
            match self.analyze_with_ai(&content, file_path, &language).await {
                Ok(result) => {
                    all_issues.extend(result.issues.iter().map(|i| i.to_issue()));
                    Some(result)
                }
                Err(e) => {
                    debug!("AI analysis failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // 4. Generate suggestions (AI-powered if available)
        if let Some(ai_client) = &self.ai_client {
            if self.use_ai {
                match ai_client
                    .suggest_improvements(file_path, &content, &all_issues)
                    .await
                {
                    Ok(suggestions) => {
                        all_suggestions.extend(suggestions);
                    }
                    Err(e) => {
                        debug!("AI suggestion generation failed: {}", e);
                    }
                }
            }
        }

        // 5. Calculate metrics
        let metrics = if let Some(ast) = &ast_result {
            self.ast_metrics_to_code_metrics(&ast)
        } else {
            self.calculate_basic_metrics(&content)?
        };

        // 6. Deduplicate issues
        all_issues = self.deduplicate_issues(all_issues);

        let duration = start.elapsed();

        Ok(EnhancedAnalysisResult {
            basic: AnalysisResult {
                file_path: file_path.to_path_buf(),
                issues: all_issues,
                suggestions: all_suggestions,
                metrics,
                timestamp: chrono::Utc::now(),
                duration,
            },
            ast_result,
            ai_result,
            language: language.to_string(),
        })
    }

    /// Quick analysis (faster, less thorough)
    pub async fn quick_analyze(&self, content: &str, language: &str) -> Result<QuickAnalysisResult> {
        info!("Quick analysis for {} code", language);

        let mut has_bugs = false;
        let mut needs_refactor = false;

        if let Some(ai_client) = &self.ai_client {
            if self.use_ai {
                has_bugs = ai_client
                    .quick_check(content, QuickCheckType::HasBugs)
                    .await
                    .unwrap_or(false);

                needs_refactor = ai_client
                    .quick_check(content, QuickCheckType::NeedsRefactor)
                    .await
                    .unwrap_or(false);
            }
        }

        // Quick complexity check for Rust
        let complexity = if language == "rust" && self.use_ast {
            if let Some(ast) = &self.ast_analyzer {
                ast.quick_complexity_check(content).ok()
            } else {
                None
            }
        } else {
            None
        };

        Ok(QuickAnalysisResult {
            has_bugs,
            needs_refactor,
            complexity,
        })
    }

    /// Detect programming language from file extension
    fn detect_language(&self, file_path: &Path) -> &str {
        match file_path.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust",
            Some("py") => "python",
            Some("js") => "javascript",
            Some("ts") => "typescript",
            Some("go") => "go",
            Some("java") => "java",
            Some("cpp") | Some("cc") | Some("cxx") => "cpp",
            Some("c") | Some("h") => "c",
            _ => "unknown",
        }
    }

    /// Analyze using AST
    async fn analyze_with_ast(&self, content: &str) -> Result<AstAnalysisResult> {
        if let Some(ast_analyzer) = &self.ast_analyzer {
            ast_analyzer.analyze(content)
        } else {
            anyhow::bail!("AST analyzer not available");
        }
    }

    /// Analyze using AI
    async fn analyze_with_ai(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
    ) -> Result<crate::ai_client::AIAnalysisResult> {
        if let Some(ai_client) = &self.ai_client {
            ai_client.analyze_code(file_path, content, language).await
        } else {
            anyhow::bail!("AI client not available");
        }
    }

    /// Convert AST metrics to CodeMetrics
    fn ast_metrics_to_code_metrics(&self, ast_result: &AstAnalysisResult) -> CodeMetrics {
        let ast_metrics = &ast_result.metrics;

        CodeMetrics {
            loc: 0, // Would need to count from source
            complexity: ast_metrics.complexity,
            num_functions: ast_metrics.num_functions,
            num_comments: 0, // Would need to count from source
            doc_coverage: if ast_metrics.num_pub_functions > 0 {
                ast_metrics.documented_functions as f32 / ast_metrics.num_pub_functions as f32
            } else {
                1.0
            },
            test_coverage: None,
        }
    }

    /// Calculate basic metrics (fallback)
    fn calculate_basic_metrics(&self, content: &str) -> Result<CodeMetrics> {
        let lines: Vec<&str> = content.lines().collect();
        let loc = lines.len();

        let num_functions = content.matches("fn ").count()
            + content.matches("function ").count()
            + content.matches("def ").count();

        let num_comments = lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("//") || trimmed.starts_with("#") || trimmed.starts_with("/*")
            })
            .count();

        let doc_lines = lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("///") || trimmed.starts_with("\"\"\"")
            })
            .count();

        let doc_coverage = if loc > 0 {
            doc_lines as f32 / loc as f32
        } else {
            0.0
        };

        // Estimate complexity
        let mut complexity = 1;
        complexity += content.matches("if ").count() as u32;
        complexity += content.matches("for ").count() as u32;
        complexity += content.matches("while ").count() as u32;
        complexity += content.matches("match ").count() as u32;

        Ok(CodeMetrics {
            loc,
            complexity,
            num_functions,
            num_comments,
            doc_coverage,
            test_coverage: None,
        })
    }

    /// Deduplicate issues from different sources
    fn deduplicate_issues(&self, mut issues: Vec<Issue>) -> Vec<Issue> {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut deduped = Vec::new();

        for issue in issues.drain(..) {
            // Create a key from issue properties
            let key = format!(
                "{:?}|{}|{:?}",
                issue.category, issue.description, issue.line
            );

            if seen.insert(key) {
                deduped.push(issue);
            }
        }

        deduped
    }
}

/// Enhanced analysis result
#[derive(Debug, Clone)]
pub struct EnhancedAnalysisResult {
    /// Basic analysis result
    pub basic: AnalysisResult,
    /// AST analysis result (Rust only)
    pub ast_result: Option<AstAnalysisResult>,
    /// AI analysis result
    pub ai_result: Option<crate::ai_client::AIAnalysisResult>,
    /// Detected language
    pub language: String,
}

impl EnhancedAnalysisResult {
    /// Get all issues combined
    pub fn all_issues(&self) -> &[Issue] {
        &self.basic.issues
    }

    /// Get summary
    pub fn summary(&self) -> AnalysisSummary {
        AnalysisSummary {
            total_issues: self.basic.issues.len(),
            total_suggestions: self.basic.suggestions.len(),
            complexity: self.basic.metrics.complexity,
            used_ai: self.ai_result.is_some(),
            used_ast: self.ast_result.is_some(),
            language: self.language.clone(),
        }
    }
}

/// Quick analysis result
#[derive(Debug, Clone)]
pub struct QuickAnalysisResult {
    pub has_bugs: bool,
    pub needs_refactor: bool,
    pub complexity: Option<u32>,
}

/// Analysis summary
#[derive(Debug, Clone)]
pub struct AnalysisSummary {
    pub total_issues: usize,
    pub total_suggestions: usize,
    pub complexity: u32,
    pub used_ai: bool,
    pub used_ast: bool,
    pub language: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AnalysisConfig;

    #[test]
    fn test_detect_language() {
        let config = AnalysisConfig::default();
        let analyzer = EnhancedAnalyzer::new(config);

        assert_eq!(analyzer.detect_language(Path::new("test.rs")), "rust");
        assert_eq!(analyzer.detect_language(Path::new("test.py")), "python");
        assert_eq!(analyzer.detect_language(Path::new("test.js")), "javascript");
    }

    #[tokio::test]
    async fn test_basic_metrics() {
        let config = AnalysisConfig::default();
        let analyzer = EnhancedAnalyzer::new(config);

        let code = r#"
fn test() {
    if true {
        println!("test");
    }
}
"#;

        let metrics = analyzer.calculate_basic_metrics(code).unwrap();
        assert!(metrics.num_functions > 0);
        assert!(metrics.complexity > 1);
    }
}

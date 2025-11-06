//! AST-based code analysis for Rust using syn

use anyhow::{Context, Result};
use std::collections::HashMap;
use syn::{visit::Visit, File, Item, ItemFn, Expr, Block};
use tracing::{debug, info};

use crate::analyzer::{CodeMetrics, Issue, IssueCategory, IssueSeverity};

/// AST analyzer for Rust code
pub struct AstAnalyzer {
    /// Maximum allowed function complexity
    max_complexity: u32,
    /// Maximum allowed function length
    max_function_length: usize,
}

impl AstAnalyzer {
    /// Create a new AST analyzer
    pub fn new(max_complexity: u32, max_function_length: usize) -> Self {
        Self {
            max_complexity,
            max_function_length,
        }
    }

    /// Analyze Rust source code using AST
    pub fn analyze(&self, source: &str) -> Result<AstAnalysisResult> {
        info!("Performing AST analysis");

        // Parse source code into AST
        let ast = syn::parse_file(source)
            .context("Failed to parse Rust source code")?;

        // Collect metrics and issues
        let mut visitor = CodeVisitor::new(self.max_complexity, self.max_function_length);
        visitor.visit_file(&ast);

        Ok(AstAnalysisResult {
            metrics: visitor.metrics,
            issues: visitor.issues,
            function_info: visitor.functions,
        })
    }

    /// Quick complexity check without full analysis
    pub fn quick_complexity_check(&self, source: &str) -> Result<u32> {
        let ast = syn::parse_file(source)?;
        let mut visitor = CodeVisitor::new(self.max_complexity, self.max_function_length);
        visitor.visit_file(&ast);
        Ok(visitor.metrics.complexity)
    }
}

impl Default for AstAnalyzer {
    fn default() -> Self {
        Self::new(10, 100)
    }
}

/// AST analysis result
#[derive(Debug, Clone)]
pub struct AstAnalysisResult {
    /// Code metrics from AST
    pub metrics: AstMetrics,
    /// Issues found during AST analysis
    pub issues: Vec<Issue>,
    /// Function information
    pub function_info: Vec<FunctionInfo>,
}

/// AST-derived code metrics
#[derive(Debug, Clone, Default)]
pub struct AstMetrics {
    /// Total functions
    pub num_functions: usize,
    /// Public functions
    pub num_pub_functions: usize,
    /// Total complexity
    pub complexity: u32,
    /// Average function length
    pub avg_function_length: f32,
    /// Documented functions
    pub documented_functions: usize,
}

/// Function information from AST
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// Function name
    pub name: String,
    /// Is public
    pub is_pub: bool,
    /// Has documentation
    pub has_doc: bool,
    /// Complexity score
    pub complexity: u32,
    /// Number of lines
    pub lines: usize,
    /// Number of parameters
    pub num_params: usize,
    /// Line number
    pub line: usize,
}

/// Visitor for collecting code metrics
struct CodeVisitor {
    metrics: AstMetrics,
    issues: Vec<Issue>,
    functions: Vec<FunctionInfo>,
    max_complexity: u32,
    max_function_length: usize,
}

impl CodeVisitor {
    fn new(max_complexity: u32, max_function_length: usize) -> Self {
        Self {
            metrics: AstMetrics::default(),
            issues: Vec::new(),
            functions: Vec::new(),
            max_complexity,
            max_function_length,
        }
    }

    fn calculate_complexity(&self, block: &Block) -> u32 {
        let mut complexity = 1; // Base complexity

        for stmt in &block.stmts {
            // Count decision points in expressions
            match stmt {
                syn::Stmt::Expr(expr, _) => {
                    complexity += self.count_decision_points(expr);
                }
                syn::Stmt::Local(local) => {
                    // Count decision points in let bindings with init
                    if let Some(init) = &local.init {
                        complexity += self.count_decision_points(&init.expr);
                    }
                }
                _ => {}
            }
        }

        complexity
    }

    fn count_decision_points(&self, expr: &Expr) -> u32 {
        let mut count = match expr {
            Expr::If(if_expr) => {
                // Count the if itself and recursively count in branches
                let mut c = 1;
                // Recursively count in then branch
                for stmt in &if_expr.then_branch.stmts {
                    if let syn::Stmt::Expr(e, _) = stmt {
                        c += self.count_decision_points(e);
                    }
                }
                // Recursively count in else branch
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    c += self.count_decision_points(else_expr);
                }
                c
            }
            Expr::Match(m) => m.arms.len() as u32,
            Expr::While(_) | Expr::ForLoop(_) | Expr::Loop(_) => 1,
            Expr::Binary(b) if matches!(b.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) => 1,
            Expr::Block(block) => {
                // Recursively count in blocks
                let mut c = 0;
                for stmt in &block.block.stmts {
                    if let syn::Stmt::Expr(e, _) = stmt {
                        c += self.count_decision_points(e);
                    }
                }
                c
            }
            _ => 0,
        };
        count
    }

    fn estimate_lines(&self, block: &Block) -> usize {
        block.stmts.len()
    }
}

impl<'ast> Visit<'ast> for CodeVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.metrics.num_functions += 1;

        let is_pub = matches!(node.vis, syn::Visibility::Public(_));
        if is_pub {
            self.metrics.num_pub_functions += 1;
        }

        // Check for documentation
        let has_doc = node.attrs.iter().any(|attr| {
            attr.path().is_ident("doc")
        });

        if has_doc {
            self.metrics.documented_functions += 1;
        }

        // Calculate complexity
        let complexity = self.calculate_complexity(&node.block);
        self.metrics.complexity += complexity;

        // Estimate function length
        let lines = self.estimate_lines(&node.block);

        let func_name = node.sig.ident.to_string();
        let line = 0; // Would need span info for accurate line numbers

        // Create function info
        self.functions.push(FunctionInfo {
            name: func_name.clone(),
            is_pub,
            has_doc,
            complexity,
            lines,
            num_params: node.sig.inputs.len(),
            line,
        });

        // Check for issues
        if complexity > self.max_complexity {
            self.issues.push(Issue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Complexity,
                description: format!(
                    "Function '{}' has complexity {} which exceeds threshold {}",
                    func_name, complexity, self.max_complexity
                ),
                line: Some(line),
                column: None,
                snippet: None,
                suggested_fix: Some("Consider breaking this function into smaller functions".to_string()),
            });
        }

        if lines > self.max_function_length {
            self.issues.push(Issue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Maintainability,
                description: format!(
                    "Function '{}' has {} lines which exceeds recommended {} lines",
                    func_name, lines, self.max_function_length
                ),
                line: Some(line),
                column: None,
                snippet: None,
                suggested_fix: Some("Consider refactoring this function".to_string()),
            });
        }

        if is_pub && !has_doc {
            self.issues.push(Issue {
                severity: IssueSeverity::Info,
                category: IssueCategory::Documentation,
                description: format!("Public function '{}' is missing documentation", func_name),
                line: Some(line),
                column: None,
                snippet: None,
                suggested_fix: Some("Add documentation with ///".to_string()),
            });
        }

        // Continue visiting
        syn::visit::visit_item_fn(self, node);
    }
}

/// Extract function signatures for test generation
pub fn extract_function_signatures(source: &str) -> Result<Vec<FunctionSignature>> {
    let ast = syn::parse_file(source)?;
    let mut signatures = Vec::new();

    for item in ast.items {
        if let Item::Fn(func) = item {
            signatures.push(FunctionSignature {
                name: func.sig.ident.to_string(),
                inputs: func.sig.inputs.len(),
                output: if matches!(func.sig.output, syn::ReturnType::Default) {
                    "()".to_string()
                } else {
                    "T".to_string() // Simplified
                },
                is_async: func.sig.asyncness.is_some(),
            });
        }
    }

    Ok(signatures)
}

/// Function signature for test generation
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    pub inputs: usize,
    pub output: String,
    pub is_async: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function() {
        let source = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
        "#;

        let analyzer = AstAnalyzer::default();
        let result = analyzer.analyze(source).unwrap();

        assert_eq!(result.metrics.num_functions, 1);
        assert_eq!(result.metrics.complexity, 1);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_complex_function() {
        let source = r#"
            fn complex(x: i32) -> i32 {
                if x > 0 {
                    if x > 10 {
                        if x > 20 {
                            x * 2
                        } else {
                            x + 1
                        }
                    } else {
                        x - 1
                    }
                } else {
                    0
                }
            }
        "#;

        let analyzer = AstAnalyzer::new(2, 100);
        let result = analyzer.analyze(source).unwrap();

        assert_eq!(result.metrics.num_functions, 1);
        assert!(result.metrics.complexity > 2);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_undocumented_public_function() {
        let source = r#"
            pub fn public_func() {}
        "#;

        let analyzer = AstAnalyzer::default();
        let result = analyzer.analyze(source).unwrap();

        assert_eq!(result.metrics.num_pub_functions, 1);
        assert_eq!(result.metrics.documented_functions, 0);

        let doc_issues: Vec<_> = result.issues.iter()
            .filter(|i| i.category == IssueCategory::Documentation)
            .collect();
        assert!(!doc_issues.is_empty());
    }

    #[test]
    fn test_extract_signatures() {
        let source = r#"
            fn sync_func(x: i32) -> String { String::new() }
            async fn async_func() -> Result<()> { Ok(()) }
        "#;

        let sigs = extract_function_signatures(source).unwrap();
        assert_eq!(sigs.len(), 2);
        assert!(!sigs[0].is_async);
        assert!(sigs[1].is_async);
    }
}

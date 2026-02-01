//! Shell command parsing and security analysis.
//!
//! This crate provides comprehensive shell command parsing using tree-sitter
//! with a tokenizer fallback, plus security analysis to detect potentially
//! dangerous patterns.
//!
//! # Features
//!
//! - **Multi-layer parsing**: Tree-sitter AST parsing with tokenizer fallback
//! - **Safe command extraction**: Whitelist-based extraction of "word-only" commands
//! - **Pipe segment extraction**: Parse pipelines into individual segments
//! - **Redirection parsing**: Detect and classify shell redirections
//! - **Security analysis**: 14 risk types across 2 phases (Allow/Ask)
//!
//! # Quick Start
//!
//! ```
//! use cocode_shell_parser::{ShellParser, security};
//!
//! // Parse a shell command
//! let mut parser = ShellParser::new();
//! let cmd = parser.parse("cat file.txt | grep pattern > output.txt");
//!
//! // Extract commands (safe extraction)
//! if let Some(commands) = cmd.try_extract_safe_commands() {
//!     for args in commands {
//!         println!("Command: {:?}", args);
//!     }
//! }
//!
//! // Analyze for security risks
//! let analysis = security::analyze(&cmd);
//! if analysis.has_risks() {
//!     for risk in &analysis.risks {
//!         println!("Risk: {}", risk);
//!     }
//! }
//! ```
//!
//! # Parsing Shell Invocations
//!
//! ```
//! use cocode_shell_parser::ShellParser;
//!
//! let mut parser = ShellParser::new();
//!
//! // Parse shell invocation from argv
//! let argv = vec!["bash".into(), "-c".into(), "ls -la && pwd".into()];
//! if let Some(cmd) = parser.parse_shell_invocation(&argv) {
//!     let commands = cmd.extract_commands();
//!     // commands = [["ls", "-la"], ["pwd"]]
//! }
//! ```
//!
//! # Tokenization Only
//!
//! For lighter-weight parsing without tree-sitter:
//!
//! ```
//! use cocode_shell_parser::Tokenizer;
//!
//! let tokenizer = Tokenizer::new();
//! let tokens = tokenizer.tokenize("echo 'hello world' $HOME").unwrap();
//!
//! for token in &tokens {
//!     println!("{:?}: {}", token.kind, token.text);
//! }
//! ```

mod error;
mod parser;
mod redirects;
mod segments;
mod tokenizer;

pub mod security;

// Re-export main types
pub use error::ParseError;
pub use error::Result;
pub use parser::ParsedCommand;
pub use parser::ShellParser;
pub use parser::ShellType;
pub use parser::detect_shell_type;
pub use parser::extract_shell_script;
pub use redirects::Redirect;
pub use redirects::RedirectKind;
pub use redirects::extract_redirects_from_tokens;
pub use redirects::extract_redirects_from_tree;
pub use segments::PipeSegment;
pub use segments::extract_segments_from_tokens;
pub use segments::extract_segments_from_tree;
pub use tokenizer::Span;
pub use tokenizer::Token;
pub use tokenizer::TokenKind;
pub use tokenizer::Tokenizer;

/// Convenience function to parse and analyze a command in one step.
pub fn parse_and_analyze(source: &str) -> (ParsedCommand, security::SecurityAnalysis) {
    let mut parser = ShellParser::new();
    let cmd = parser.parse(source);
    let analysis = security::analyze(&cmd);
    (cmd, analysis)
}

/// Convenience function to check if a command string is safe.
///
/// A command is considered safe if:
/// - It can be parsed as a "word-only" command sequence
/// - It has no high or critical security risks
pub fn is_safe_command(source: &str) -> bool {
    let mut parser = ShellParser::new();
    let cmd = parser.parse(source);

    // Must be a word-only command sequence
    if cmd.try_extract_safe_commands().is_none() {
        return false;
    }

    // Check security analysis
    let analysis = security::analyze(&cmd);
    !analysis.requires_approval()
        && analysis
            .risks
            .iter()
            .all(|r| r.level < security::RiskLevel::High)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_analyze() {
        let (cmd, analysis) = parse_and_analyze("ls -la");
        assert!(cmd.has_tree());
        assert!(!analysis.requires_approval());
    }

    #[test]
    fn test_is_safe_command() {
        assert!(is_safe_command("ls -la"));
        assert!(is_safe_command("git status && pwd"));
        assert!(!is_safe_command("rm -rf /"));
        assert!(!is_safe_command("eval $cmd"));
    }

    #[test]
    fn test_full_workflow() {
        let mut parser = ShellParser::new();

        // Parse a pipeline
        let cmd = parser.parse("cat file | grep pattern | wc -l");

        // Extract pipe segments
        if let Some(tree) = cmd.tree() {
            let segments = extract_segments_from_tree(tree, cmd.source());
            assert_eq!(segments.len(), 3);
            assert!(segments[0].is_piped);
        }

        // Check for redirections (none in this case)
        if let Some(tree) = cmd.tree() {
            let redirects = extract_redirects_from_tree(tree, cmd.source());
            assert!(redirects.is_empty());
        }

        // Security analysis
        let analysis = security::analyze(&cmd);
        // Simple pipeline should be relatively safe
        assert!(!analysis.requires_approval());
    }

    #[test]
    fn test_shell_invocation_workflow() {
        let mut parser = ShellParser::new();

        let argv = vec![
            "bash".to_string(),
            "-c".to_string(),
            "echo hello && ls".to_string(),
        ];

        let cmd = parser.parse_shell_invocation(&argv).unwrap();
        let commands = cmd.try_extract_safe_commands().unwrap();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], vec!["echo", "hello"]);
        assert_eq!(commands[1], vec!["ls"]);
    }

    #[test]
    fn test_tokenizer_standalone() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("echo 'hello' \"world\" $HOME").unwrap();

        // Filter out whitespace for easier testing
        let non_ws: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .collect();

        assert_eq!(non_ws.len(), 4);
        assert_eq!(non_ws[0].kind, TokenKind::Word);
        assert_eq!(non_ws[1].kind, TokenKind::SingleQuoted);
        assert_eq!(non_ws[2].kind, TokenKind::DoubleQuoted);
        assert_eq!(non_ws[3].kind, TokenKind::VariableExpansion);
    }
}

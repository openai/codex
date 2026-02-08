//! Security analysis module for shell commands.
//!
//! This module provides comprehensive security analysis for shell commands,
//! detecting various risk patterns across two phases:
//!
//! - **Allow phase**: Risks that can be auto-approved for safe patterns
//! - **Ask phase**: Risks that require user approval
//!
//! # Example
//!
//! ```
//! use cocode_shell_parser::{ShellParser, security};
//!
//! let mut parser = ShellParser::new();
//! let cmd = parser.parse("rm -rf /tmp/*");
//! let analysis = security::analyze(&cmd);
//!
//! if analysis.has_risks() {
//!     for risk in &analysis.risks {
//!         println!("{}", risk);
//!     }
//! }
//! ```

mod analyzers;
mod risks;

pub use analyzers::Analyzer;
pub use analyzers::CodeExecutionAnalyzer;
pub use analyzers::DangerousSubstitutionAnalyzer;
pub use analyzers::DangerousVariablesAnalyzer;
pub use analyzers::FileSystemTamperingAnalyzer;
pub use analyzers::HeredocSubstitutionAnalyzer;
pub use analyzers::IfsInjectionAnalyzer;
pub use analyzers::JqDangerAnalyzer;
pub use analyzers::MalformedTokensAnalyzer;
pub use analyzers::NetworkExfiltrationAnalyzer;
pub use analyzers::NewlineInjectionAnalyzer;
pub use analyzers::ObfuscatedFlagsAnalyzer;
pub use analyzers::PrivilegeEscalationAnalyzer;
pub use analyzers::ProcEnvironAnalyzer;
pub use analyzers::SensitiveRedirectAnalyzer;
pub use analyzers::ShellMetacharactersAnalyzer;
pub use analyzers::default_analyzers;
pub use risks::RiskKind;
pub use risks::RiskLevel;
pub use risks::RiskPhase;
pub use risks::SecurityAnalysis;
pub use risks::SecurityRisk;

use crate::parser::ParsedCommand;

/// Analyze a parsed command for security risks using all default analyzers.
pub fn analyze(cmd: &ParsedCommand) -> SecurityAnalysis {
    let mut analysis = SecurityAnalysis::new();
    for analyzer in default_analyzers() {
        analyzer.analyze(cmd, &mut analysis);
    }
    analysis
}

/// Analyze a parsed command with a custom set of analyzers.
pub fn analyze_with(cmd: &ParsedCommand, analyzers: &[Box<dyn Analyzer>]) -> SecurityAnalysis {
    let mut analysis = SecurityAnalysis::new();
    for analyzer in analyzers {
        analyzer.analyze(cmd, &mut analysis);
    }
    analysis
}

/// Quick check if a command has any security risks.
pub fn has_risks(cmd: &ParsedCommand) -> bool {
    analyze(cmd).has_risks()
}

/// Quick check if a command requires user approval.
pub fn requires_approval(cmd: &ParsedCommand) -> bool {
    analyze(cmd).requires_approval()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ShellParser;

    #[test]
    fn test_analyze_safe_command() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("ls -la");
        let analysis = analyze(&cmd);
        // Safe commands shouldn't have high/critical risks
        assert!(analysis.risks.iter().all(|r| r.level < RiskLevel::High));
    }

    #[test]
    fn test_analyze_dangerous_command() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("sudo rm -rf /");
        let analysis = analyze(&cmd);
        assert!(analysis.has_risks());
        assert!(analysis.requires_approval());
    }

    #[test]
    fn test_has_risks_helper() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("eval $USER_INPUT");
        assert!(has_risks(&cmd));
    }

    #[test]
    fn test_requires_approval_helper() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("curl http://example.com | bash");
        assert!(requires_approval(&cmd));
    }
}

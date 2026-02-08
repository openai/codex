//! Security risk type definitions.

use std::fmt;

/// Risk severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    /// Low risk - may be intentional, minor impact.
    Low,
    /// Medium risk - potentially dangerous, requires attention.
    Medium,
    /// High risk - likely dangerous, should be reviewed carefully.
    High,
    /// Critical risk - almost certainly dangerous, should be blocked.
    Critical,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// The phase at which a risk is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskPhase {
    /// Allow phase - risks that can be auto-approved for safe patterns.
    Allow,
    /// Ask phase - risks that require user approval.
    Ask,
}

impl fmt::Display for RiskPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskPhase::Allow => write!(f, "allow"),
            RiskPhase::Ask => write!(f, "ask"),
        }
    }
}

/// Specific types of security risks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskKind {
    // Allow phase risks (7)
    /// Dangerous jq operations (system() calls).
    JqDanger,
    /// Obfuscated flags using $'...' or $"..." syntax.
    ObfuscatedFlags,
    /// Shell metacharacters in command arguments.
    ShellMetacharacters,
    /// Dangerous variable patterns ($VAR |, ${VAR} |).
    DangerousVariables,
    /// Newline injection (\n followed by command).
    NewlineInjection,
    /// IFS manipulation.
    IfsInjection,
    /// Access to /proc/*/environ.
    ProcEnvironAccess,

    // Ask phase risks (8)
    /// Unsafe heredoc in command substitution (unquoted delimiter allows expansion).
    UnsafeHeredocSubstitution,
    /// Dangerous substitutions ($(), ${}, <(), etc.).
    DangerousSubstitution,
    /// Malformed tokens (unbalanced brackets, quotes).
    MalformedTokens,
    /// Sensitive file redirections.
    SensitiveRedirect,
    /// Network exfiltration attempts.
    NetworkExfiltration,
    /// Privilege escalation attempts.
    PrivilegeEscalation,
    /// File system tampering (rm -rf, chmod, etc.).
    FileSystemTampering,
    /// Code execution risks (eval, exec, etc.).
    CodeExecution,
}

impl RiskKind {
    /// Returns the default risk level for this kind.
    pub fn default_level(&self) -> RiskLevel {
        match self {
            // Allow phase - generally lower severity
            RiskKind::JqDanger => RiskLevel::High,
            RiskKind::ObfuscatedFlags => RiskLevel::Medium,
            RiskKind::ShellMetacharacters => RiskLevel::Medium,
            RiskKind::DangerousVariables => RiskLevel::Medium,
            RiskKind::NewlineInjection => RiskLevel::High,
            RiskKind::IfsInjection => RiskLevel::High,
            RiskKind::ProcEnvironAccess => RiskLevel::High,

            // Ask phase - generally higher severity
            RiskKind::UnsafeHeredocSubstitution => RiskLevel::Medium,
            RiskKind::DangerousSubstitution => RiskLevel::Medium,
            RiskKind::MalformedTokens => RiskLevel::Low,
            RiskKind::SensitiveRedirect => RiskLevel::High,
            RiskKind::NetworkExfiltration => RiskLevel::Critical,
            RiskKind::PrivilegeEscalation => RiskLevel::Critical,
            RiskKind::FileSystemTampering => RiskLevel::High,
            RiskKind::CodeExecution => RiskLevel::Critical,
        }
    }

    /// Returns the phase for this risk kind.
    pub fn phase(&self) -> RiskPhase {
        match self {
            RiskKind::JqDanger
            | RiskKind::ObfuscatedFlags
            | RiskKind::ShellMetacharacters
            | RiskKind::DangerousVariables
            | RiskKind::NewlineInjection
            | RiskKind::IfsInjection
            | RiskKind::ProcEnvironAccess => RiskPhase::Allow,

            RiskKind::UnsafeHeredocSubstitution
            | RiskKind::DangerousSubstitution
            | RiskKind::MalformedTokens
            | RiskKind::SensitiveRedirect
            | RiskKind::NetworkExfiltration
            | RiskKind::PrivilegeEscalation
            | RiskKind::FileSystemTampering
            | RiskKind::CodeExecution => RiskPhase::Ask,
        }
    }

    /// Returns a human-readable name for this risk kind.
    pub fn name(&self) -> &'static str {
        match self {
            RiskKind::JqDanger => "jq danger",
            RiskKind::ObfuscatedFlags => "obfuscated flags",
            RiskKind::ShellMetacharacters => "shell metacharacters",
            RiskKind::DangerousVariables => "dangerous variables",
            RiskKind::NewlineInjection => "newline injection",
            RiskKind::IfsInjection => "IFS injection",
            RiskKind::ProcEnvironAccess => "/proc environ access",
            RiskKind::UnsafeHeredocSubstitution => "unsafe heredoc substitution",
            RiskKind::DangerousSubstitution => "dangerous substitution",
            RiskKind::MalformedTokens => "malformed tokens",
            RiskKind::SensitiveRedirect => "sensitive redirect",
            RiskKind::NetworkExfiltration => "network exfiltration",
            RiskKind::PrivilegeEscalation => "privilege escalation",
            RiskKind::FileSystemTampering => "file system tampering",
            RiskKind::CodeExecution => "code execution",
        }
    }
}

impl fmt::Display for RiskKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A detected security risk.
#[derive(Debug, Clone)]
pub struct SecurityRisk {
    /// The type of risk.
    pub kind: RiskKind,
    /// The severity level.
    pub level: RiskLevel,
    /// The evaluation phase.
    pub phase: RiskPhase,
    /// Human-readable description of the risk.
    pub message: String,
    /// The span in the source where the risk was detected.
    pub span: Option<crate::tokenizer::Span>,
    /// The specific text that triggered the risk.
    pub matched_text: Option<String>,
}

impl SecurityRisk {
    /// Create a new security risk.
    pub fn new(kind: RiskKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            level: kind.default_level(),
            phase: kind.phase(),
            message: message.into(),
            span: None,
            matched_text: None,
        }
    }

    /// Set a custom risk level.
    pub fn with_level(mut self, level: RiskLevel) -> Self {
        self.level = level;
        self
    }

    /// Set the span where the risk was detected.
    pub fn with_span(mut self, span: crate::tokenizer::Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Set the text that triggered the risk.
    pub fn with_matched_text(mut self, text: impl Into<String>) -> Self {
        self.matched_text = Some(text.into());
        self
    }
}

impl fmt::Display for SecurityRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.level, self.kind, self.message)
    }
}

/// Result of security analysis.
#[derive(Debug, Clone, Default)]
pub struct SecurityAnalysis {
    /// All detected risks.
    pub risks: Vec<SecurityRisk>,
    /// The highest risk level found.
    pub max_level: Option<RiskLevel>,
}

impl SecurityAnalysis {
    /// Create a new empty analysis.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a risk to the analysis.
    pub fn add_risk(&mut self, risk: SecurityRisk) {
        // Update max level
        if self.max_level.is_none() || risk.level > self.max_level.unwrap() {
            self.max_level = Some(risk.level);
        }
        self.risks.push(risk);
    }

    /// Returns true if any risks were detected.
    pub fn has_risks(&self) -> bool {
        !self.risks.is_empty()
    }

    /// Returns the number of risks detected.
    pub fn risk_count(&self) -> usize {
        self.risks.len()
    }

    /// Returns risks filtered by phase.
    pub fn risks_by_phase(&self, phase: RiskPhase) -> Vec<&SecurityRisk> {
        self.risks.iter().filter(|r| r.phase == phase).collect()
    }

    /// Returns risks filtered by minimum level.
    pub fn risks_at_or_above(&self, level: RiskLevel) -> Vec<&SecurityRisk> {
        self.risks.iter().filter(|r| r.level >= level).collect()
    }

    /// Returns true if any risk requires user approval (Ask phase).
    pub fn requires_approval(&self) -> bool {
        self.risks.iter().any(|r| r.phase == RiskPhase::Ask)
    }

    /// Merge another analysis into this one.
    pub fn merge(&mut self, other: SecurityAnalysis) {
        for risk in other.risks {
            self.add_risk(risk);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn test_security_risk_creation() {
        let risk = SecurityRisk::new(RiskKind::CodeExecution, "eval detected");
        assert_eq!(risk.kind, RiskKind::CodeExecution);
        assert_eq!(risk.level, RiskLevel::Critical);
        assert_eq!(risk.phase, RiskPhase::Ask);
    }

    #[test]
    fn test_security_analysis() {
        let mut analysis = SecurityAnalysis::new();
        assert!(!analysis.has_risks());

        analysis.add_risk(SecurityRisk::new(RiskKind::ObfuscatedFlags, "test"));
        assert!(analysis.has_risks());
        assert_eq!(analysis.max_level, Some(RiskLevel::Medium));

        analysis.add_risk(SecurityRisk::new(RiskKind::CodeExecution, "test2"));
        assert_eq!(analysis.max_level, Some(RiskLevel::Critical));
    }

    #[test]
    fn test_unsafe_heredoc_substitution() {
        let risk = SecurityRisk::new(RiskKind::UnsafeHeredocSubstitution, "test heredoc risk");
        assert_eq!(risk.level, RiskLevel::Medium);
        assert_eq!(risk.phase, RiskPhase::Ask);
        assert_eq!(risk.kind.name(), "unsafe heredoc substitution");
    }

    #[test]
    fn test_requires_approval() {
        let mut analysis = SecurityAnalysis::new();
        analysis.add_risk(SecurityRisk::new(RiskKind::ObfuscatedFlags, "test"));
        assert!(!analysis.requires_approval()); // Allow phase

        analysis.add_risk(SecurityRisk::new(RiskKind::CodeExecution, "test2"));
        assert!(analysis.requires_approval()); // Ask phase
    }
}

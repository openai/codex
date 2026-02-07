//! Permission types for tool execution control.
//!
//! These types control how the agent requests and receives permissions
//! for potentially dangerous operations.

use serde::Deserialize;
use serde::Serialize;

/// Permission mode that controls how the agent handles tool execution permissions.
///
/// Determines the overall permission strategy for a session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    /// Default mode - ask for permission on sensitive operations.
    #[default]
    Default,
    /// Plan mode - read-only, no execution without approval.
    Plan,
    /// Accept edits automatically, but ask for other operations.
    AcceptEdits,
    /// Bypass all permission checks (dangerous).
    Bypass,
    /// Never ask for permission, deny if not pre-approved.
    DontAsk,
}

impl PermissionMode {
    /// Check if this mode requires explicit approval for writes.
    pub fn requires_write_approval(&self) -> bool {
        matches!(self, PermissionMode::Default | PermissionMode::Plan)
    }

    /// Check if this mode allows automatic edit acceptance.
    pub fn auto_accept_edits(&self) -> bool {
        matches!(self, PermissionMode::AcceptEdits | PermissionMode::Bypass)
    }

    /// Check if this mode bypasses all permission checks.
    pub fn is_bypass(&self) -> bool {
        matches!(self, PermissionMode::Bypass)
    }

    /// Get the mode as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::Plan => "plan",
            PermissionMode::AcceptEdits => "accept-edits",
            PermissionMode::Bypass => "bypass",
            PermissionMode::DontAsk => "dont-ask",
        }
    }
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Behavior for a specific permission check.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionBehavior {
    /// Allow the operation without asking.
    Allow,
    /// Ask the user for permission.
    #[default]
    Ask,
    /// Deny the operation without asking.
    Deny,
}

impl PermissionBehavior {
    /// Check if this behavior allows the operation.
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionBehavior::Allow)
    }

    /// Check if this behavior requires asking the user.
    pub fn requires_approval(&self) -> bool {
        matches!(self, PermissionBehavior::Ask)
    }

    /// Check if this behavior denies the operation.
    pub fn is_denied(&self) -> bool {
        matches!(self, PermissionBehavior::Deny)
    }
}

/// Result of a permission check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum PermissionResult {
    /// Operation is allowed to proceed.
    Allowed,
    /// Operation is denied.
    Denied {
        /// Reason for denial.
        reason: String,
    },
    /// Operation needs user approval before proceeding.
    NeedsApproval {
        /// The approval request to present to the user.
        request: ApprovalRequest,
    },
    /// No rule matched — fall through to defaults.
    ///
    /// Tools return this from `check_permission()` when they have no
    /// opinion, letting the pipeline apply default behavior
    /// (reads → Allow, writes → NeedsApproval).
    Passthrough,
}

impl PermissionResult {
    /// Check if the operation is allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionResult::Allowed)
    }

    /// Check if the operation is denied.
    pub fn is_denied(&self) -> bool {
        matches!(self, PermissionResult::Denied { .. })
    }

    /// Check if the operation needs approval.
    pub fn needs_approval(&self) -> bool {
        matches!(self, PermissionResult::NeedsApproval { .. })
    }

    /// Check if no rule matched (passthrough to defaults).
    pub fn is_passthrough(&self) -> bool {
        matches!(self, PermissionResult::Passthrough)
    }
}

/// A permission decision with additional context about why the decision was made.
///
/// This wraps `PermissionResult` with metadata about which rule matched
/// and from which source, enabling debugging and audit logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    /// The permission result.
    pub result: PermissionResult,
    /// Human-readable reason for the decision.
    pub reason: String,
    /// The source of the rule that matched (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<RuleSource>,
    /// The pattern that matched (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_pattern: Option<String>,
}

impl PermissionDecision {
    /// Create an allowed decision with a reason.
    pub fn allowed(reason: impl Into<String>) -> Self {
        Self {
            result: PermissionResult::Allowed,
            reason: reason.into(),
            source: None,
            matched_pattern: None,
        }
    }

    /// Create a denied decision with a reason.
    pub fn denied(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            result: PermissionResult::Denied {
                reason: reason.clone(),
            },
            reason,
            source: None,
            matched_pattern: None,
        }
    }

    /// Set the rule source.
    pub fn with_source(mut self, source: RuleSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the matched pattern.
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.matched_pattern = Some(pattern.into());
        self
    }

    /// Check if the operation is allowed.
    pub fn is_allowed(&self) -> bool {
        self.result.is_allowed()
    }
}

/// Source of a permission rule, ordered by priority (highest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleSource {
    /// Policy-level rules (highest priority).
    Policy,
    /// Project-level settings (.claude/settings.json in project).
    Project,
    /// Local settings (.claude/settings.local.json).
    Local,
    /// User-level settings (~/.claude/settings.json).
    User,
    /// CLI flag overrides.
    Flag,
    /// CLI argument overrides.
    Cli,
    /// Per-command overrides.
    Command,
    /// Session-level approvals (lowest priority).
    Session,
}

impl RuleSource {
    /// Get the source as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleSource::Policy => "policy",
            RuleSource::Project => "project",
            RuleSource::Local => "local",
            RuleSource::User => "user",
            RuleSource::Flag => "flag",
            RuleSource::Cli => "cli",
            RuleSource::Command => "command",
            RuleSource::Session => "session",
        }
    }
}

impl std::fmt::Display for RuleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Request for user approval of an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique identifier for this request.
    pub request_id: String,
    /// The tool requesting approval.
    pub tool_name: String,
    /// Human-readable description of what will happen.
    pub description: String,
    /// Security risks associated with this operation.
    #[serde(default)]
    pub risks: Vec<SecurityRisk>,
    /// Whether this can be auto-approved for similar future operations.
    #[serde(default)]
    pub allow_remember: bool,
}

/// Result of a permission check with additional context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionCheckResult {
    /// The behavior to apply.
    pub behavior: PermissionBehavior,
    /// Optional message explaining the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Security risks identified during the check.
    #[serde(default)]
    pub risks: Vec<SecurityRisk>,
}

/// A security risk associated with an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityRisk {
    /// Type of risk.
    pub risk_type: RiskType,
    /// Severity of the risk.
    pub severity: RiskSeverity,
    /// Human-readable description of the risk.
    pub message: String,
}

/// Type of security risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskType {
    /// Operation could destroy or modify data.
    Destructive,
    /// Operation involves network access.
    Network,
    /// Operation modifies system configuration.
    SystemConfig,
    /// Operation accesses sensitive files.
    SensitiveFile,
    /// Operation requires elevated privileges.
    Elevated,
    /// Unknown or unclassified risk.
    Unknown,
}

impl RiskType {
    /// Get the risk type as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskType::Destructive => "destructive",
            RiskType::Network => "network",
            RiskType::SystemConfig => "system-config",
            RiskType::SensitiveFile => "sensitive-file",
            RiskType::Elevated => "elevated",
            RiskType::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for RiskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Severity level of a security risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskSeverity {
    /// Low severity - minor impact.
    Low,
    /// Medium severity - moderate impact.
    Medium,
    /// High severity - significant impact.
    High,
    /// Critical severity - severe impact.
    Critical,
}

impl RiskSeverity {
    /// Get the severity as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskSeverity::Low => "low",
            RiskSeverity::Medium => "medium",
            RiskSeverity::High => "high",
            RiskSeverity::Critical => "critical",
        }
    }

    /// Check if this severity is at least the given level.
    pub fn at_least(&self, other: RiskSeverity) -> bool {
        *self >= other
    }
}

impl std::fmt::Display for RiskSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mode_default() {
        assert_eq!(PermissionMode::default(), PermissionMode::Default);
    }

    #[test]
    fn test_permission_mode_methods() {
        assert!(PermissionMode::Default.requires_write_approval());
        assert!(PermissionMode::Plan.requires_write_approval());
        assert!(!PermissionMode::AcceptEdits.requires_write_approval());
        assert!(!PermissionMode::Bypass.requires_write_approval());

        assert!(!PermissionMode::Default.auto_accept_edits());
        assert!(PermissionMode::AcceptEdits.auto_accept_edits());
        assert!(PermissionMode::Bypass.auto_accept_edits());

        assert!(!PermissionMode::Default.is_bypass());
        assert!(PermissionMode::Bypass.is_bypass());
    }

    #[test]
    fn test_permission_behavior_default() {
        assert_eq!(PermissionBehavior::default(), PermissionBehavior::Ask);
    }

    #[test]
    fn test_permission_behavior_methods() {
        assert!(PermissionBehavior::Allow.is_allowed());
        assert!(!PermissionBehavior::Ask.is_allowed());
        assert!(!PermissionBehavior::Deny.is_allowed());

        assert!(!PermissionBehavior::Allow.requires_approval());
        assert!(PermissionBehavior::Ask.requires_approval());
        assert!(!PermissionBehavior::Deny.requires_approval());

        assert!(!PermissionBehavior::Allow.is_denied());
        assert!(!PermissionBehavior::Ask.is_denied());
        assert!(PermissionBehavior::Deny.is_denied());
    }

    #[test]
    fn test_permission_result_methods() {
        assert!(PermissionResult::Allowed.is_allowed());
        assert!(!PermissionResult::Allowed.is_denied());
        assert!(!PermissionResult::Allowed.needs_approval());
        assert!(!PermissionResult::Allowed.is_passthrough());

        let denied = PermissionResult::Denied {
            reason: "test".to_string(),
        };
        assert!(!denied.is_allowed());
        assert!(denied.is_denied());
        assert!(!denied.needs_approval());

        let needs_approval = PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: "1".to_string(),
                tool_name: "test".to_string(),
                description: "test".to_string(),
                risks: vec![],
                allow_remember: false,
            },
        };
        assert!(!needs_approval.is_allowed());
        assert!(!needs_approval.is_denied());
        assert!(needs_approval.needs_approval());

        assert!(PermissionResult::Passthrough.is_passthrough());
        assert!(!PermissionResult::Passthrough.is_allowed());
    }

    #[test]
    fn test_risk_severity_ordering() {
        assert!(RiskSeverity::Low < RiskSeverity::Medium);
        assert!(RiskSeverity::Medium < RiskSeverity::High);
        assert!(RiskSeverity::High < RiskSeverity::Critical);

        assert!(RiskSeverity::Critical.at_least(RiskSeverity::Low));
        assert!(RiskSeverity::Medium.at_least(RiskSeverity::Medium));
        assert!(!RiskSeverity::Low.at_least(RiskSeverity::High));
    }

    #[test]
    fn test_permission_decision_constructors() {
        let allowed = PermissionDecision::allowed("bypass mode");
        assert!(allowed.is_allowed());
        assert_eq!(allowed.reason, "bypass mode");

        let denied = PermissionDecision::denied("read-only command");
        assert!(!denied.is_allowed());
    }

    #[test]
    fn test_permission_decision_with_source() {
        let decision = PermissionDecision::allowed("matched rule")
            .with_source(RuleSource::Project)
            .with_pattern("Edit:src/**/*.rs");
        assert_eq!(decision.source, Some(RuleSource::Project));
        assert_eq!(
            decision.matched_pattern.as_deref(),
            Some("Edit:src/**/*.rs")
        );
    }

    #[test]
    fn test_rule_source_ordering() {
        assert!(RuleSource::Policy < RuleSource::Project);
        assert!(RuleSource::Project < RuleSource::Local);
        assert!(RuleSource::Local < RuleSource::User);
        assert!(RuleSource::Command < RuleSource::Session);
    }

    #[test]
    fn test_permission_decision_serde() {
        let decision = PermissionDecision::allowed("test reason").with_source(RuleSource::Project);
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: PermissionDecision = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_allowed());
        assert_eq!(parsed.source, Some(RuleSource::Project));
    }

    #[test]
    fn test_serde_roundtrip() {
        let mode = PermissionMode::AcceptEdits;
        let json = serde_json::to_string(&mode).unwrap();
        let parsed: PermissionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, mode);

        let behavior = PermissionBehavior::Allow;
        let json = serde_json::to_string(&behavior).unwrap();
        let parsed: PermissionBehavior = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, behavior);

        let risk = SecurityRisk {
            risk_type: RiskType::Destructive,
            severity: RiskSeverity::High,
            message: "May delete files".to_string(),
        };
        let json = serde_json::to_string(&risk).unwrap();
        let parsed: SecurityRisk = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, risk);
    }
}

//! Plan policy enforcement
//!
//! Defines permission tiers and approval gates for Plan operations.
//! Ensures that privileged operations (network, install, destructive commands)
//! require explicit approval.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Permission tier for an operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionTier {
    /// Safe operations (read workspace, compute, lint/test dry-runs)
    Safe,

    /// Privileged operations requiring approval
    /// (network calls, package install, destructive git ops)
    Privileged,
}

/// Types of privileged operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegedOperation {
    /// Network call (research, webhooks)
    Network,

    /// Package installation
    Install,

    /// Destructive git operation (force push, hard reset)
    GitDestructive,

    /// File write outside workspace
    FileWriteExternal,

    /// Execute arbitrary shell command
    ShellExec,
}

/// Policy configuration for Plan operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPolicy {
    /// Whether to require approval for network operations
    pub network_requires_approval: bool,

    /// Whether to require approval for package installation
    pub install_requires_approval: bool,

    /// Whether to require approval for destructive git ops
    pub git_destructive_requires_approval: bool,

    /// Whether research requires approval
    pub research_requires_approval: bool,

    /// Whether webhooks require approval
    pub webhook_requires_approval: bool,

    /// Allowed domains for network operations (empty = all allowed if approved)
    pub allowed_domains: Vec<String>,

    /// Role required for approvals
    pub approval_role: ApprovalRole,
}

impl Default for PlanPolicy {
    fn default() -> Self {
        Self {
            network_requires_approval: true,
            install_requires_approval: true,
            git_destructive_requires_approval: true,
            research_requires_approval: true,
            webhook_requires_approval: true,
            allowed_domains: Vec::new(),
            approval_role: ApprovalRole::Maintainer,
        }
    }
}

/// Role required for approving operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRole {
    /// Any user can approve
    User,

    /// Reviewer role required
    Reviewer,

    /// Maintainer role required
    Maintainer,

    /// Admin role required
    Admin,
}

/// Errors related to policy enforcement
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Operation {operation:?} requires approval")]
    ApprovalRequired { operation: PrivilegedOperation },

    #[error("Insufficient permissions: {role:?} required, but user has {user_role:?}")]
    InsufficientRole {
        role: ApprovalRole,
        user_role: ApprovalRole,
    },

    #[error("Domain {domain} not in allowed list")]
    DomainNotAllowed { domain: String },

    #[error("Operation {operation:?} not allowed in current mode")]
    OperationNotAllowed { operation: PrivilegedOperation },
}

/// Policy enforcer for Plan operations
pub struct PolicyEnforcer {
    policy: PlanPolicy,
}

impl PolicyEnforcer {
    /// Create a new policy enforcer
    pub fn new(policy: PlanPolicy) -> Self {
        Self { policy }
    }

    /// Check if an operation requires approval
    pub fn requires_approval(&self, operation: PrivilegedOperation) -> bool {
        match operation {
            PrivilegedOperation::Network => self.policy.network_requires_approval,
            PrivilegedOperation::Install => self.policy.install_requires_approval,
            PrivilegedOperation::GitDestructive => self.policy.git_destructive_requires_approval,
            PrivilegedOperation::FileWriteExternal => true, // Always require approval
            PrivilegedOperation::ShellExec => true,         // Always require approval
        }
    }

    /// Check if a user has sufficient role for approval
    pub fn can_approve(&self, user_role: ApprovalRole) -> bool {
        let required = self.policy.approval_role;

        // Check role hierarchy: User < Reviewer < Maintainer < Admin
        let user_level = match user_role {
            ApprovalRole::User => 0,
            ApprovalRole::Reviewer => 1,
            ApprovalRole::Maintainer => 2,
            ApprovalRole::Admin => 3,
        };

        let required_level = match required {
            ApprovalRole::User => 0,
            ApprovalRole::Reviewer => 1,
            ApprovalRole::Maintainer => 2,
            ApprovalRole::Admin => 3,
        };

        user_level >= required_level
    }

    /// Check if a domain is allowed for network operations
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        // If allowed_domains is empty, all domains are allowed (after approval)
        if self.policy.allowed_domains.is_empty() {
            return true;
        }

        self.policy
            .allowed_domains
            .iter()
            .any(|allowed| domain == allowed || domain.ends_with(&format!(".{}", allowed)))
    }

    /// Enforce policy for an operation
    pub fn enforce(
        &self,
        operation: PrivilegedOperation,
        user_role: Option<ApprovalRole>,
        domain: Option<&str>,
    ) -> Result<(), PolicyError> {
        // Check if operation requires approval
        if self.requires_approval(operation) {
            // Check if user has sufficient role
            if let Some(role) = user_role {
                if !self.can_approve(role) {
                    return Err(PolicyError::InsufficientRole {
                        role: self.policy.approval_role,
                        user_role: role,
                    });
                }
            } else {
                return Err(PolicyError::ApprovalRequired { operation });
            }
        }

        // Check domain if network operation
        if operation == PrivilegedOperation::Network {
            if let Some(d) = domain {
                if !self.is_domain_allowed(d) {
                    return Err(PolicyError::DomainNotAllowed {
                        domain: d.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl Default for PolicyEnforcer {
    fn default() -> Self {
        Self::new(PlanPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = PlanPolicy::default();
        assert!(policy.network_requires_approval);
        assert!(policy.install_requires_approval);
        assert!(policy.research_requires_approval);
    }

    #[test]
    fn test_role_hierarchy() {
        let enforcer = PolicyEnforcer::default();

        assert!(enforcer.can_approve(ApprovalRole::Admin));
        assert!(enforcer.can_approve(ApprovalRole::Maintainer));
        assert!(!enforcer.can_approve(ApprovalRole::Reviewer));
        assert!(!enforcer.can_approve(ApprovalRole::User));
    }

    #[test]
    fn test_domain_allowlist() {
        let mut policy = PlanPolicy::default();
        policy.allowed_domains = vec!["example.com".to_string()];

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.is_domain_allowed("example.com"));
        assert!(enforcer.is_domain_allowed("api.example.com"));
        assert!(!enforcer.is_domain_allowed("evil.com"));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let policy = PlanPolicy::default(); // Empty allowed_domains
        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.is_domain_allowed("any-domain.com"));
    }

    #[test]
    fn test_enforce_requires_approval() {
        let enforcer = PolicyEnforcer::default();

        // Network operation without approval should fail
        let result = enforcer.enforce(PrivilegedOperation::Network, None, Some("example.com"));
        assert!(result.is_err());

        // Network operation with sufficient role should succeed
        let result = enforcer.enforce(
            PrivilegedOperation::Network,
            Some(ApprovalRole::Maintainer),
            Some("example.com"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_insufficient_role() {
        let enforcer = PolicyEnforcer::default();

        // User role insufficient for network operation
        let result = enforcer.enforce(
            PrivilegedOperation::Network,
            Some(ApprovalRole::User),
            Some("example.com"),
        );
        assert!(matches!(result, Err(PolicyError::InsufficientRole { .. })));
    }
}

#![allow(dead_code)]

// Command approval logic for shell commands and patches.

pub mod ast;
pub mod ast_matcher;
pub mod git_model;
pub mod git_parser;
mod rules;
pub(crate) use rules::command_rules;
pub use rules::git_rules;

mod classifier;
mod parser;
mod policy;
mod rules_index;
mod shell_parser;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use codex_apply_patch::ApplyPatchAction;

use crate::exec::SandboxType;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;

#[derive(Debug, PartialEq)]
pub enum SafetyCheck {
    AutoApprove {
        sandbox_type: SandboxType,
        user_explicitly_approved: bool,
    },
    AskUser,
    Reject {
        reason: String,
    },
}

// Provide both `SafetyCheck` and `CommandDecision` names for the same decision type.
pub type CommandDecision = SafetyCheck;

impl SafetyCheck {
    /// Approve execution within the supplied sandbox scope, optionally noting that
    /// the user explicitly approved the command.
    #[doc(alias = "Permit")]
    #[inline]
    pub fn permit(scope: SandboxType, user_override: bool) -> Self {
        SafetyCheck::AutoApprove {
            sandbox_type: scope,
            user_explicitly_approved: user_override,
        }
    }

    /// Require an interactive approval step before running the command.
    #[doc(alias = "RequireApproval")]
    #[inline]
    pub fn require_approval() -> Self {
        SafetyCheck::AskUser
    }

    /// Reject the command outright with a diagnostic reason.
    #[doc(alias = "Deny")]
    #[inline]
    pub fn deny(reason: impl Into<String>) -> Self {
        SafetyCheck::Reject {
            reason: reason.into(),
        }
    }
}

/// Convenience accessors and predicates for working with approval decisions.
pub trait DecisionApi {
    /// Returns the sandbox scope if the decision permits execution.
    fn execution_scope(&self) -> Option<&SandboxType>;
    /// Indicates whether the user explicitly approved the command.
    fn user_override(&self) -> bool;

    /// Returns true when the decision permits execution.
    fn is_permit(&self) -> bool;
    /// Returns true when interactive approval is required.
    fn is_require_approval(&self) -> bool;
    /// Returns true when the command is rejected.
    fn is_deny(&self) -> bool;

    /// Provides the rejection reason when present.
    fn deny_reason(&self) -> Option<&str>;
}

impl DecisionApi for SafetyCheck {
    #[inline]
    fn execution_scope(&self) -> Option<&SandboxType> {
        match self {
            SafetyCheck::AutoApprove { sandbox_type, .. } => Some(sandbox_type),
            _ => None,
        }
    }

    #[inline]
    fn user_override(&self) -> bool {
        matches!(
            self,
            SafetyCheck::AutoApprove {
                user_explicitly_approved: true,
                ..
            }
        )
    }

    #[inline]
    fn is_permit(&self) -> bool {
        matches!(self, SafetyCheck::AutoApprove { .. })
    }
    #[inline]
    fn is_require_approval(&self) -> bool {
        matches!(self, SafetyCheck::AskUser)
    }
    #[inline]
    fn is_deny(&self) -> bool {
        matches!(self, SafetyCheck::Reject { .. })
    }

    #[inline]
    fn deny_reason(&self) -> Option<&str> {
        match self {
            SafetyCheck::Reject { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

// Ergonomic conversions for callers.
impl From<(SandboxType, bool)> for SafetyCheck {
    #[inline]
    fn from(v: (SandboxType, bool)) -> Self {
        SafetyCheck::permit(v.0, v.1)
    }
}
impl From<String> for SafetyCheck {
    #[inline]
    fn from(reason: String) -> Self {
        SafetyCheck::deny(reason)
    }
}
impl<'a> From<&'a str> for SafetyCheck {
    #[inline]
    fn from(reason: &'a str) -> Self {
        SafetyCheck::deny(reason)
    }
}

pub fn assess_patch_safety(
    action: &ApplyPatchAction,
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> CommandDecision {
    // TODO: Implement rules-based patch safety assessment
    let _ = (action, policy, sandbox_policy, cwd);
    CommandDecision::require_approval()
}

pub fn assess_command_safety(
    command: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    approved: &HashSet<Vec<String>>,
    with_escalated_permissions: bool,
) -> CommandDecision {
    policy::assess_command(
        command,
        approval_policy,
        sandbox_policy,
        approved,
        with_escalated_permissions,
    )
}

pub fn get_platform_sandbox() -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else {
        None
    }
}

static PLATFORM_SANDBOX: LazyLock<Option<SandboxType>> = LazyLock::new(get_platform_sandbox);

pub(crate) fn platform_sandbox() -> Option<SandboxType> {
    *PLATFORM_SANDBOX
}

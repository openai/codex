//! Shared approvals and sandboxing traits used by tool runtimes.
//!
//! Consolidates the approval flow primitives (`ApprovalDecision`, `ApprovalStore`,
//! `ApprovalCtx`, `Approvable`) together with the sandbox orchestration traits
//! and helpers (`Sandboxable`, `ToolRuntime`, `SandboxAttempt`, etc.).

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxTransformError;
use crate::state::SessionServices;
use codex_network_proxy::NetworkProxy;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;

use futures::Future;
use futures::future::BoxFuture;
use serde::Serialize;

#[derive(Clone, Default, Debug)]
pub(crate) struct ApprovalStore {
    // Store serialized keys for generic caching across requests.
    map: HashMap<String, ReviewDecision>,
}

impl ApprovalStore {
    pub fn get<K>(&self, key: &K) -> Option<ReviewDecision>
    where
        K: Serialize,
    {
        let s = serde_json::to_string(key).ok()?;
        self.map.get(&s).cloned()
    }

    pub fn put<K>(&mut self, key: K, value: ReviewDecision)
    where
        K: Serialize,
    {
        if let Ok(s) = serde_json::to_string(&key) {
            self.map.insert(s, value);
        }
    }
}

/// Takes a vector of approval keys and returns a ReviewDecision.
/// There will be one key in most cases, but apply_patch can modify multiple files at once.
///
/// - If all keys are already approved for session, we skip prompting.
/// - If the user approves for session, we store the decision for each key individually
///   so future requests touching any subset can also skip prompting.
pub(crate) async fn with_cached_approval<K, F, Fut>(
    services: &SessionServices,
    // Name of the tool, used for metrics collection.
    tool_name: &str,
    keys: Vec<K>,
    fetch: F,
) -> ReviewDecision
where
    K: Serialize,
    F: FnOnce() -> Fut,
    Fut: Future<Output = ReviewDecision>,
{
    // To be defensive here, don't bother with checking the cache if keys are empty.
    if keys.is_empty() {
        return fetch().await;
    }

    let already_approved = {
        let store = services.tool_approvals.lock().await;
        keys.iter()
            .all(|key| matches!(store.get(key), Some(ReviewDecision::ApprovedForSession)))
    };

    if already_approved {
        return ReviewDecision::ApprovedForSession;
    }

    let decision = fetch().await;

    services.otel_manager.counter(
        "codex.approval.requested",
        1,
        &[
            ("tool", tool_name),
            ("approved", decision.to_opaque_string()),
        ],
    );

    if matches!(decision, ReviewDecision::ApprovedForSession) {
        let mut store = services.tool_approvals.lock().await;
        for key in keys {
            store.put(key, ReviewDecision::ApprovedForSession);
        }
    }

    decision
}

#[derive(Clone)]
pub(crate) struct ApprovalCtx<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub call_id: &'a str,
    pub retry_reason: Option<String>,
}

// Specifies what tool orchestrator should do with a given tool call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExecApprovalRequirement {
    /// No approval required for this tool call.
    Skip {
        /// The first attempt should skip sandboxing (e.g., when explicitly
        /// greenlit by policy).
        bypass_sandbox: bool,
        /// Proposed execpolicy amendment to skip future approvals for similar commands
        /// Only applies if the command fails to run in sandbox and codex prompts the user to run outside the sandbox.
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    /// Approval required for this tool call.
    NeedsApproval {
        reason: Option<String>,
        /// Proposed execpolicy amendment to skip future approvals for similar commands
        /// See core/src/exec_policy.rs for more details on how proposed_execpolicy_amendment is determined.
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    /// Execution forbidden for this tool call.
    Forbidden { reason: String },
}

impl ExecApprovalRequirement {
    pub fn proposed_execpolicy_amendment(&self) -> Option<&ExecPolicyAmendment> {
        match self {
            Self::NeedsApproval {
                proposed_execpolicy_amendment: Some(prefix),
                ..
            } => Some(prefix),
            Self::Skip {
                proposed_execpolicy_amendment: Some(prefix),
                ..
            } => Some(prefix),
            _ => None,
        }
    }
}

/// - Never, OnFailure: do not ask
/// - OnRequest: ask unless sandbox policy is DangerFullAccess
/// - UnlessTrusted: always ask
pub(crate) fn default_exec_approval_requirement(
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
) -> ExecApprovalRequirement {
    let needs_approval = match policy {
        AskForApproval::Never | AskForApproval::OnFailure => false,
        AskForApproval::OnRequest => !matches!(
            sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        ),
        AskForApproval::UnlessTrusted => true,
    };

    if needs_approval {
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    } else {
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SandboxOverride {
    NoOverride,
    BypassSandboxFirstAttempt,
}

pub(crate) trait Approvable<Req> {
    type ApprovalKey: Hash + Eq + Clone + Debug + Serialize;

    // In most cases (shell, unified_exec), a request will have a single approval key.
    //
    // However, apply_patch needs session "approve once, don't ask again" semantics that
    // apply to multiple atomic targets (e.g., apply_patch approves per file path). Returning
    // a list of keys lets the runtime treat the request as approved-for-session only if
    // *all* keys are already approved, while still caching approvals per-key so future
    // requests touching a subset can be auto-approved.
    fn approval_keys(&self, req: &Req) -> Vec<Self::ApprovalKey>;

    /// Some tools may request to skip the sandbox on the first attempt
    /// (e.g., when the request explicitly asks for escalated permissions).
    /// Defaults to `NoOverride`.
    fn sandbox_mode_for_first_attempt(&self, _req: &Req) -> SandboxOverride {
        SandboxOverride::NoOverride
    }

    fn should_bypass_approval(&self, policy: AskForApproval, already_approved: bool) -> bool {
        if already_approved {
            // We do not ask one more time
            return true;
        }
        matches!(policy, AskForApproval::Never)
    }

    /// Return `Some(_)` to specify a custom exec approval requirement, or `None`
    /// to fall back to policy-based default.
    fn exec_approval_requirement(&self, _req: &Req) -> Option<ExecApprovalRequirement> {
        None
    }

    /// Decide we can request an approval for no-sandbox execution.
    fn wants_no_sandbox_approval(&self, policy: AskForApproval) -> bool {
        !matches!(policy, AskForApproval::Never | AskForApproval::OnRequest)
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a Req,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SandboxablePreference {
    Auto,
    #[allow(dead_code)] // Will be used by later tools.
    Require,
    #[allow(dead_code)] // Will be used by later tools.
    Forbid,
}

pub(crate) trait Sandboxable {
    fn sandbox_preference(&self) -> SandboxablePreference;
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

pub(crate) struct ToolCtx<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub call_id: String,
    pub tool_name: String,
}

#[derive(Debug)]
pub(crate) enum ToolError {
    Rejected(String),
    Codex(CodexErr),
}

pub(crate) trait ToolRuntime<Req, Out>: Approvable<Req> + Sandboxable {
    async fn run(
        &mut self,
        req: &Req,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<Out, ToolError>;
}

pub(crate) struct SandboxAttempt<'a> {
    pub sandbox: crate::exec::SandboxType,
    pub policy: &'a crate::protocol::SandboxPolicy,
    pub enforce_managed_network: bool,
    pub(crate) manager: &'a SandboxManager,
    pub(crate) sandbox_cwd: &'a Path,
    pub codex_linux_sandbox_exe: Option<&'a std::path::PathBuf>,
    pub use_linux_sandbox_bwrap: bool,
    pub windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
}

impl<'a> SandboxAttempt<'a> {
    pub fn env_for(
        &self,
        spec: CommandSpec,
        network: Option<&NetworkProxy>,
    ) -> Result<crate::sandboxing::ExecRequest, SandboxTransformError> {
        self.manager
            .transform(crate::sandboxing::SandboxTransformRequest {
                spec,
                policy: self.policy,
                sandbox: self.sandbox,
                enforce_managed_network: self.enforce_managed_network,
                network,
                sandbox_policy_cwd: self.sandbox_cwd,
                codex_linux_sandbox_exe: self.codex_linux_sandbox_exe,
                use_linux_sandbox_bwrap: self.use_linux_sandbox_bwrap,
                windows_sandbox_level: self.windows_sandbox_level,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::NetworkAccess;
    use pretty_assertions::assert_eq;

    #[test]
    fn external_sandbox_skips_exec_approval_on_request() {
        assert_eq!(
            default_exec_approval_requirement(
                AskForApproval::OnRequest,
                &SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                },
            ),
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            }
        );
    }

    #[test]
    fn restricted_sandbox_requires_exec_approval_on_request() {
        assert_eq!(
            default_exec_approval_requirement(
                AskForApproval::OnRequest,
                &SandboxPolicy::new_read_only_policy()
            ),
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: None,
            }
        );
    }
}

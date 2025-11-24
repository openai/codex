//! Shared approvals and sandboxing traits used by tool runtimes.
//!
//! Consolidates the approval flow primitives (`ApprovalDecision`, `ApprovalStore`,
//! `ApprovalCtx`, `Approvable`) together with the sandbox orchestration traits
//! and helpers (`Sandboxable`, `ToolRuntime`, `SandboxAttempt`, etc.).

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::protocol::SandboxCommandAssessment;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxTransformError;
use crate::state::SessionServices;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;

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

pub(crate) async fn with_cached_approval<K, F, Fut>(
    services: &SessionServices,
    key: K,
    fetch: F,
) -> ReviewDecision
where
    K: Serialize + Clone,
    F: FnOnce() -> Fut,
    Fut: Future<Output = ReviewDecision>,
{
    {
        let store = services.tool_approvals.lock().await;
        if let Some(decision) = store.get(&key) {
            return decision;
        }
    }

    let decision = fetch().await;

    if matches!(decision, ReviewDecision::ApprovedForSession) {
        let mut store = services.tool_approvals.lock().await;
        store.put(key, ReviewDecision::ApprovedForSession);
    }

    decision
}

#[derive(Clone)]
pub(crate) struct ApprovalCtx<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub call_id: &'a str,
    pub retry_reason: Option<String>,
    pub risk: Option<SandboxCommandAssessment>,
}

// Specifies what tool orchestrator should do with a given tool call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalRequirement {
    /// No approval required for this tool call.
    Skip {
        /// The first attempt should skip sandboxing (e.g., when explicitly
        /// greenlit by policy).
        bypass_sandbox: bool,
    },
    /// Approval required for this tool call
    NeedsApproval { reason: Option<String> },
    /// Execution forbidden for this tool call
    Forbidden { reason: String },
}

/// - Never, OnFailure: do not ask
/// - OnRequest: ask unless sandbox policy is DangerFullAccess
/// - UnlessTrusted: always ask
pub(crate) fn default_approval_requirement(
    policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
) -> ApprovalRequirement {
    let needs_approval = match policy {
        AskForApproval::Never | AskForApproval::OnFailure => false,
        AskForApproval::OnRequest => !matches!(sandbox_policy, SandboxPolicy::DangerFullAccess),
        AskForApproval::UnlessTrusted => true,
    };

    if needs_approval {
        ApprovalRequirement::NeedsApproval { reason: None }
    } else {
        ApprovalRequirement::Skip {
            bypass_sandbox: false,
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

    fn approval_key(&self, req: &Req) -> Self::ApprovalKey;

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

    /// Override the default approval requirement. Return `Some(_)` to specify
    /// a custom requirement, or `None` to fall back to
    /// policy-based default.
    fn approval_requirement(&self, _req: &Req) -> Option<ApprovalRequirement> {
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

/// Captures the command metadata needed to re-run a tool request without sandboxing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SandboxRetryData {
    pub command: Vec<String>,
    pub cwd: PathBuf,
}

pub(crate) trait ProvidesSandboxRetryData {
    fn sandbox_retry_data(&self) -> Option<SandboxRetryData>;
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
    pub(crate) manager: &'a SandboxManager,
    pub(crate) sandbox_cwd: &'a Path,
    pub codex_linux_sandbox_exe: Option<&'a std::path::PathBuf>,
}

impl<'a> SandboxAttempt<'a> {
    pub fn env_for(
        &self,
        spec: CommandSpec,
    ) -> Result<crate::sandboxing::ExecEnv, SandboxTransformError> {
        self.manager.transform(
            spec,
            self.policy,
            self.sandbox,
            self.sandbox_cwd,
            self.codex_linux_sandbox_exe,
        )
    }
}

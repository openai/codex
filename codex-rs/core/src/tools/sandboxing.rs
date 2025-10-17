//! Shared approvals and sandboxing traits used by tool runtimes.
//!
//! Consolidates the approval flow primitives (`ApprovalDecision`, `ApprovalStore`,
//! `ApprovalCtx`, `Approvable`) together with the sandbox orchestration traits
//! and helpers (`Sandboxable`, `ToolRuntime`, `SandboxAttempt`, etc.).

use crate::codex::Session;
use crate::error::CodexErr;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxTransformError;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    ApprovedForSession,
    Denied,
    Abort,
}

impl From<ReviewDecision> for ApprovalDecision {
    fn from(value: ReviewDecision) -> Self {
        match value {
            ReviewDecision::Approved => ApprovalDecision::Approved,
            ReviewDecision::ApprovedForSession => ApprovalDecision::ApprovedForSession,
            ReviewDecision::Denied => ApprovalDecision::Denied,
            ReviewDecision::Abort => ApprovalDecision::Abort,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct ApprovalStore {
    // Store serialized keys for generic caching across requests.
    map: HashMap<String, ApprovalDecision>,
}

impl ApprovalStore {
    pub fn get<K>(&self, key: &K) -> Option<ApprovalDecision>
    where
        K: Serialize,
    {
        let s = serde_json::to_string(key).ok()?;
        self.map.get(&s).cloned()
    }

    pub fn put<K>(&mut self, key: K, value: ApprovalDecision)
    where
        K: Serialize,
    {
        if let Ok(s) = serde_json::to_string(&key) {
            self.map.insert(s, value);
        }
    }
}

#[derive(Clone)]
pub(crate) struct ApprovalCtx<'a> {
    pub session: &'a Session,
    pub sub_id: &'a str,
    pub call_id: &'a str,
    pub retry_reason: Option<String>,
}

pub(crate) trait Approvable<Req> {
    type ApprovalKey: Hash + Eq + Clone + Debug + Serialize;

    fn approval_key(&self, req: &Req) -> Self::ApprovalKey;

    fn should_bypass_approval(&self, policy: AskForApproval) -> bool {
        matches!(policy, AskForApproval::Never)
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a Req,
        ctx: ApprovalCtx<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ApprovalDecision> + Send + 'a>>;
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
    pub sub_id: String,
    pub call_id: String,
}

#[derive(Debug)]
pub(crate) enum ToolError {
    Rejected(String),
    SandboxDenied(String),
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
        spec: &CommandSpec,
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

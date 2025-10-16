//! Approvals layer: request-aware approvals with session caching.
//!
//! Defines `Approvable<Req>`, a lightweight `ApprovalStore` used to cache
//! decisions across retries, and `ApprovalCtx` passed to runtimes when they
//! need to ask for approval.
/*
Module: approvals

Request-aware approvals with a small cache to avoid re-prompting on retries.
Defines `Approvable<Req>`, `ApprovalStore`, and `ApprovalCtx`.
*/
use crate::codex::Session;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

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

#[derive(Clone, Copy)]
pub(crate) struct ApprovalCtx<'a> {
    pub policy: AskForApproval,
    pub session: &'a Session,
    pub sub_id: &'a str,
    pub call_id: &'a str,
}

pub(crate) trait Approvable<Req> {
    type ApprovalKey: Hash + Eq + Clone + Debug + Serialize;

    fn approval_key(&self, req: &Req) -> Self::ApprovalKey;

    fn reset_cache(&mut self);

    fn approval_preview(&self, _req: &Req) -> Vec<String> {
        Vec::new()
    }

    fn should_bypass_approval(&self, policy: AskForApproval) -> bool {
        matches!(policy, AskForApproval::Never)
    }

    fn map_review_decision(decision: ReviewDecision) -> ApprovalDecision {
        ApprovalDecision::from(decision)
    }

    // Optional helpers are intentionally omitted to keep the trait minimal.

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a Req,
        ctx: ApprovalCtx<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ApprovalDecision> + Send + 'a>>;
}

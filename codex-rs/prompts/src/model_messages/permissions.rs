//! Resolves permission and approval text from sparse catalog messages and bundled defaults.
//! Preserves explicit empty strings and source identity; composition and rendering live in
//! `permissions_instructions` and consume caller-resolved runtime facts.

use super::ResolvedMessage;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::PermissionMessages;

const APPROVAL_POLICY_NEVER: &str =
    include_str!("../../templates/permissions/approval_policy/never.md");
const APPROVAL_POLICY_UNLESS_TRUSTED: &str =
    include_str!("../../templates/permissions/approval_policy/unless_trusted.md");
const APPROVAL_POLICY_ON_REQUEST: &str =
    include_str!("../../templates/permissions/approval_policy/on_request.md");

pub(crate) const DANGER_FULL_ACCESS_TEMPLATE: &str =
    include_str!("../../templates/permissions/sandbox_mode/danger_full_access.md");
pub(crate) const WORKSPACE_WRITE_TEMPLATE: &str =
    include_str!("../../templates/permissions/sandbox_mode/workspace_write.md");
pub(crate) const READ_ONLY_TEMPLATE: &str =
    include_str!("../../templates/permissions/sandbox_mode/read_only.md");

/// Resolved approval-policy text; policy and tool selection remain with the consumer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedApprovalMessages<'a> {
    pub(crate) never: ResolvedMessage<'a>,
    pub(crate) on_request: ResolvedMessage<'a>,
    pub(crate) on_request_auto_review: ResolvedMessage<'a>,
    pub(crate) unless_trusted: ResolvedMessage<'a>,
}

impl<'a> ResolvedApprovalMessages<'a> {
    pub(crate) fn new(messages: Option<&'a ApprovalMessages>) -> Self {
        Self {
            never: ResolvedMessage::new(
                messages.and_then(|m| m.never.as_deref()),
                APPROVAL_POLICY_NEVER,
            ),
            on_request: ResolvedMessage::new(
                messages.and_then(|m| m.on_request.as_deref()),
                APPROVAL_POLICY_ON_REQUEST,
            ),
            on_request_auto_review: ResolvedMessage::new(
                messages.and_then(|m| m.on_request_auto_review.as_deref()),
                APPROVAL_POLICY_ON_REQUEST,
            ),
            unless_trusted: ResolvedMessage::new(
                messages.and_then(|m| m.unless_trusted.as_deref()),
                APPROVAL_POLICY_UNLESS_TRUSTED,
            ),
        }
    }
}

/// Permission text retains its source so renderers can distinguish catalog text from templates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedPermissionMessages<'a> {
    pub(crate) danger_full_access: ResolvedMessage<'a>,
    pub(crate) workspace_write: ResolvedMessage<'a>,
    pub(crate) read_only: ResolvedMessage<'a>,
}

impl<'a> ResolvedPermissionMessages<'a> {
    pub(crate) fn new(messages: Option<&'a PermissionMessages>) -> Self {
        Self {
            danger_full_access: ResolvedMessage::new(
                messages.and_then(|m| m.danger_full_access.as_deref()),
                DANGER_FULL_ACCESS_TEMPLATE,
            ),
            workspace_write: ResolvedMessage::new(
                messages.and_then(|m| m.workspace_write.as_deref()),
                WORKSPACE_WRITE_TEMPLATE,
            ),
            read_only: ResolvedMessage::new(
                messages.and_then(|m| m.read_only.as_deref()),
                READ_ONLY_TEMPLATE,
            ),
        }
    }
}

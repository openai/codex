use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::protocol::SandboxPolicy;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ProvidesSandboxRetryData;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use serde::Serialize;
use std::path::PathBuf;

const ARG_PREVIEW_CHAR_LIMIT: usize = 120;

#[derive(Clone, Debug)]
pub struct McpToolCallRequest {
    pub server: String,
    pub tool: String,
    pub raw_arguments: String,
    pub cwd: PathBuf,
}

impl ProvidesSandboxRetryData for McpToolCallRequest {
    fn sandbox_retry_data(&self) -> Option<crate::tools::sandboxing::SandboxRetryData> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct McpApprovalKey {
    server: String,
    tool: String,
}

#[derive(Default)]
pub struct McpRuntime;

impl McpRuntime {
    pub fn new() -> Self {
        Self
    }

    fn synthetic_command(req: &McpToolCallRequest) -> Vec<String> {
        let mut command = vec![
            "mcp.call".to_string(),
            format!("server={}", req.server),
            format!("tool={}", req.tool),
        ];

        if let Some(preview) = argument_preview(&req.raw_arguments) {
            command.push(preview);
        }

        command
    }
}

impl Sandboxable for McpRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Forbid
    }

    fn escalate_on_failure(&self) -> bool {
        false
    }
}

impl Approvable<McpToolCallRequest> for McpRuntime {
    type ApprovalKey = McpApprovalKey;

    fn approval_key(&self, req: &McpToolCallRequest) -> Self::ApprovalKey {
        McpApprovalKey {
            server: req.server.clone(),
            tool: req.tool.clone(),
        }
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a McpToolCallRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let key = self.approval_key(req);
        let command = Self::synthetic_command(req);
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let cwd = req.cwd.clone();
        let reason = ctx.retry_reason.clone();
        let risk = ctx.risk.clone();

        Box::pin(async move {
            with_cached_approval(&session.services, key, move || {
                let command = command.clone();
                let call_id = call_id.clone();
                let reason = reason.clone();
                let risk = risk.clone();
                let cwd = cwd.clone();
                async move {
                    session
                        .request_command_approval(turn, call_id, command, cwd, reason, risk)
                        .await
                }
            })
            .await
        })
    }

    fn wants_initial_approval(
        &self,
        _req: &McpToolCallRequest,
        policy: AskForApproval,
        _sandbox_policy: &SandboxPolicy,
    ) -> bool {
        matches!(
            policy,
            AskForApproval::OnRequest | AskForApproval::UnlessTrusted
        )
    }
}

impl ToolRuntime<McpToolCallRequest, ResponseInputItem> for McpRuntime {
    async fn run(
        &mut self,
        req: &McpToolCallRequest,
        _attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ResponseInputItem, ToolError> {
        Ok(handle_mcp_tool_call(
            ctx.session,
            ctx.turn,
            ctx.call_id.clone(),
            req.server.clone(),
            req.tool.clone(),
            req.raw_arguments.clone(),
        )
        .await)
    }
}

fn argument_preview(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut chars = trimmed.chars();
    let mut preview = String::new();
    for _ in 0..ARG_PREVIEW_CHAR_LIMIT {
        match chars.next() {
            Some(ch) => preview.push(ch),
            None => break,
        }
    }

    if preview.is_empty() {
        return None;
    }

    if chars.next().is_some() {
        preview.push_str("...");
    }

    Some(format!("args={preview}"))
}

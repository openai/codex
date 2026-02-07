use async_trait::async_trait;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::SandboxPermissions;
use serde::Deserialize;

use crate::exec_env::create_env;
use crate::exec_policy::ExecApprovalRequest;
use crate::function_tool::FunctionCallError;
use crate::protocol::ExecCommandSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::parse_arguments;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::runtimes::shell::ShellRuntime;
use crate::tools::sandboxing::ToolCtx;

pub struct PythonHandler;

#[derive(Debug, Deserialize)]
struct PythonToolCallParams {
    code: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    workdir: Option<String>,
    #[serde(alias = "timeout", default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    python: Option<String>,
    #[serde(default)]
    sandbox_permissions: Option<SandboxPermissions>,
    #[serde(default)]
    prefix_rule: Option<Vec<String>>,
    #[serde(default)]
    justification: Option<String>,
}

#[async_trait]
impl ToolHandler for PythonHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for python handler: {tool_name}"
            )));
        };

        let params: PythonToolCallParams = parse_arguments(&arguments)?;
        let PythonToolCallParams {
            code,
            args,
            workdir,
            timeout_ms,
            python,
            sandbox_permissions,
            prefix_rule,
            justification,
        } = params;

        let mut command = vec![python.unwrap_or_else(|| "python3".to_string())];
        command.push("-c".to_string());
        command.push(code);
        command.extend(args);

        let sandbox_permissions = sandbox_permissions.unwrap_or_default();
        if sandbox_permissions.requires_escalated_permissions()
            && !matches!(
                turn.approval_policy,
                codex_protocol::protocol::AskForApproval::OnRequest
            )
        {
            let approval_policy = turn.approval_policy;
            return Err(FunctionCallError::RespondToModel(format!(
                "approval policy is {approval_policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {approval_policy:?}"
            )));
        }

        let features = session.features();
        let request_rule_enabled = features.enabled(crate::features::Feature::RequestRule);
        let prefix_rule = if request_rule_enabled {
            prefix_rule
        } else {
            None
        };

        let cwd = turn.resolve_path(workdir);
        let mut env = create_env(
            &turn.shell_environment_policy,
            Some(session.conversation_id),
        );
        let dependency_env = session.dependency_env().await;
        if !dependency_env.is_empty() {
            env.extend(dependency_env);
        }

        let emitter = ToolEmitter::shell(
            command.clone(),
            cwd.clone(),
            ExecCommandSource::Agent,
            false,
        );
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, None);
        emitter.begin(event_ctx).await;

        let exec_approval_requirement = session
            .services
            .exec_policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: turn.approval_policy,
                sandbox_policy: &turn.sandbox_policy,
                sandbox_permissions,
                prefix_rule,
            })
            .await;

        let req = ShellRequest {
            command,
            cwd,
            timeout_ms,
            env,
            sandbox_permissions,
            justification,
            exec_approval_requirement,
        };

        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ShellRuntime::new();
        let tool_ctx = ToolCtx {
            session: session.as_ref(),
            turn: turn.as_ref(),
            call_id: call_id.clone(),
            tool_name,
        };
        let out = orchestrator
            .run(&mut runtime, &req, &tool_ctx, &turn, turn.approval_policy)
            .await;

        let event_ctx = ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, None);
        let content = emitter.finish(event_ctx, out).await?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

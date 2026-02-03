use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecExpiration;
use crate::exec::process_exec_tool_call;
use crate::exec_env::create_env;
use crate::protocol::AskForApproval;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandSource;
use crate::protocol::HookInput;
use crate::protocol::HookKind;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::default_exec_approval_requirement;

const HOOKS_FILENAME: &str = "hook.toml";

#[derive(Debug, Clone, Deserialize)]
struct HookCommandToml {
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct HooksToml {
    #[serde(default)]
    turn_start: Option<HookCommandToml>,
    #[serde(default)]
    turn_end: Option<HookCommandToml>,
}

#[derive(Debug, Clone)]
pub(crate) struct HookCommand {
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HookConfig {
    pub(crate) turn_start: Option<HookCommand>,
    pub(crate) turn_end: Option<HookCommand>,
}

impl HookCommand {
    pub(crate) fn command_vec(&self) -> Vec<String> {
        let mut cmd = Vec::with_capacity(1 + self.args.len());
        cmd.push(self.command.clone());
        cmd.extend(self.args.clone());
        cmd
    }
}

impl HookConfig {
    pub(crate) fn command_for(&self, kind: HookKind) -> Option<&HookCommand> {
        match kind {
            HookKind::TurnStart => self.turn_start.as_ref(),
            HookKind::TurnEnd => self.turn_end.as_ref(),
        }
    }
}

pub(crate) async fn load_hook_config(cwd: &Path) -> Option<HookConfig> {
    let path = hook_config_path(cwd);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            warn!("failed to read hook config {}: {err}", path.display());
            return None;
        }
    };

    let parsed: HooksToml = match toml::from_str(&contents) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!("failed to parse hook config {}: {err}", path.display());
            return None;
        }
    };

    Some(HookConfig {
        turn_start: parsed.turn_start.map(|cmd| HookCommand {
            command: cmd.command,
            args: cmd.args,
        }),
        turn_end: parsed.turn_end.map(|cmd| HookCommand {
            command: cmd.command,
            args: cmd.args,
        }),
    })
}

pub(crate) async fn run_hook(
    session: &Session,
    turn_context: &TurnContext,
    kind: HookKind,
    cancellation_token: &CancellationToken,
) -> Option<HookInput> {
    let config = load_hook_config(&turn_context.cwd).await?;
    let hook_command = config.command_for(kind)?;
    let command = hook_command.command_vec();
    let call_id = Uuid::new_v4().to_string();
    let source = match kind {
        HookKind::TurnStart => ExecCommandSource::HookTurnStart,
        HookKind::TurnEnd => ExecCommandSource::HookTurnEnd,
    };

    let emitter = ToolEmitter::shell(command.clone(), turn_context.cwd.clone(), source, false);
    let event_ctx = ToolEventCtx::new(session, turn_context, &call_id, None);
    emitter.begin(event_ctx).await;

    let exec_approval_requirement = default_exec_approval_requirement(
        turn_context.approval_policy,
        &turn_context.sandbox_policy,
    );
    let req = ShellRequest {
        command: command.clone(),
        cwd: turn_context.cwd.clone(),
        timeout_ms: None,
        env: create_env(&turn_context.shell_environment_policy),
        sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
        justification: None,
        exec_approval_requirement,
    };

    if requires_approval(
        &req,
        turn_context.approval_policy,
        &turn_context.sandbox_policy,
    ) {
        let decision = session
            .request_command_approval(
                turn_context,
                call_id.clone(),
                req.command.clone(),
                req.cwd.clone(),
                None,
                req.exec_approval_requirement
                    .proposed_execpolicy_amendment()
                    .cloned(),
            )
            .await;
        if matches!(
            decision,
            codex_protocol::protocol::ReviewDecision::Denied
                | codex_protocol::protocol::ReviewDecision::Abort
        ) {
            let finish_ctx = ToolEventCtx::new(session, turn_context, &call_id, None);
            let _ = emitter
                .finish(
                    finish_ctx,
                    Err(ToolError::Rejected("rejected by user".to_string())),
                )
                .await;
            return None;
        }
    }

    let run_result = tokio::select! {
        out = run_hook_command(session, turn_context, &req, &call_id, cancellation_token) => out,
        _ = cancellation_token.cancelled() => {
            return None;
        }
    };

    let stderr = match &run_result {
        Ok(output) => output.stderr.text.clone(),
        Err(_) => String::new(),
    };
    let exit_code = match &run_result {
        Ok(output) => output.exit_code,
        Err(_) => -1,
    };

    let finish_ctx = ToolEventCtx::new(session, turn_context, &call_id, None);
    let _ = emitter
        .finish(finish_ctx, run_result.map_err(ToolError::Codex))
        .await;

    let stderr = stderr.trim().to_string();
    if stderr.is_empty() {
        return None;
    }

    let hook_input = HookInput {
        hook: kind,
        command,
        stderr,
        exit_code,
    };

    session
        .send_event(turn_context, EventMsg::HookInput(hook_input.clone()))
        .await;

    Some(hook_input)
}

fn hook_config_path(cwd: &Path) -> PathBuf {
    cwd.join(".codex").join(HOOKS_FILENAME)
}

fn requires_approval(
    req: &ShellRequest,
    policy: AskForApproval,
    sandbox_policy: &crate::protocol::SandboxPolicy,
) -> bool {
    matches!(
        default_exec_approval_requirement(policy, sandbox_policy),
        crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval { .. }
    ) && matches!(
        req.exec_approval_requirement,
        crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval { .. }
    )
}

async fn run_hook_command(
    session: &Session,
    turn_context: &TurnContext,
    req: &ShellRequest,
    call_id: &str,
    cancellation_token: &CancellationToken,
) -> Result<crate::exec::ExecToolCallOutput, crate::error::CodexErr> {
    let params = crate::exec::ExecParams {
        command: req.command.clone(),
        cwd: req.cwd.clone(),
        expiration: ExecExpiration::Cancellation(cancellation_token.clone()),
        env: req.env.clone(),
        sandbox_permissions: req.sandbox_permissions,
        windows_sandbox_level: turn_context.windows_sandbox_level,
        justification: req.justification.clone(),
        arg0: None,
    };

    process_exec_tool_call(
        params,
        &turn_context.sandbox_policy,
        &turn_context.cwd,
        &turn_context.codex_linux_sandbox_exe,
        Some(crate::exec::StdoutStream {
            sub_id: turn_context.sub_id.clone(),
            call_id: call_id.to_string(),
            tx_event: session.get_tx_event(),
        }),
    )
    .await
}

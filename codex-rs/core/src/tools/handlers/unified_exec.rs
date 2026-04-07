use crate::function_tool::FunctionCallError;
use crate::maybe_emit_implicit_skill_invocation;
use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::shell::get_shell_by_model_provided_path;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::apply_granted_turn_permissions;
use crate::tools::handlers::apply_patch::intercept_apply_patch;
use crate::tools::handlers::implicit_granted_permissions;
use crate::tools::handlers::normalize_and_validate_additional_permissions;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::handlers::resolve_workdir_base_path;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::unified_exec::ExecCommandRequest;
use crate::unified_exec::UnifiedExecContext;
use crate::unified_exec::UnifiedExecProcessManager;
use crate::unified_exec::WriteStdinRequest;
use codex_features::Feature;
use codex_otel::SessionTelemetry;
use codex_otel::TOOL_CALL_UNIFIED_EXEC_METRIC;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TerminalInteractionEvent;
use codex_shell_command::is_safe_command::is_known_safe_command;
use codex_tools::UnifiedExecShellMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

pub struct UnifiedExecHandler;

#[derive(Debug, Deserialize)]
pub(crate) struct ExecCommandArgs {
    cmd: String,
    #[serde(default)]
    pub(crate) workdir: Option<String>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    login: Option<bool>,
    #[serde(default = "default_tty")]
    tty: bool,
    #[serde(default = "default_exec_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_output_tokens: Option<usize>,
    #[serde(default)]
    sandbox_permissions: SandboxPermissions,
    #[serde(default)]
    additional_permissions: Option<PermissionProfile>,
    #[serde(default)]
    justification: Option<String>,
    #[serde(default)]
    prefix_rule: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct WriteStdinArgs {
    // The model is trained on `session_id`.
    session_id: i32,
    #[serde(default)]
    chars: String,
    #[serde(default = "default_write_stdin_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_output_tokens: Option<usize>,
}

fn default_exec_yield_time_ms() -> u64 {
    10_000
}

fn default_write_stdin_yield_time_ms() -> u64 {
    250
}

fn default_tty() -> bool {
    false
}

impl ToolHandler for UnifiedExecHandler {
    type Output = ExecCommandToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            tracing::error!(
                "This should never happen, invocation payload is wrong: {:?}",
                invocation.payload
            );
            return true;
        };

        let Ok(params) = parse_arguments::<ExecCommandArgs>(arguments) else {
            return true;
        };
        let command = match get_command(
            &params,
            invocation.session.user_shell(),
            &invocation.turn.tools_config.unified_exec_shell_mode,
            invocation.turn.tools_config.allow_login_shell,
        ) {
            Ok(command) => command,
            Err(_) => return true,
        };
        !is_known_safe_command(&command)
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        if invocation.tool_name != "exec_command" {
            return None;
        }

        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        parse_arguments::<ExecCommandArgs>(arguments)
            .ok()
            .map(|args| PreToolUsePayload { command: args.cmd })
    }

    fn post_tool_use_payload(
        &self,
        call_id: &str,
        payload: &ToolPayload,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Function { arguments } = payload else {
            return None;
        };

        let args = parse_arguments::<ExecCommandArgs>(arguments).ok()?;
        if args.tty {
            return None;
        }

        let tool_response = result.post_tool_use_response(call_id, payload)?;
        Some(PostToolUsePayload {
            command: args.cmd,
            tool_response,
        })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "unified_exec handler received unsupported payload".to_string(),
                ));
            }
        };

        let manager: &UnifiedExecProcessManager = &session.services.unified_exec_manager;
        let context = UnifiedExecContext::new(session.clone(), turn.clone(), call_id.clone());

        let response = match tool_name.as_str() {
            "exec_command" => {
                let cwd = resolve_workdir_base_path(&arguments, context.turn.cwd.as_path())?;
                let args: ExecCommandArgs =
                    parse_arguments_with_base_path(&arguments, cwd.as_path())?;
                let workdir = context.turn.resolve_path(args.workdir.clone());
                maybe_emit_implicit_skill_invocation(
                    session.as_ref(),
                    context.turn.as_ref(),
                    &args.cmd,
                    &workdir,
                )
                .await;
                let process_id = manager.allocate_process_id().await;
                let command = get_command(
                    &args,
                    session.user_shell(),
                    &turn.tools_config.unified_exec_shell_mode,
                    turn.tools_config.allow_login_shell,
                )
                .map_err(FunctionCallError::RespondToModel)?;
                let command_for_display = codex_shell_command::parse_command::shlex_join(&command);

                let ExecCommandArgs {
                    workdir,
                    tty,
                    yield_time_ms,
                    max_output_tokens,
                    sandbox_permissions,
                    additional_permissions,
                    justification,
                    prefix_rule,
                    ..
                } = args;

                let exec_permission_approvals_enabled =
                    session.features().enabled(Feature::ExecPermissionApprovals);
                let requested_additional_permissions = additional_permissions.clone();
                let effective_additional_permissions = apply_granted_turn_permissions(
                    context.session.as_ref(),
                    sandbox_permissions,
                    additional_permissions,
                )
                .await;
                let additional_permissions_allowed = exec_permission_approvals_enabled
                    || (session.features().enabled(Feature::RequestPermissionsTool)
                        && effective_additional_permissions.permissions_preapproved);

                // Sticky turn permissions have already been approved, so they should
                // continue through the normal exec approval flow for the command.
                if effective_additional_permissions
                    .sandbox_permissions
                    .requests_sandbox_override()
                    && !effective_additional_permissions.permissions_preapproved
                    && !matches!(
                        context.turn.approval_policy.value(),
                        codex_protocol::protocol::AskForApproval::OnRequest
                    )
                {
                    let approval_policy = context.turn.approval_policy.value();
                    manager.release_process_id(process_id).await;
                    return Err(FunctionCallError::RespondToModel(format!(
                        "approval policy is {approval_policy:?}; reject command — you cannot ask for escalated permissions if the approval policy is {approval_policy:?}"
                    )));
                }

                let workdir = workdir.filter(|value| !value.is_empty());

                let workdir = workdir.map(|dir| context.turn.resolve_path(Some(dir)));
                let cwd = workdir.clone().unwrap_or(cwd);
                let normalized_additional_permissions = match implicit_granted_permissions(
                    sandbox_permissions,
                    requested_additional_permissions.as_ref(),
                    &effective_additional_permissions,
                )
                .map_or_else(
                    || {
                        normalize_and_validate_additional_permissions(
                            additional_permissions_allowed,
                            context.turn.approval_policy.value(),
                            effective_additional_permissions.sandbox_permissions,
                            effective_additional_permissions.additional_permissions,
                            effective_additional_permissions.permissions_preapproved,
                            &cwd,
                        )
                    },
                    |permissions| Ok(Some(permissions)),
                ) {
                    Ok(normalized) => normalized,
                    Err(err) => {
                        manager.release_process_id(process_id).await;
                        return Err(FunctionCallError::RespondToModel(err));
                    }
                };

                let apply_patch_cwd = match AbsolutePathBuf::from_absolute_path(&cwd) {
                    Ok(cwd) => cwd,
                    Err(err) => {
                        manager.release_process_id(process_id).await;
                        return Err(FunctionCallError::RespondToModel(format!(
                            "apply_patch verification failed: failed to resolve cwd: {err}"
                        )));
                    }
                };
                if let Some(output) = intercept_apply_patch(
                    &command,
                    &apply_patch_cwd,
                    Some(yield_time_ms),
                    context.session.clone(),
                    context.turn.clone(),
                    Some(&tracker),
                    &context.call_id,
                    tool_name.as_str(),
                )
                .await?
                {
                    manager.release_process_id(process_id).await;
                    return Ok(ExecCommandToolOutput {
                        event_call_id: String::new(),
                        chunk_id: String::new(),
                        wall_time: std::time::Duration::ZERO,
                        raw_output: output.into_text().into_bytes(),
                        max_output_tokens: None,
                        process_id: None,
                        exit_code: None,
                        original_token_count: None,
                        session_command: None,
                    });
                }

                emit_unified_exec_tty_metric(&turn.session_telemetry, tty);
                manager
                    .exec_command(
                        ExecCommandRequest {
                            command,
                            process_id,
                            yield_time_ms,
                            max_output_tokens,
                            workdir,
                            network: context.turn.network.clone(),
                            tty,
                            sandbox_permissions: effective_additional_permissions
                                .sandbox_permissions,
                            additional_permissions: normalized_additional_permissions,
                            additional_permissions_preapproved: effective_additional_permissions
                                .permissions_preapproved,
                            justification,
                            prefix_rule,
                        },
                        &context,
                    )
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "exec_command failed for `{command_for_display}`: {err:?}"
                        ))
                    })?
            }
            "write_stdin" => {
                let args: WriteStdinArgs = parse_arguments(&arguments)?;
                let response = manager
                    .write_stdin(WriteStdinRequest {
                        process_id: args.session_id,
                        input: &args.chars,
                        yield_time_ms: args.yield_time_ms,
                        max_output_tokens: args.max_output_tokens,
                    })
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!("write_stdin failed: {err}"))
                    })?;

                let interaction = TerminalInteractionEvent {
                    call_id: response.event_call_id.clone(),
                    process_id: args.session_id.to_string(),
                    stdin: args.chars.clone(),
                };
                session
                    .send_event(turn.as_ref(), EventMsg::TerminalInteraction(interaction))
                    .await;

                response
            }
            other => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "unsupported unified exec function {other}"
                )));
            }
        };

        Ok(response)
    }
}

fn emit_unified_exec_tty_metric(session_telemetry: &SessionTelemetry, tty: bool) {
    session_telemetry.counter(
        TOOL_CALL_UNIFIED_EXEC_METRIC,
        /*inc*/ 1,
        &[("tty", if tty { "true" } else { "false" })],
    );
}

pub(crate) fn get_command(
    args: &ExecCommandArgs,
    session_shell: Arc<Shell>,
    shell_mode: &UnifiedExecShellMode,
    allow_login_shell: bool,
) -> Result<Vec<String>, String> {
    let use_login_shell = match args.login {
        Some(true) if !allow_login_shell => {
            return Err(
                "login shell is disabled by config; omit `login` or set it to false.".to_string(),
            );
        }
        Some(use_login_shell) => use_login_shell,
        None => allow_login_shell,
    };

    match shell_mode {
        UnifiedExecShellMode::Direct => {
            let model_shell = args.shell.as_ref().map(|shell_str| {
                let mut shell = get_shell_by_model_provided_path(&PathBuf::from(shell_str));
                shell.shell_snapshot = crate::shell::empty_shell_snapshot_receiver();
                shell
            });
            let shell = model_shell.as_ref().unwrap_or(session_shell.as_ref());
            Ok(shell.derive_exec_args(&args.cmd, use_login_shell))
        }
        UnifiedExecShellMode::ZshFork(zsh_fork_config) => Ok(vec![
            zsh_fork_config.shell_zsh_path.to_string_lossy().to_string(),
            if use_login_shell { "-lc" } else { "-c" }.to_string(),
            args.cmd.clone(),
        ]),
    }
}

#[cfg(test)]
#[path = "unified_exec_tests.rs"]
mod tests;

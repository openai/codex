//! Resolves exec arguments against the selected environment and effective turn permissions.
//! The live handler emits its skill event between parsing and preparation.

use super::ExecCommandArgs;
use super::ExecCommandWorkdirArgs;
use super::ResolvedCommand;
use super::get_command;
use crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
use crate::exec_policy::prompt_is_rejected_by_policy;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::handlers::EffectiveAdditionalPermissions;
use crate::tools::handlers::apply_granted_turn_permissions;
use crate::tools::handlers::file_system_sandbox_policy_context_for_cwd;
use crate::tools::handlers::implicit_granted_permissions;
use crate::tools::handlers::normalize_and_validate_additional_permissions;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::handlers::resolve_sandbox_permissions;
use codex_features::Feature;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_shell_command::shell_detect::detect_shell_type;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy)]
pub(crate) enum ExecCommandLifetime {
    Interactive,
    OneShot,
}

/// Command details available before the live handler reserves a process.
pub(crate) struct PreparedExecCommand {
    pub(crate) resolved: ResolvedCommand,
    pub(crate) hook_command: String,
    pub(crate) cwd: PathUri,
    pub(crate) permissions: EffectiveAdditionalPermissions,
    pub(crate) justification: Option<String>,
    pub(crate) prefix_rule: Option<Vec<String>>,
    pub(crate) tty: bool,
    pub(crate) yield_time_ms: u64,
    pub(crate) max_output_tokens: Option<usize>,
    pub(crate) completion_timeout: Option<Duration>,
}

pub(crate) struct ParsedExecCommand<'a> {
    session: &'a Session,
    step: &'a StepContext,
    environment: &'a TurnEnvironment,
    pub(super) args: ExecCommandArgs,
    pub(super) cwd: PathUri,
    pub(super) native_cwd: Option<AbsolutePathBuf>,
    sandbox_permissions: SandboxPermissions,
}

impl<'a> ParsedExecCommand<'a> {
    /// Resolve this call's workdir and arguments using the executor selected by the caller.
    pub(crate) fn parse(
        arguments: &str,
        session: &'a Session,
        step: &'a StepContext,
        turn_environment: &'a TurnEnvironment,
    ) -> Result<Self, FunctionCallError> {
        let workdir: ExecCommandWorkdirArgs = parse_arguments(arguments)?;
        let cwd = workdir.resolve_cwd(turn_environment)?;
        let environment = &turn_environment.environment;

        // Remote executors enforce URI-native sandbox policy themselves. Only a host-local
        // sandbox needs a native cwd for resolving paths nested in the permissions config.
        let requires_host_native_cwd = !environment.is_remote()
            && SandboxManager::new().select_initial(
                turn_environment.permission_profile(),
                SandboxablePreference::Auto,
                codex_protocol::sandbox::effective_windows_sandbox_type(
                    turn_environment.config().windows_sandbox_type,
                    turn_environment.config().windows_sandbox_level,
                ),
                step.turn.network.is_some(),
            ) != SandboxType::None;
        // `to_abs_path()` alone cannot identify foreign drive paths: `file:///C:/repo` is
        // representable as `/C:/repo` on POSIX. Require the inferred convention to match too.
        let cwd_uses_native_convention =
            cwd.infer_path_convention() == Some(PathConvention::native());
        let native_cwd = match cwd.to_abs_path() {
            Ok(cwd) if cwd_uses_native_convention => Some(cwd),
            _ if !requires_host_native_cwd => None,
            Err(err) => return Err(FunctionCallError::RespondToModel(err.to_string())),
            Ok(_) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "path URI `{cwd}` does not use the host's native {} path convention",
                    PathConvention::native()
                )));
            }
        };
        let args: ExecCommandArgs = match native_cwd.as_ref() {
            Some(native_cwd) => {
                // The base path only resolves paths nested in the permissions config types.
                parse_arguments_with_base_path(arguments, native_cwd)?
            }
            None => {
                // Foreign executor cwd values cannot seed this host's AbsolutePathBufGuard.
                // Sandbox intent and URI-native roots are still sent to the executor.
                parse_arguments(arguments)?
            }
        };
        if args.tty && !session.features().enabled(Feature::UnifiedExecTty) {
            return Err(FunctionCallError::RespondToModel(
                "TTY execution is disabled by config; omit `tty` or set it to false.".to_string(),
            ));
        }
        let sandbox_permissions =
            resolve_sandbox_permissions(args.sandbox_permissions, args.justification.as_deref())?;
        Ok(Self {
            session,
            step,
            environment: turn_environment,
            args,
            cwd,
            native_cwd,
            sandbox_permissions,
        })
    }

    pub(crate) fn prepare(
        self,
        lifetime: ExecCommandLifetime,
    ) -> Result<PreparedExecCommand, FunctionCallError> {
        let Self {
            session,
            step,
            environment: turn_environment,
            mut args,
            cwd,
            sandbox_permissions,
            ..
        } = self;
        // Remote environments may use a different OS and must build commands with their native
        // shell; fall back to the session shell when the environment did not report one.
        let shell = turn_environment
            .shell
            .clone()
            .map(Arc::new)
            .unwrap_or_else(|| session.user_shell());
        // TODO(anp): Resolve requested shells in remote environments instead of restricting
        // commands to the reported default shell.
        if turn_environment.environment.is_remote()
            && let Some(requested_shell) = args.shell.take()
        {
            let Some(remote_shell) = turn_environment.shell.as_ref() else {
                return Err(FunctionCallError::RespondToModel(format!(
                    "environment `{}` does not report a shell",
                    turn_environment.selection.environment_id
                )));
            };
            if detect_shell_type(Path::new(&requested_shell)) != Some(remote_shell.shell_type) {
                return Err(FunctionCallError::RespondToModel(format!(
                    "environment `{}` only supports `{}`",
                    turn_environment.selection.environment_id,
                    remote_shell.name()
                )));
            }
        }
        let resolved_command =
            get_command(&args, shell, turn_environment.config().allow_login_shell)
                .map_err(FunctionCallError::RespondToModel)?;
        let completion_timeout = match lifetime {
            ExecCommandLifetime::Interactive => None,
            ExecCommandLifetime::OneShot => {
                args.tty = false;
                Some(Duration::from_millis(
                    args.timeout_ms.unwrap_or(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
                ))
            }
        };

        let exec_permission_approvals_enabled =
            session.features().enabled(Feature::ExecPermissionApprovals);
        let requested_additional_permissions = args.additional_permissions.take();
        let sandbox_context =
            turn_environment.sandbox_context(/*additional_permissions*/ None);
        let permission_context = file_system_sandbox_policy_context_for_cwd(&sandbox_context, &cwd);
        let mut permissions = apply_granted_turn_permissions(
            step,
            turn_environment,
            &cwd,
            sandbox_permissions,
            requested_additional_permissions.clone(),
        );
        let additional_permissions_allowed = exec_permission_approvals_enabled
            || (session.features().enabled(Feature::RequestPermissionsTool)
                && permissions.permissions_preapproved);

        // Sticky turn permissions have already been approved, so they should
        // continue through the normal exec approval flow for the command.
        let approval_policy = step.settings.approval_policy();
        if permissions.sandbox_permissions.requests_sandbox_override()
            && !permissions.permissions_preapproved
            && prompt_is_rejected_by_policy(approval_policy, /*prompt_is_rule*/ false).is_some()
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "approval policy is {approval_policy:?}; reject command — you cannot ask for escalated permissions if the approval policy is {approval_policy:?}"
            )));
        }

        permissions.additional_permissions = implicit_granted_permissions(
            sandbox_permissions,
            requested_additional_permissions.as_ref(),
            &permissions,
        )
        .map_or_else(
            || {
                normalize_and_validate_additional_permissions(
                    additional_permissions_allowed,
                    approval_policy,
                    permissions.sandbox_permissions,
                    permissions.additional_permissions,
                    permissions.permissions_preapproved,
                    &permission_context,
                )
            },
            |permissions| Ok(Some(permissions)),
        )
        .map_err(FunctionCallError::RespondToModel)?;

        Ok(PreparedExecCommand {
            resolved: resolved_command,
            hook_command: args.cmd,
            yield_time_ms: args.yield_time_ms,
            max_output_tokens: args.max_output_tokens,
            completion_timeout,
            cwd,
            tty: args.tty,
            permissions,
            justification: args.justification,
            prefix_rule: args.prefix_rule,
        })
    }
}

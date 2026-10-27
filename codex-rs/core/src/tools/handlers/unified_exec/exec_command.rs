use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use crate::maybe_emit_implicit_skill_invocation;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::ToolEnvironmentArgs;
use crate::tools::handlers::apply_patch::intercept_apply_patch;
use crate::tools::handlers::function_tool_environment;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::rewrite_function_string_argument;
use crate::tools::handlers::selected_executor;
use crate::tools::handlers::updated_hook_command;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolTelemetryTags;
use crate::unified_exec::ExecCommandRequest;
use crate::unified_exec::UnifiedExecContext;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecProcessManager;
use crate::unified_exec::generate_chunk_id;
use crate::unified_exec::output::CapturedStream;
use codex_features::Feature;
use codex_otel::SessionTelemetry;
use codex_otel::TOOL_CALL_UNIFIED_EXEC_METRIC;
use codex_tools::JsonSchema;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_path_uri::PathUri;
use codex_utils_string::truncate_middle_chars;

use super::super::shell_spec::CommandToolOptions;
use super::super::shell_spec::create_exec_command_tool_with_environment_id;
use super::ExecCommandArgs;
use super::ExecCommandWorkdirArgs;
use super::post_unified_exec_tool_use_payload;
use super::preparation::ExecCommandLifetime;
use super::preparation::ParsedExecCommand;

// A byte limit is a conservative hard token bound even for byte-fallback tokenizers.
const EXEC_COMMAND_REJECTION_MAX_BYTES: usize = 900;

#[derive(Clone, Copy)]
pub(crate) struct ExecCommandHandlerOptions {
    pub(crate) allow_tty: bool,
    pub(crate) exec_permission_approvals_enabled: bool,
    pub(crate) include_environment_id: bool,
    pub(crate) include_windows_shell_guidance: bool,
}

pub struct ExecCommandHandler {
    options: ExecCommandHandlerOptions,
    lifetime: ExecCommandLifetime,
}

impl Default for ExecCommandHandler {
    fn default() -> Self {
        Self {
            lifetime: ExecCommandLifetime::Interactive,
            options: ExecCommandHandlerOptions {
                allow_tty: true,
                exec_permission_approvals_enabled: false,
                include_environment_id: false,
                include_windows_shell_guidance: cfg!(windows),
            },
        }
    }
}

impl ExecCommandHandler {
    pub(crate) fn new(options: ExecCommandHandlerOptions) -> Self {
        Self {
            options,
            lifetime: ExecCommandLifetime::Interactive,
        }
    }

    pub(crate) fn one_shot(options: ExecCommandHandlerOptions) -> Self {
        Self {
            options,
            lifetime: ExecCommandLifetime::OneShot,
        }
    }
}

impl ToolExecutor<ToolInvocation> for ExecCommandHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("exec_command")
    }

    fn spec(&self) -> ToolSpec {
        let spec = create_exec_command_tool_with_environment_id(
            CommandToolOptions {
                exec_permission_approvals_enabled: self.options.exec_permission_approvals_enabled,
            },
            self.options.include_environment_id,
            self.options.include_windows_shell_guidance,
        );
        let mut spec = match self.lifetime {
            ExecCommandLifetime::Interactive => spec,
            ExecCommandLifetime::OneShot => one_shot_exec_command_spec(spec),
        };
        if !self.options.allow_tty
            && let ToolSpec::Function(spec) = &mut spec
        {
            spec.parameters
                .properties
                .get_or_insert_default()
                .remove("tty");
        }
        spec
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl ExecCommandHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            environments,
            session,
            step_context,
            cancellation_token,
            tracker,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "exec_command handler received unsupported payload".to_string(),
                ));
            }
        };

        let manager: &UnifiedExecProcessManager = &session.services.unified_exec_manager;
        let context = UnifiedExecContext::new(
            session.clone(),
            step_context.clone(),
            cancellation_token,
            call_id.clone(),
        );
        let turn_environment = selected_executor(&environments, &step_context.environments)?;
        let parsed =
            ParsedExecCommand::parse(&arguments, &session, &step_context, turn_environment)?;
        maybe_emit_implicit_skill_invocation(
            &session,
            &step_context,
            &parsed.args.cmd,
            &parsed.cwd,
            parsed.native_cwd.as_ref(),
            &turn_environment.selection.environment_id,
        )
        .await;
        let prepared = parsed.prepare(self.lifetime)?;
        let max_output_tokens = prepared.max_output_tokens;
        let fs = prepared.turn_environment.environment.get_filesystem();
        let intercepted_patch = intercept_apply_patch(
            &prepared.resolved.command,
            &prepared.cwd,
            fs.as_ref(),
            prepared.turn_environment.clone(),
            context.session.clone(),
            Arc::clone(&context.step_context),
            context.cancellation_token.clone(),
            Some(&tracker),
            &context.call_id,
            "exec_command",
        )
        .await;
        if let Some(output) = intercepted_patch? {
            return Ok(boxed_tool_output(ExecCommandToolOutput {
                event_call_id: String::new(),
                chunk_id: String::new(),
                wall_time: std::time::Duration::ZERO,
                raw_output: CapturedStream {
                    text: output.into_text(),
                    omitted_bytes: 0,
                },
                truncation_policy: step_context.settings.model_info.truncation_policy.into(),
                max_output_tokens,
                process_id: None,
                exit_code: None,
                original_token_count: None,
                hook_command: None,
                tool_cwd: None,
            }));
        }

        emit_unified_exec_tty_metric(&step_context.session_telemetry, prepared.tty);
        crate::tools::lifecycle::notify_command_start(
            context.session.as_ref(),
            context.step_context.turn.as_ref(),
            &context.call_id,
            &prepared.resolved.command,
            &prepared.cwd,
            fs.as_ref(),
        )
        .await;
        // Preparation can be cancelled. Reserve a process only once it is done.
        let process_id = manager.allocate_process_id().await;
        let hook_command = prepared.hook_command.clone();
        let tool_cwd =
            (!prepared.turn_environment.environment.is_remote()).then(|| prepared.cwd.clone());
        let request = ExecCommandRequest {
            prepared,
            process_id,
            network: context.step_context.turn.network.clone(),
        };
        let result = match request.prepared.completion_timeout {
            Some(timeout) => {
                UnifiedExecProcessManager::exec_command_to_completion(request, &context, timeout)
                    .await
            }
            None => manager.exec_command(request, &context).await,
        };
        match result {
            Ok(response) => Ok(boxed_tool_output(response)),
            Err(UnifiedExecError::SandboxDenied {
                output,
                original_token_count,
                output_omitted_bytes,
                ..
            }) => {
                let output_text = output.aggregated_output.text;
                let original_token_count =
                    original_token_count.unwrap_or_else(|| approx_token_count(&output_text));
                Ok(boxed_tool_output(ExecCommandToolOutput {
                    event_call_id: context.call_id.clone(),
                    chunk_id: generate_chunk_id(),
                    wall_time: output.duration,
                    raw_output: CapturedStream {
                        text: output_text,
                        omitted_bytes: output_omitted_bytes.map_or(0, std::num::NonZeroUsize::get),
                    },
                    truncation_policy: step_context.settings.model_info.truncation_policy.into(),
                    max_output_tokens,
                    // Sandbox denial is terminal, so there is no live
                    // process for write_stdin to resume.
                    process_id: None,
                    exit_code: Some(output.exit_code),
                    original_token_count: Some(original_token_count),
                    hook_command: Some(hook_command),
                    tool_cwd,
                }))
            }
            Err(err) => {
                let message = format!("exec_command failed: {err:?}");
                Err(FunctionCallError::RespondToModel(truncate_middle_chars(
                    &message,
                    EXEC_COMMAND_REJECTION_MAX_BYTES,
                )))
            }
        }
    }
}

fn one_shot_exec_command_spec(spec: ToolSpec) -> ToolSpec {
    let ToolSpec::Function(mut spec) = spec else {
        unreachable!("exec_command has a function schema");
    };
    spec.description = spec.description.replacen(
        "Runs a command in a PTY, returning output or a session ID for ongoing interaction.",
        "Runs a command to completion and returns its output. The process is terminated on timeout or cancellation and cannot be resumed.",
        1,
    );
    let properties = spec.parameters.properties.get_or_insert_default();
    properties.remove("tty");
    properties.remove("yield_time_ms");
    properties.insert(
        "timeout_ms".to_string(),
        JsonSchema::number(Some(
            "Maximum command runtime. Defaults to 10000 ms.".to_string(),
        )),
    );
    spec.output_schema = spec.output_schema.map(|schema| {
        let mut schema = schema.into_value();
        if let Some(output_properties) = schema
            .get_mut("properties")
            .and_then(serde_json::Value::as_object_mut)
        {
            output_properties.remove("session_id");
        }
        schema.into()
    });
    ToolSpec::Function(spec)
}

impl CoreToolRuntime for ExecCommandHandler {
    fn telemetry_tags(&self, invocation: &ToolInvocation) -> ToolTelemetryTags {
        // Record the configured mode so fallback calls remain in their rollout cohort.
        // This does not indicate whether the command actually used a snapshot.
        let mode = if invocation.session.enabled(Feature::ShellSnapshotV2) {
            "v2"
        } else if invocation.session.enabled(Feature::ShellSnapshot) {
            "v1"
        } else {
            "disabled"
        };
        vec![("shell_snapshot_mode", mode.to_string())]
    }

    fn environments_for_input(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<ToolEnvironments, FunctionCallError> {
        function_tool_environment(invocation, |args: ToolEnvironmentArgs| args.environment_id)
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        parse_arguments::<ExecCommandArgs>(arguments)
            .ok()
            .map(|args| PreToolUsePayload {
                tool_name: HookToolName::bash(),
                tool_input: serde_json::json!({ "command": args.cmd }),
            })
    }

    fn pre_tool_use_cwd(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<Option<PathUri>, FunctionCallError> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return Ok(None);
        };
        let args: ExecCommandWorkdirArgs = parse_arguments(arguments)?;
        let environment = selected_executor(
            &invocation.environments,
            &invocation.step_context.environments,
        )?;
        let cwd = args.resolve_cwd(environment)?;
        // Hook scripts execute on the host. Remote command paths cannot replace
        // the host cwd in hook input, even when both machines use the same OS.
        Ok((!environment.environment.is_remote()).then_some(cwd))
    }

    fn with_updated_hook_input(
        &self,
        mut invocation: ToolInvocation,
        updated_input: serde_json::Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "hook input rewrite received unsupported exec_command payload".to_string(),
            ));
        };
        invocation.payload = ToolPayload::Function {
            arguments: rewrite_function_string_argument(
                &arguments,
                "exec_command",
                "cmd",
                updated_hook_command(&updated_input)?,
            )?,
        };
        Ok(invocation)
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn crate::tools::context::ToolOutput,
    ) -> Option<PostToolUsePayload> {
        post_unified_exec_tool_use_payload(invocation, result)
    }
}

fn emit_unified_exec_tty_metric(session_telemetry: &SessionTelemetry, tty: bool) {
    session_telemetry.counter(
        TOOL_CALL_UNIFIED_EXEC_METRIC,
        /*inc*/ 1,
        &[("tty", if tty { "true" } else { "false" })],
    );
}

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::apply_patch;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::function_tool::FunctionCallError;
use crate::safety::PatchPolicyMatcher;
use crate::safety::PatchSandboxRoute;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::context::ApplyPatchToolOutput;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::apply_granted_turn_permissions;
use crate::tools::handlers::apply_patch_spec::create_apply_patch_freeform_tool;
use crate::tools::handlers::file_system_sandbox_policy_context_for_cwd;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::handlers::updated_hook_command;
use crate::tools::hook_names::HookToolName;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::registry::ToolExecutor;
use crate::tools::runtimes::apply_patch::ApplyPatchRequest;
use crate::tools::runtimes::apply_patch::ApplyPatchRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::windows_sandbox::windows_sandbox_level_for_legacy_checks;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_apply_patch::ApplyPatchFileUpdateMode;
use codex_apply_patch::Hunk;
use codex_apply_patch::StreamingPatchParser;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::PatchApplyUpdatedEvent;
use codex_sandboxing::policy_transforms::effective_file_system_sandbox_policy;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_sandboxing::policy_transforms::normalize_additional_permissions;
use codex_sandboxing::policy_transforms::normalize_additional_permissions_with_context;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_path_uri::PathUri;

const APPLY_PATCH_ARGUMENT_DIFF_BUFFER_INTERVAL: Duration = Duration::from_millis(500);

fn apply_patch_file_update_mode(turn: &TurnContext) -> ApplyPatchFileUpdateMode {
    if turn
        .config
        .features
        .enabled(Feature::ApplyPatchPreserveLineEndings)
    {
        ApplyPatchFileUpdateMode::PreserveLineEndings
    } else {
        ApplyPatchFileUpdateMode::NormalizeToLf
    }
}

/// Handles freeform `apply_patch` requests and routes verified patches to the
/// selected environment filesystem.
#[derive(Default)]
pub struct ApplyPatchHandler {
    multi_environment: bool,
}

impl ApplyPatchHandler {
    pub(crate) fn new(multi_environment: bool) -> Self {
        Self { multi_environment }
    }
}

#[derive(Default)]
struct ApplyPatchArgumentDiffConsumer {
    parser: StreamingPatchParser,
    last_sent_at: Option<Instant>,
    pending: Option<PatchApplyUpdatedEvent>,
}

impl ToolArgumentDiffConsumer for ApplyPatchArgumentDiffConsumer {
    fn consume_diff(
        &mut self,
        turn: &TurnContext,
        call_id: String,
        diff: &str,
    ) -> Option<EventMsg> {
        if !turn
            .config
            .features
            .enabled(Feature::ApplyPatchStreamingEvents)
        {
            return None;
        }

        self.push_delta(call_id, diff)
            .map(EventMsg::PatchApplyUpdated)
    }

    fn finish(&mut self) -> Result<Option<EventMsg>, FunctionCallError> {
        self.finish_update_on_complete()
            .map(|event| event.map(EventMsg::PatchApplyUpdated))
    }
}

impl ApplyPatchArgumentDiffConsumer {
    fn push_delta(&mut self, call_id: String, delta: &str) -> Option<PatchApplyUpdatedEvent> {
        let hunks = self.parser.push_delta(delta).ok()?;
        if hunks.is_empty() {
            return None;
        }
        let changes = convert_apply_patch_hunks_to_protocol(&hunks);
        let event = PatchApplyUpdatedEvent { call_id, changes };
        let now = Instant::now();
        match self.last_sent_at {
            Some(last_sent_at)
                if now.duration_since(last_sent_at) < APPLY_PATCH_ARGUMENT_DIFF_BUFFER_INTERVAL =>
            {
                self.pending = Some(event);
                None
            }
            Some(_) | None => {
                self.pending = None;
                self.last_sent_at = Some(now);
                Some(event)
            }
        }
    }

    fn finish_update_on_complete(
        &mut self,
    ) -> Result<Option<PatchApplyUpdatedEvent>, FunctionCallError> {
        self.parser.finish().map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to parse apply_patch: {err}"))
        })?;

        let event = self.pending.take();
        if event.is_some() {
            self.last_sent_at = Some(Instant::now());
        }
        Ok(event)
    }
}

fn convert_apply_patch_hunks_to_protocol(hunks: &[Hunk]) -> HashMap<PathBuf, FileChange> {
    hunks
        .iter()
        .map(|hunk| {
            let path = hunk_source_path(hunk).to_path_buf();
            let change = match hunk {
                Hunk::AddFile { contents, .. } => FileChange::Add {
                    content: contents.clone(),
                },
                Hunk::DeleteFile { .. } => FileChange::Delete {
                    content: String::new(),
                },
                Hunk::UpdateFile {
                    chunks, move_path, ..
                } => FileChange::Update {
                    unified_diff: format_update_chunks_for_progress(chunks),
                    move_path: move_path.clone(),
                },
            };
            (path, change)
        })
        .collect()
}

fn hunk_source_path(hunk: &Hunk) -> &Path {
    match hunk {
        Hunk::AddFile { path, .. } | Hunk::DeleteFile { path } | Hunk::UpdateFile { path, .. } => {
            path
        }
    }
}

fn format_update_chunks_for_progress(chunks: &[codex_apply_patch::UpdateFileChunk]) -> String {
    let mut unified_diff = String::new();
    for chunk in chunks {
        match &chunk.change_context {
            Some(context) => {
                unified_diff.push_str("@@ ");
                unified_diff.push_str(context);
                unified_diff.push('\n');
            }
            None => {
                unified_diff.push_str("@@");
                unified_diff.push('\n');
            }
        }
        for line in &chunk.old_lines {
            unified_diff.push('-');
            unified_diff.push_str(line);
            unified_diff.push('\n');
        }
        for line in &chunk.new_lines {
            unified_diff.push('+');
            unified_diff.push_str(line);
            unified_diff.push('\n');
        }
        if chunk.is_end_of_file {
            unified_diff.push_str("*** End of File");
            unified_diff.push('\n');
        }
    }
    unified_diff
}

fn file_paths_for_action(action: &ApplyPatchAction) -> Vec<PathUri> {
    let mut keys = Vec::new();
    for (path, change) in action.changes() {
        keys.push(path.clone());

        if let ApplyPatchFileChange::Update { move_path, .. } = change
            && let Some(dest) = move_path
        {
            keys.push(dest.clone());
        }
    }

    keys
}

fn write_permissions_for_paths(
    file_paths: &[PathUri],
    matching: &PatchPolicyMatcher<'_>,
) -> io::Result<Option<AdditionalPermissionProfile>> {
    let sandbox_route = matching.sandbox_route;
    let context = &matching.context;
    let mut write_paths = Vec::new();
    for path in file_paths {
        // Skip already-writable targets before deriving parent permissions.
        // Otherwise, a writable directory could grant access to its parent.
        if matching.can_write_path(path)? {
            continue;
        }
        let parent = path
            .parent()
            .or_else(|| match sandbox_route {
                PatchSandboxRoute::Platform(_) => {
                    // Host path rules can recover parents of opaque local paths.
                    // `to_abs_path` verifies that the target round-trips losslessly.
                    path.to_abs_path().ok()?.parent().map(PathUri::from)
                }
                PatchSandboxRoute::ExecutorManaged => None,
            })
            .unwrap_or_else(|| path.clone());
        if !matching.can_write_path(&parent)? {
            write_paths.push(parent);
        }
    }
    write_paths.sort_by_key(PathUri::to_string);
    write_paths.dedup();

    if write_paths.is_empty() {
        return Ok(None);
    }
    let permissions = AdditionalPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_path_uris(
            Some(vec![]),
            Some(write_paths),
        )),
        ..Default::default()
    };

    Ok(match sandbox_route {
        PatchSandboxRoute::Platform(_) => normalize_additional_permissions(permissions).ok(),
        PatchSandboxRoute::ExecutorManaged => {
            normalize_additional_permissions_with_context(permissions, context).ok()
        }
    })
}

/// Extracts the raw patch text used as the command-shaped hook input for apply_patch.
fn apply_patch_payload_command(payload: &ToolPayload) -> Option<String> {
    match payload {
        ToolPayload::Custom { input } => Some(input.clone()),
        _ => None,
    }
}

impl ToolExecutor<ToolInvocation> for ApplyPatchHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("apply_patch")
    }

    fn spec(&self) -> ToolSpec {
        create_apply_patch_freeform_tool(self.multi_environment)
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl ApplyPatchHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            step_context,
            cancellation_token,
            tracker,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let ToolPayload::Custom { input: patch_input } = payload else {
            return Err(FunctionCallError::RespondToModel(
                "apply_patch handler received unsupported payload".to_string(),
            ));
        };
        let args = match codex_apply_patch::parse_patch(&patch_input) {
            Ok(args) => args,
            Err(parse_error) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "apply_patch verification failed: {parse_error}"
                )));
            }
        };
        let selected_environment_id =
            require_environment_id(args.environment_id.as_deref(), self.multi_environment)?;

        // Verify the parsed patch against the selected environment filesystem.
        let Some(turn_environment) = resolve_tool_environment(
            &step_context.environments,
            selected_environment_id.as_deref(),
        )?
        else {
            return Err(FunctionCallError::RespondToModel(
                "apply_patch is unavailable in this session".to_string(),
            ));
        };
        let fs = turn_environment.environment.get_filesystem();
        let sandbox = turn_environment.sandbox_context(/*additional_permissions*/ None);
        match codex_apply_patch::verify_apply_patch_args_with_mode(
            args,
            turn_environment.cwd(),
            apply_patch_file_update_mode(&turn),
            fs.as_ref(),
            Some(&sandbox),
        )
        .await
        {
            codex_apply_patch::MaybeApplyPatchVerified::Body(changes) => {
                let tool_ctx = ToolCtx {
                    session,
                    step_context: Arc::clone(&step_context),
                    cancellation_token,
                    call_id,
                    tool_name,
                };
                let content = execute_verified_patch(
                    changes,
                    turn_environment.clone(),
                    Some(&tracker),
                    tool_ctx,
                )
                .await?;
                Ok(boxed_tool_output(ApplyPatchToolOutput::from_text(content)))
            }
            codex_apply_patch::MaybeApplyPatchVerified::CorrectnessError(parse_error) => {
                Err(FunctionCallError::RespondToModel(format!(
                    "apply_patch verification failed: {parse_error}"
                )))
            }
            codex_apply_patch::MaybeApplyPatchVerified::ShellParseError(error) => {
                tracing::trace!("Failed to parse apply_patch input, {error:?}");
                Err(FunctionCallError::RespondToModel(
                    "apply_patch handler received invalid patch input".to_string(),
                ))
            }
            codex_apply_patch::MaybeApplyPatchVerified::NotApplyPatch => {
                Err(FunctionCallError::RespondToModel(
                    "apply_patch handler received non-apply_patch input".to_string(),
                ))
            }
        }
    }
}

impl CoreToolRuntime for ApplyPatchHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }

    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        Some(Box::<ApplyPatchArgumentDiffConsumer>::default())
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        apply_patch_payload_command(&invocation.payload).map(|command| PreToolUsePayload {
            tool_name: HookToolName::apply_patch(),
            tool_input: serde_json::json!({ "command": command }),
        })
    }

    fn with_updated_hook_input(
        &self,
        mut invocation: ToolInvocation,
        updated_input: serde_json::Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        let patch = updated_hook_command(&updated_input)?;
        invocation.payload = match invocation.payload {
            ToolPayload::Custom { .. } => ToolPayload::Custom {
                input: patch.to_string(),
            },
            payload => payload,
        };
        Ok(invocation)
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn crate::tools::context::ToolOutput,
    ) -> Option<PostToolUsePayload> {
        let tool_response =
            result.post_tool_use_response(&invocation.call_id, &invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: HookToolName::apply_patch(),
            tool_use_id: invocation.call_id.clone(),
            tool_input: serde_json::json!({
                "command": apply_patch_payload_command(&invocation.payload)?,
            }),
            tool_response,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn intercept_apply_patch(
    command: &[String],
    cwd: &PathUri,
    fs: &dyn ExecutorFileSystem,
    turn_environment: TurnEnvironment,
    session: Arc<Session>,
    step_context: Arc<StepContext>,
    cancellation_token: CancellationToken,
    tracker: Option<&SharedTurnDiffTracker>,
    call_id: &str,
    tool_name: &str,
) -> Result<Option<FunctionToolOutput>, FunctionCallError> {
    let turn = &step_context.turn;
    let sandbox = turn_environment.sandbox_context(/*additional_permissions*/ None);
    match codex_apply_patch::maybe_parse_apply_patch_verified_with_mode(
        command,
        cwd,
        apply_patch_file_update_mode(turn),
        fs,
        Some(&sandbox),
    )
    .await
    {
        codex_apply_patch::MaybeApplyPatchVerified::Body(changes) => {
            let tool_ctx = ToolCtx {
                session,
                step_context,
                cancellation_token,
                call_id: call_id.to_string(),
                tool_name: ToolName::plain(tool_name),
            };
            let content =
                execute_verified_patch(changes, turn_environment, tracker, tool_ctx).await?;
            Ok(Some(FunctionToolOutput::from_text(content, Some(true))))
        }
        codex_apply_patch::MaybeApplyPatchVerified::CorrectnessError(parse_error) => {
            Err(FunctionCallError::RespondToModel(format!(
                "apply_patch verification failed: {parse_error}"
            )))
        }
        codex_apply_patch::MaybeApplyPatchVerified::ShellParseError(error) => {
            tracing::trace!("Failed to parse apply_patch input, {error:?}");
            Ok(None)
        }
        codex_apply_patch::MaybeApplyPatchVerified::NotApplyPatch => Ok(None),
    }
}

async fn execute_verified_patch(
    action: ApplyPatchAction,
    turn_environment: TurnEnvironment,
    tracker: Option<&SharedTurnDiffTracker>,
    tool_ctx: ToolCtx,
) -> Result<String, FunctionCallError> {
    let cwd = action.cwd.clone();
    let sandbox_context = turn_environment.sandbox_context(/*additional_permissions*/ None);
    let policy_context = file_system_sandbox_policy_context_for_cwd(&sandbox_context, &cwd);
    let sandbox_route = if turn_environment.environment.is_remote() {
        PatchSandboxRoute::ExecutorManaged
    } else {
        PatchSandboxRoute::Platform(windows_sandbox_level_for_legacy_checks(
            turn_environment.config().windows_sandbox_type,
            turn_environment.config().windows_sandbox_level,
        ))
    };
    let environment_id = turn_environment.selection.environment_id.as_str();
    let file_paths = file_paths_for_action(&action);
    let granted_permissions = merge_permission_profiles(
        tool_ctx
            .session
            .granted_session_permissions(environment_id)
            .await
            .as_ref(),
        tool_ctx
            .session
            .granted_turn_permissions(environment_id)
            .await
            .as_ref(),
    );
    let base_file_system_sandbox_policy = turn_environment
        .permission_profile()
        .file_system_sandbox_policy();
    let file_system_sandbox_policy = effective_file_system_sandbox_policy(
        &base_file_system_sandbox_policy,
        granted_permissions.as_ref(),
    );
    let matching = sandbox_route
        .prepare_matching(&file_system_sandbox_policy, &policy_context)
        .map_err(|error| {
            FunctionCallError::RespondToModel(format!(
                "failed to prepare patch permissions: {error}"
            ))
        })?;
    let additional_permissions =
        write_permissions_for_paths(&file_paths, &matching).map_err(|error| {
            FunctionCallError::RespondToModel(format!("failed to check patch permissions: {error}"))
        })?;
    let effective_additional_permissions = apply_granted_turn_permissions(
        tool_ctx.session.as_ref(),
        &turn_environment,
        &cwd,
        crate::sandboxing::SandboxPermissions::UseDefault,
        additional_permissions,
    )
    .await;
    let apply = apply_patch::prepare_apply_patch(
        &tool_ctx.step_context,
        &turn_environment,
        &matching,
        action,
    )?;
    let changes = convert_apply_patch_to_protocol(&apply.action);
    let emitter = ToolEmitter::apply_patch_for_environment(
        changes.clone(),
        apply.auto_approved,
        turn_environment.selection.environment_id.clone(),
    );
    let event_ctx = ToolEventCtx::new(
        tool_ctx.session.as_ref(),
        tool_ctx.step_context.turn.as_ref(),
        &tool_ctx.step_context.settings.model_info,
        &tool_ctx.call_id,
        tracker,
    );
    emitter.begin(event_ctx).await;

    let request = ApplyPatchRequest {
        turn_environment,
        action: apply.action,
        file_paths,
        changes: Arc::new(changes),
        exec_approval_requirement: apply.exec_approval_requirement,
        additional_permissions: effective_additional_permissions.additional_permissions,
        permissions_preapproved: effective_additional_permissions.permissions_preapproved,
    };
    let mut orchestrator = ToolOrchestrator::new();
    let mut runtime = ApplyPatchRuntime::new();
    let result = orchestrator
        .run(&mut runtime, &request, &tool_ctx)
        .await
        .map(|result| result.output);
    let (result, delta) = match result {
        Ok(output) => (Ok(output.exec_output), Some(output.delta)),
        Err(error) => (Err(error), Some(runtime.committed_delta().clone())),
    };
    let event_ctx = ToolEventCtx::new(
        tool_ctx.session.as_ref(),
        tool_ctx.step_context.turn.as_ref(),
        &tool_ctx.step_context.settings.model_info,
        &tool_ctx.call_id,
        tracker,
    );
    emitter.finish(event_ctx, result, delta.as_ref()).await
}

fn require_environment_id(
    parsed_environment_id: Option<&str>,
    allow_environment_id: bool,
) -> Result<Option<String>, FunctionCallError> {
    match parsed_environment_id {
        Some(_) if !allow_environment_id => Err(FunctionCallError::RespondToModel(
            "apply_patch environment selection is unavailable for this turn".to_string(),
        )),
        Some(environment_id) => Ok(Some(environment_id.to_string())),
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;

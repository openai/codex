use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::apply_patch;
use crate::apply_patch::InternalApplyPatchInvocation;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::ApplyPatchToolOutput;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::apply_granted_turn_permissions;
use crate::tools::handlers::parse_arguments;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::apply_patch::ApplyPatchRequest;
use crate::tools::runtimes::apply_patch::ApplyPatchRuntime;
use crate::tools::sandboxing::ToolCtx;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchArgs;
use codex_apply_patch::ApplyPatchFileChange;
use codex_apply_patch::Hunk;
use codex_apply_patch::parse_patch_streaming;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::PatchApplyUpdatedEvent;
use codex_sandboxing::policy_transforms::effective_file_system_sandbox_policy;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_sandboxing::policy_transforms::normalize_additional_permissions;
use codex_tools::ApplyPatchToolArgs;
use codex_utils_absolute_path::AbsolutePathBuf;

const APPLY_PATCH_ARGUMENT_DIFF_BUFFER_INTERVAL: Duration = Duration::from_millis(500);
const APPLY_PATCH_ENVIRONMENT_PREFIX: &str = "*** Environment: ";

pub struct ApplyPatchHandler;

#[derive(Default)]
struct ApplyPatchArgumentDiffConsumer {
    input: String,
    last_progress: Option<Vec<Hunk>>,
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
        if !turn.features.enabled(Feature::ApplyPatchStreamingEvents) {
            return None;
        }

        self.push_delta(call_id, diff)
            .map(EventMsg::PatchApplyUpdated)
    }

    fn flush_on_complete(&mut self) -> Option<EventMsg> {
        self.flush_update_on_complete()
            .map(EventMsg::PatchApplyUpdated)
    }
}

impl ApplyPatchArgumentDiffConsumer {
    fn push_delta(&mut self, call_id: String, delta: &str) -> Option<PatchApplyUpdatedEvent> {
        self.input.push_str(delta);

        let ApplyPatchArgs { hunks, .. } = parse_patch_streaming(&self.input).ok()?;
        if hunks.is_empty() {
            return None;
        }
        if self.last_progress.as_ref() == Some(&hunks) {
            return None;
        }

        let changes = convert_apply_patch_hunks_to_protocol(&hunks);
        self.last_progress = Some(hunks);
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

    fn flush_update_on_complete(&mut self) -> Option<PatchApplyUpdatedEvent> {
        let event = self.pending.take();
        if event.is_some() {
            self.last_sent_at = Some(Instant::now());
        }
        event
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

fn file_paths_for_action(action: &ApplyPatchAction) -> Vec<AbsolutePathBuf> {
    let mut keys = Vec::new();
    let cwd = &action.cwd;

    for (path, change) in action.changes() {
        if let Some(key) = to_abs_path(cwd, path) {
            keys.push(key);
        }

        if let ApplyPatchFileChange::Update { move_path, .. } = change
            && let Some(dest) = move_path
            && let Some(key) = to_abs_path(cwd, dest)
        {
            keys.push(key);
        }
    }

    keys
}

fn to_abs_path(cwd: &AbsolutePathBuf, path: &Path) -> Option<AbsolutePathBuf> {
    Some(AbsolutePathBuf::resolve_path_against_base(path, cwd))
}

fn write_permissions_for_paths(
    file_paths: &[AbsolutePathBuf],
    file_system_sandbox_policy: &codex_protocol::permissions::FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> Option<PermissionProfile> {
    let write_paths = file_paths
        .iter()
        .map(|path| {
            path.parent()
                .unwrap_or_else(|| path.clone())
                .into_path_buf()
        })
        .filter(|path| {
            !file_system_sandbox_policy.can_write_path_with_cwd(path.as_path(), cwd.as_path())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(AbsolutePathBuf::from_absolute_path)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;

    let permissions = (!write_paths.is_empty()).then_some(PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(write_paths),
        )),
        ..Default::default()
    })?;

    normalize_additional_permissions(permissions).ok()
}

fn split_freeform_apply_patch_environment(
    input: String,
) -> Result<(String, Option<String>), FunctionCallError> {
    let Some(rest) = input.strip_prefix(APPLY_PATCH_ENVIRONMENT_PREFIX) else {
        return Ok((input, None));
    };
    let Some((environment_id, patch_input)) = rest.split_once('\n') else {
        return Err(FunctionCallError::RespondToModel(
            "apply_patch environment header must be followed by a patch body".to_string(),
        ));
    };
    let environment_id = environment_id.trim();
    if environment_id.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "apply_patch environment header must include an environment id".to_string(),
        ));
    }
    Ok((patch_input.to_string(), Some(environment_id.to_string())))
}

fn merge_apply_patch_environment_ids(
    argument_environment_id: Option<String>,
    header_environment_id: Option<String>,
) -> Result<Option<String>, FunctionCallError> {
    match (argument_environment_id, header_environment_id) {
        (Some(argument_environment_id), Some(header_environment_id))
            if argument_environment_id != header_environment_id =>
        {
            Err(FunctionCallError::RespondToModel(format!(
                "apply_patch environment id mismatch: argument requested `{argument_environment_id}` but patch header requested `{header_environment_id}`"
            )))
        }
        (Some(environment_id), Some(_)) | (Some(environment_id), None) => Ok(Some(environment_id)),
        (None, Some(environment_id)) => Ok(Some(environment_id)),
        (None, None) => Ok(None),
    }
}

async fn effective_patch_permissions(
    session: &Session,
    turn: &TurnContext,
    cwd: &AbsolutePathBuf,
    action: &ApplyPatchAction,
) -> (
    Vec<AbsolutePathBuf>,
    crate::tools::handlers::EffectiveAdditionalPermissions,
    codex_protocol::permissions::FileSystemSandboxPolicy,
) {
    let file_paths = file_paths_for_action(action);
    let granted_permissions = merge_permission_profiles(
        session.granted_session_permissions().await.as_ref(),
        session.granted_turn_permissions().await.as_ref(),
    );
    let file_system_sandbox_policy = effective_file_system_sandbox_policy(
        &turn.file_system_sandbox_policy,
        granted_permissions.as_ref(),
    );
    let effective_additional_permissions = apply_granted_turn_permissions(
        session,
        turn.cwd.as_path(),
        crate::sandboxing::SandboxPermissions::UseDefault,
        write_permissions_for_paths(&file_paths, &file_system_sandbox_policy, cwd),
    )
    .await;

    (
        file_paths,
        effective_additional_permissions,
        file_system_sandbox_policy,
    )
}

impl ToolHandler for ApplyPatchHandler {
    type Output = ApplyPatchToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::Custom { .. }
        )
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        Some(Box::<ApplyPatchArgumentDiffConsumer>::default())
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

        let multi_environment_tools = turn.tools_config.multi_environment_tools;
        let (patch_input, requested_environment_id) = match payload {
            ToolPayload::Function { arguments } => {
                let args: ApplyPatchToolArgs = parse_arguments(&arguments)?;
                if multi_environment_tools {
                    let (patch_input, header_environment_id) =
                        split_freeform_apply_patch_environment(args.input)?;
                    let environment_id = merge_apply_patch_environment_ids(
                        args.environment_id,
                        header_environment_id,
                    )?;
                    (patch_input, environment_id)
                } else {
                    (args.input, None)
                }
            }
            ToolPayload::Custom { input } => {
                if multi_environment_tools {
                    split_freeform_apply_patch_environment(input)?
                } else {
                    (input, None)
                }
            }
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "apply_patch handler received unsupported payload".to_string(),
                ));
            }
        };

        // Re-parse and verify the patch so we can compute changes and approval.
        // Avoid building temporary ExecParams/command vectors; derive directly from inputs.
        let tool_environment = turn
            .environment_for_tool(requested_environment_id.as_deref())
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(match requested_environment_id.as_deref() {
                    Some(environment_id) => {
                        format!("environment `{environment_id}` is unavailable in this session")
                    }
                    None => "apply_patch is unavailable in this session".to_string(),
                })
            })?;
        let cwd = tool_environment.cwd.clone();
        let command = vec!["apply_patch".to_string(), patch_input.clone()];
        let fs = tool_environment.environment.get_filesystem();
        let sandbox = tool_environment.environment.is_remote().then(|| {
            turn.file_system_sandbox_context_for_cwd(&cwd, /*additional_permissions*/ None)
        });
        match codex_apply_patch::maybe_parse_apply_patch_verified(
            &command,
            &cwd,
            fs.as_ref(),
            sandbox.as_ref(),
        )
        .await
        {
            codex_apply_patch::MaybeApplyPatchVerified::Body(changes) => {
                let action_cwd = changes.cwd.clone();
                let (file_paths, effective_additional_permissions, file_system_sandbox_policy) =
                    effective_patch_permissions(
                        session.as_ref(),
                        turn.as_ref(),
                        &action_cwd,
                        &changes,
                    )
                    .await;
                match apply_patch::apply_patch(
                    turn.as_ref(),
                    &action_cwd,
                    &file_system_sandbox_policy,
                    changes,
                )
                .await
                {
                    InternalApplyPatchInvocation::Output(item) => {
                        let content = item?;
                        Ok(ApplyPatchToolOutput::from_text(content))
                    }
                    InternalApplyPatchInvocation::DelegateToRuntime(apply) => {
                        let changes = convert_apply_patch_to_protocol(&apply.action);
                        let emitter =
                            ToolEmitter::apply_patch(changes.clone(), apply.auto_approved);
                        let event_ctx = ToolEventCtx::new(
                            session.as_ref(),
                            turn.as_ref(),
                            &call_id,
                            Some(&tracker),
                        );
                        emitter.begin(event_ctx).await;

                        let req = ApplyPatchRequest {
                            environment_id: tool_environment.environment_id.clone(),
                            environment: tool_environment.environment,
                            cwd: action_cwd,
                            action: apply.action,
                            file_paths,
                            changes,
                            exec_approval_requirement: apply.exec_approval_requirement,
                            additional_permissions: effective_additional_permissions
                                .additional_permissions,
                            permissions_preapproved: effective_additional_permissions
                                .permissions_preapproved,
                        };

                        let mut orchestrator = ToolOrchestrator::new();
                        let mut runtime = ApplyPatchRuntime::new();
                        let tool_ctx = ToolCtx {
                            session: session.clone(),
                            turn: turn.clone(),
                            call_id: call_id.clone(),
                            tool_name: tool_name.display(),
                        };
                        let out = orchestrator
                            .run(
                                &mut runtime,
                                &req,
                                &tool_ctx,
                                turn.as_ref(),
                                turn.approval_policy.value(),
                            )
                            .await
                            .map(|result| result.output);
                        let event_ctx = ToolEventCtx::new(
                            session.as_ref(),
                            turn.as_ref(),
                            &call_id,
                            Some(&tracker),
                        );
                        let content = emitter.finish(event_ctx, out).await?;
                        Ok(ApplyPatchToolOutput::from_text(content))
                    }
                }
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn intercept_apply_patch(
    command: &[String],
    cwd: &AbsolutePathBuf,
    fs: &dyn ExecutorFileSystem,
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: Option<&SharedTurnDiffTracker>,
    call_id: &str,
    tool_name: &str,
) -> Result<Option<FunctionToolOutput>, FunctionCallError> {
    let tool_environment = turn.environment_for_tool(/*environment_id*/ None);
    let sandbox = tool_environment
        .as_ref()
        .filter(|environment| environment.environment.is_remote())
        .map(|_| {
            turn.file_system_sandbox_context_for_cwd(cwd, /*additional_permissions*/ None)
        });
    match codex_apply_patch::maybe_parse_apply_patch_verified(command, cwd, fs, sandbox.as_ref())
        .await
    {
        codex_apply_patch::MaybeApplyPatchVerified::Body(changes) => {
            let action_cwd = changes.cwd.clone();
            session
                .record_model_warning(
                    format!(
                        "apply_patch was requested via {tool_name}. Use the apply_patch tool instead of exec_command."
                    ),
                    turn.as_ref(),
                )
                .await;
            let (approval_keys, effective_additional_permissions, file_system_sandbox_policy) =
                effective_patch_permissions(session.as_ref(), turn.as_ref(), &action_cwd, &changes)
                    .await;
            match apply_patch::apply_patch(
                turn.as_ref(),
                &action_cwd,
                &file_system_sandbox_policy,
                changes,
            )
            .await
            {
                InternalApplyPatchInvocation::Output(item) => {
                    let content = item?;
                    Ok(Some(FunctionToolOutput::from_text(content, Some(true))))
                }
                InternalApplyPatchInvocation::DelegateToRuntime(apply) => {
                    let changes = convert_apply_patch_to_protocol(&apply.action);
                    let emitter = ToolEmitter::apply_patch(changes.clone(), apply.auto_approved);
                    let event_ctx = ToolEventCtx::new(
                        session.as_ref(),
                        turn.as_ref(),
                        call_id,
                        tracker.as_ref().copied(),
                    );
                    emitter.begin(event_ctx).await;

                    let req = ApplyPatchRequest {
                        environment_id: tool_environment
                            .as_ref()
                            .and_then(|environment| environment.environment_id.clone()),
                        environment: tool_environment
                            .as_ref()
                            .map(|environment| Arc::clone(&environment.environment))
                            .ok_or_else(|| {
                                FunctionCallError::RespondToModel(
                                    "apply_patch is unavailable in this session".to_string(),
                                )
                            })?,
                        cwd: action_cwd,
                        action: apply.action,
                        file_paths: approval_keys,
                        changes,
                        exec_approval_requirement: apply.exec_approval_requirement,
                        additional_permissions: effective_additional_permissions
                            .additional_permissions,
                        permissions_preapproved: effective_additional_permissions
                            .permissions_preapproved,
                    };

                    let mut orchestrator = ToolOrchestrator::new();
                    let mut runtime = ApplyPatchRuntime::new();
                    let tool_ctx = ToolCtx {
                        session: session.clone(),
                        turn: turn.clone(),
                        call_id: call_id.to_string(),
                        tool_name: tool_name.to_string(),
                    };
                    let out = orchestrator
                        .run(
                            &mut runtime,
                            &req,
                            &tool_ctx,
                            turn.as_ref(),
                            turn.approval_policy.value(),
                        )
                        .await
                        .map(|result| result.output);
                    let event_ctx = ToolEventCtx::new(
                        session.as_ref(),
                        turn.as_ref(),
                        call_id,
                        tracker.as_ref().copied(),
                    );
                    let content = emitter.finish(event_ctx, out).await?;
                    Ok(Some(FunctionToolOutput::from_text(content, Some(true))))
                }
            }
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

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;

use async_trait::async_trait;
use codex_artifact_spreadsheet::PathAccessKind;
use codex_artifact_spreadsheet::PathAccessRequirement;
use codex_artifact_spreadsheet::SpreadsheetArtifactError;
use codex_artifact_spreadsheet::SpreadsheetArtifactRequest;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use serde_json::to_string;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::path_utils::normalize_for_path_comparison;
use crate::path_utils::resolve_symlink_write_paths;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::sandboxing::with_cached_approval;

pub struct SpreadsheetArtifactHandler;

#[async_trait]
impl ToolHandler for SpreadsheetArtifactHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;

        if !session.enabled(Feature::Artifact) {
            return Err(FunctionCallError::RespondToModel(
                "spreadsheet_artifact is disabled by feature flag".to_string(),
            ));
        }

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "spreadsheet_artifact handler received unsupported payload".to_string(),
                ));
            }
        };

        let request: SpreadsheetArtifactRequest = parse_arguments(&arguments)?;
        for access in request
            .required_path_accesses(&turn.cwd)
            .map_err(spreadsheet_error)?
        {
            authorize_path_access(session.as_ref(), turn.as_ref(), &call_id, &access).await?;
        }

        let response = session
            .execute_spreadsheet_artifact(request, &turn.cwd)
            .await
            .map_err(spreadsheet_error)?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(to_string(&response).map_err(|error| {
                FunctionCallError::RespondToModel(format!(
                    "failed to serialize spreadsheet_artifact response: {error}"
                ))
            })?),
            success: Some(true),
        })
    }
}

fn spreadsheet_error(error: SpreadsheetArtifactError) -> FunctionCallError {
    FunctionCallError::RespondToModel(error.to_string())
}

async fn authorize_path_access(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    access: &PathAccessRequirement,
) -> Result<(), FunctionCallError> {
    let effective_path = match access.kind {
        PathAccessKind::Read => effective_read_path(&access.path),
        PathAccessKind::Write => effective_write_path(&access.path),
    };
    let allowed = match access.kind {
        PathAccessKind::Read => path_is_readable(turn, &effective_path),
        PathAccessKind::Write => path_is_writable(turn, &effective_path),
    };
    if allowed {
        return Ok(());
    }

    let approval_policy = turn.approval_policy.value();
    if !matches!(
        approval_policy,
        AskForApproval::OnRequest | AskForApproval::UnlessTrusted
    ) {
        return Err(FunctionCallError::RespondToModel(format!(
            "{} path `{}` is outside the current sandbox policy",
            access_kind_label(access.kind),
            access.path.display()
        )));
    }

    let key = format!(
        "spreadsheet_artifact:{:?}:{}",
        access.kind,
        effective_path.display()
    );
    let path = access.path.clone();
    let action = access.action.clone();
    let decision =
        with_cached_approval(&session.services, "spreadsheet_artifact", vec![key], || {
            let path = path.clone();
            let action = action.clone();
            async move {
                session
                    .request_command_approval(
                        turn,
                        call_id.to_string(),
                        None,
                        vec![
                            "spreadsheet_artifact".to_string(),
                            action,
                            path.display().to_string(),
                        ],
                        turn.cwd.clone(),
                        Some(format!(
                            "Allow spreadsheet_artifact to {} `{}`?",
                            access_kind_verb(access.kind),
                            path.display()
                        )),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await
            }
        })
        .await;

    if matches!(
        decision,
        ReviewDecision::Approved
            | ReviewDecision::ApprovedForSession
            | ReviewDecision::ApprovedExecpolicyAmendment { .. }
    ) {
        return Ok(());
    }

    Err(FunctionCallError::RespondToModel(format!(
        "{} path `{}` was not approved",
        access_kind_label(access.kind),
        access.path.display()
    )))
}

fn path_is_readable(turn: &TurnContext, path: &Path) -> bool {
    if turn.sandbox_policy.has_full_disk_read_access() {
        return true;
    }

    turn.sandbox_policy
        .get_readable_roots_with_cwd(&turn.cwd)
        .iter()
        .any(|root| path.starts_with(root.as_path()))
}

fn path_is_writable(turn: &TurnContext, path: &Path) -> bool {
    if turn.sandbox_policy.has_full_disk_write_access() {
        return true;
    }

    turn.sandbox_policy
        .get_writable_roots_with_cwd(&turn.cwd)
        .iter()
        .any(|root| root.is_path_writable(path))
}

fn effective_read_path(path: &Path) -> PathBuf {
    normalize_for_path_comparison(path).unwrap_or_else(|_| normalize_without_fs(path))
}

fn effective_write_path(path: &Path) -> PathBuf {
    let write_path = resolve_symlink_write_paths(path)
        .map(|paths| paths.write_path)
        .unwrap_or_else(|_| path.to_path_buf());
    normalize_for_path_comparison(&write_path).unwrap_or_else(|_| normalize_without_fs(&write_path))
}

fn normalize_without_fs(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn access_kind_label(kind: PathAccessKind) -> &'static str {
    match kind {
        PathAccessKind::Read => "read",
        PathAccessKind::Write => "write",
    }
}

fn access_kind_verb(kind: PathAccessKind) -> &'static str {
    match kind {
        PathAccessKind::Read => "read from",
        PathAccessKind::Write => "write to",
    }
}

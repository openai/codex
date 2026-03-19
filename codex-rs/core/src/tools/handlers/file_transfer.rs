use crate::codex::Session;
use crate::codex::TurnContext;
use crate::default_client::get_codex_user_agent;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::effective_file_system_sandbox_policy;
use crate::sandboxing::merge_permission_profiles;
use crate::sandboxing::normalize_additional_permissions;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_workdir_base_path;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::file_transfer::FileTransferRuntime;
use crate::tools::runtimes::file_transfer::InternalFileTransferRequest;
use crate::tools::sandboxing::ToolCtx;
use async_trait::async_trait;
use codex_file_transfer::DownloadFileToolResult;
use codex_file_transfer::FILE_TRANSFER_ACCOUNT_ID_ENV;
use codex_file_transfer::FILE_TRANSFER_BASE_URL_ENV;
use codex_file_transfer::FILE_TRANSFER_BEARER_TOKEN_ENV;
use codex_file_transfer::FILE_TRANSFER_USER_AGENT_ENV;
use codex_file_transfer::FileTransferRequest;
use codex_file_transfer::UploadFileToolResult;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

pub struct FileTransferHandler;

#[derive(Debug, Deserialize)]
struct UploadFileArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct DownloadFileArgs {
    file_id: String,
    path: String,
}

#[derive(Debug)]
struct EffectivePathAccess {
    base_allowed: bool,
    effective_allowed: bool,
}

#[async_trait]
impl ToolHandler for FileTransferHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;
        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "file transfer handler received unsupported payload".to_string(),
                ));
            }
        };

        match tool_name.as_str() {
            "upload_file" => {
                let cwd = resolve_workdir_base_path(&arguments, turn.cwd.as_path())?;
                let args: UploadFileArgs = parse_arguments(&arguments)?;
                let path = crate::util::resolve_path(cwd.as_path(), &PathBuf::from(args.path));
                handle_upload(&session, &turn, &call_id, &tool_name, path).await
            }
            "download_file" => {
                let cwd = resolve_workdir_base_path(&arguments, turn.cwd.as_path())?;
                let args: DownloadFileArgs = parse_arguments(&arguments)?;
                let path = crate::util::resolve_path(cwd.as_path(), &PathBuf::from(args.path));
                let file_id = parse_file_id(&args.file_id).ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "download_file.file_id must be a bare file id or openai-file://v1/{file_id}"
                            .to_string(),
                    )
                })?;
                handle_download(&session, &turn, &call_id, &tool_name, file_id, path).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "unsupported file transfer tool `{tool_name}`"
            ))),
        }
    }
}

async fn handle_upload(
    session: &std::sync::Arc<Session>,
    turn: &std::sync::Arc<TurnContext>,
    call_id: &str,
    tool_name: &str,
    path: PathBuf,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let access =
        effective_path_access(session.as_ref(), turn.as_ref(), &path, AccessKind::Read).await;
    if !access.effective_allowed {
        return json_output(
            &UploadFileToolResult {
                ok: false,
                file_id: None,
                uri: None,
                file_name: None,
                file_size_bytes: None,
                mime_type: None,
                error_code: Some("sandbox_path_denied".to_string()),
                message: Some(format!(
                    "upload path `{}` is outside the current sandbox",
                    path.display()
                )),
                retryable: Some(false),
                http_status_code: None,
                path: Some(path.display().to_string()),
            },
            /*success*/ false,
        );
    }

    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(err) => {
            let error_code = match err.kind() {
                ErrorKind::NotFound => "path_not_found",
                ErrorKind::PermissionDenied => "sandbox_path_denied",
                _ => "upload_failed",
            };
            return json_output(
                &UploadFileToolResult {
                    ok: false,
                    file_id: None,
                    uri: None,
                    file_name: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_string),
                    file_size_bytes: None,
                    mime_type: None,
                    error_code: Some(error_code.to_string()),
                    message: Some(format!("failed to inspect `{}`: {err}", path.display())),
                    retryable: Some(false),
                    http_status_code: None,
                    path: Some(path.display().to_string()),
                },
                /*success*/ false,
            );
        }
    };
    if metadata.is_dir() {
        return json_output(
            &UploadFileToolResult {
                ok: false,
                file_id: None,
                uri: None,
                file_name: None,
                file_size_bytes: None,
                mime_type: None,
                error_code: Some("path_is_directory".to_string()),
                message: Some(format!("upload path `{}` is a directory", path.display())),
                retryable: Some(false),
                http_status_code: None,
                path: Some(path.display().to_string()),
            },
            /*success*/ false,
        );
    }

    let auth = session.services.auth_manager.auth().await;
    let Some(auth) = auth else {
        return json_output(&upload_auth_required(&path), /*success*/ false);
    };
    if !auth.is_chatgpt_auth() {
        return json_output(&upload_auth_required(&path), /*success*/ false);
    }
    let bearer_token = auth.get_token().map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to load chatgpt auth token: {err}"))
    })?;

    let additional_permissions =
        additional_permissions_for_access(&path, AccessKind::Read, &access).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to derive file transfer permissions: {err}"
            ))
        })?;
    let request = InternalFileTransferRequest {
        request: FileTransferRequest::Upload { path: path.clone() },
        cwd: turn.cwd.clone(),
        env: helper_env(
            turn.as_ref(),
            &bearer_token,
            auth.get_account_id().as_deref(),
        ),
        network: turn.network.clone(),
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions,
        codex_exe: turn.codex_linux_sandbox_exe.clone(),
    };
    run_transfer(
        request,
        session,
        turn,
        call_id,
        tool_name,
        TransferKind::Upload,
    )
    .await
}

async fn handle_download(
    session: &std::sync::Arc<Session>,
    turn: &std::sync::Arc<TurnContext>,
    call_id: &str,
    tool_name: &str,
    file_id: String,
    path: PathBuf,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let path_is_directory = tokio::fs::metadata(&path)
        .await
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false);
    let access_path = path.clone();
    if !path_is_directory {
        let Some(parent) = path.parent() else {
            return json_output(
                &DownloadFileToolResult {
                    ok: false,
                    file_id: Some(file_id.clone()),
                    uri: Some(openai_file_uri(&file_id)),
                    file_name: None,
                    mime_type: None,
                    destination_path: Some(path.display().to_string()),
                    bytes_written: None,
                    error_code: Some("destination_parent_missing".to_string()),
                    message: Some(format!(
                        "download destination `{}` has no parent directory",
                        path.display()
                    )),
                    retryable: Some(false),
                    http_status_code: None,
                },
                /*success*/ false,
            );
        };
        if !parent.exists() {
            return json_output(
                &DownloadFileToolResult {
                    ok: false,
                    file_id: Some(file_id.clone()),
                    uri: Some(openai_file_uri(&file_id)),
                    file_name: None,
                    mime_type: None,
                    destination_path: Some(path.display().to_string()),
                    bytes_written: None,
                    error_code: Some("destination_parent_missing".to_string()),
                    message: Some(format!(
                        "download destination parent `{}` does not exist",
                        parent.display()
                    )),
                    retryable: Some(false),
                    http_status_code: None,
                },
                /*success*/ false,
            );
        }
    }

    let access = effective_path_access(
        session.as_ref(),
        turn.as_ref(),
        &access_path,
        AccessKind::Write,
    )
    .await;
    if !access.effective_allowed {
        return json_output(
            &DownloadFileToolResult {
                ok: false,
                file_id: Some(file_id.clone()),
                uri: Some(openai_file_uri(&file_id)),
                file_name: None,
                mime_type: None,
                destination_path: Some(path.display().to_string()),
                bytes_written: None,
                error_code: Some("sandbox_path_denied".to_string()),
                message: Some(format!(
                    "download destination `{}` is outside the current sandbox",
                    path.display()
                )),
                retryable: Some(false),
                http_status_code: None,
            },
            /*success*/ false,
        );
    }

    let auth = session.services.auth_manager.auth().await;
    let Some(auth) = auth else {
        return json_output(
            &download_auth_required(&file_id, &path),
            /*success*/ false,
        );
    };
    if !auth.is_chatgpt_auth() {
        return json_output(
            &download_auth_required(&file_id, &path),
            /*success*/ false,
        );
    }
    let bearer_token = auth.get_token().map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to load chatgpt auth token: {err}"))
    })?;

    let additional_permissions =
        additional_permissions_for_access(&access_path, AccessKind::Write, &access).map_err(
            |err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to derive file transfer permissions: {err}"
                ))
            },
        )?;
    let request = InternalFileTransferRequest {
        request: FileTransferRequest::Download {
            file_id: file_id.clone(),
            path: path.clone(),
            path_is_directory,
        },
        cwd: turn.cwd.clone(),
        env: helper_env(
            turn.as_ref(),
            &bearer_token,
            auth.get_account_id().as_deref(),
        ),
        network: turn.network.clone(),
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions,
        codex_exe: turn.codex_linux_sandbox_exe.clone(),
    };
    run_transfer(
        request,
        session,
        turn,
        call_id,
        tool_name,
        TransferKind::Download,
    )
    .await
}

async fn run_transfer(
    request: InternalFileTransferRequest,
    session: &std::sync::Arc<Session>,
    turn: &std::sync::Arc<TurnContext>,
    call_id: &str,
    tool_name: &str,
    kind: TransferKind,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let mut orchestrator = ToolOrchestrator::new();
    let mut runtime = FileTransferRuntime::new();
    let tool_ctx = ToolCtx {
        session: session.clone(),
        turn: turn.clone(),
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
    };
    let output = orchestrator
        .run(
            &mut runtime,
            &request,
            &tool_ctx,
            turn.as_ref(),
            turn.approval_policy.value(),
        )
        .await;

    match output {
        Ok(result) => parse_helper_output(result.output, kind, &request),
        Err(crate::tools::sandboxing::ToolError::Codex(CodexErr::Sandbox(
            SandboxErr::Denied {
                network_policy_decision,
                ..
            },
        ))) => {
            let content = if network_policy_decision.is_some() {
                transfer_network_denied(kind, &request)
            } else {
                transfer_internal_error(
                    kind,
                    &request,
                    "file transfer helper could not run inside the sandbox".to_string(),
                )
            };
            json_output_value(content, /*success*/ false)
        }
        Err(crate::tools::sandboxing::ToolError::Rejected(message)) => {
            json_output_value(
                transfer_internal_error(kind, &request, message),
                /*success*/ false,
            )
        }
        Err(crate::tools::sandboxing::ToolError::Codex(err)) => json_output_value(
            transfer_internal_error(kind, &request, err.to_string()),
            /*success*/ false,
        ),
    }
}

fn parse_helper_output(
    output: crate::exec::ExecToolCallOutput,
    kind: TransferKind,
    request: &InternalFileTransferRequest,
) -> Result<FunctionToolOutput, FunctionCallError> {
    if output.exit_code != 0 {
        return json_output_value(
            transfer_internal_error(
                kind,
                request,
                format!(
                    "file transfer helper exited with status {}: {}",
                    output.exit_code, output.stderr.text
                ),
            ),
            /*success*/ false,
        );
    }
    let parsed: Value = serde_json::from_str(&output.stdout.text).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "file transfer helper returned invalid JSON: {err}"
        ))
    })?;
    let success = parsed.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let content = serde_json::to_string(&parsed).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to encode file transfer output: {err}"))
    })?;
    Ok(FunctionToolOutput::from_text(content, Some(success)))
}

fn helper_env(
    turn: &TurnContext,
    bearer_token: &str,
    account_id: Option<&str>,
) -> HashMap<String, String> {
    let mut env = HashMap::from([
        (
            FILE_TRANSFER_BASE_URL_ENV.to_string(),
            turn.config.chatgpt_base_url.clone(),
        ),
        (
            FILE_TRANSFER_BEARER_TOKEN_ENV.to_string(),
            bearer_token.to_string(),
        ),
        (
            FILE_TRANSFER_USER_AGENT_ENV.to_string(),
            get_codex_user_agent(),
        ),
    ]);
    if let Some(account_id) = account_id {
        env.insert(
            FILE_TRANSFER_ACCOUNT_ID_ENV.to_string(),
            account_id.to_string(),
        );
    }
    env
}

async fn effective_path_access(
    session: &Session,
    turn: &TurnContext,
    path: &Path,
    kind: AccessKind,
) -> EffectivePathAccess {
    let granted_permissions = merge_permission_profiles(
        session.granted_session_permissions().await.as_ref(),
        session.granted_turn_permissions().await.as_ref(),
    );
    let effective_policy = effective_file_system_sandbox_policy(
        &turn.file_system_sandbox_policy,
        granted_permissions.as_ref(),
    );
    let base_allowed = match kind {
        AccessKind::Read => turn
            .file_system_sandbox_policy
            .can_read_path_with_cwd(path, turn.cwd.as_path()),
        AccessKind::Write => turn
            .file_system_sandbox_policy
            .can_write_path_with_cwd(path, turn.cwd.as_path()),
    };
    let effective_allowed = match kind {
        AccessKind::Read => effective_policy.can_read_path_with_cwd(path, turn.cwd.as_path()),
        AccessKind::Write => effective_policy.can_write_path_with_cwd(path, turn.cwd.as_path()),
    };
    EffectivePathAccess {
        base_allowed,
        effective_allowed,
    }
}

fn additional_permissions_for_access(
    path: &Path,
    kind: AccessKind,
    access: &EffectivePathAccess,
) -> Result<Option<PermissionProfile>, String> {
    if access.base_allowed || !access.effective_allowed {
        return Ok(None);
    }
    let absolute_path = AbsolutePathBuf::from_absolute_path(path)
        .map_err(|err| format!("invalid absolute path `{}`: {err}", path.display()))?;
    let file_system = match kind {
        AccessKind::Read => FileSystemPermissions {
            read: Some(vec![absolute_path]),
            write: None,
        },
        AccessKind::Write => FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![absolute_path]),
        },
    };
    normalize_additional_permissions(PermissionProfile {
        file_system: Some(file_system),
        ..Default::default()
    })
    .map(Some)
}

fn parse_file_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("openai-file://v1/")
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .filter(|file_id| !file_id.is_empty())
        .map(str::to_string)
}

fn upload_auth_required(path: &Path) -> UploadFileToolResult {
    UploadFileToolResult {
        ok: false,
        file_id: None,
        uri: None,
        file_name: None,
        file_size_bytes: None,
        mime_type: None,
        error_code: Some("chatgpt_auth_required".to_string()),
        message: Some("chatgpt authentication is required to upload files".to_string()),
        retryable: Some(false),
        http_status_code: None,
        path: Some(path.display().to_string()),
    }
}

fn download_auth_required(file_id: &str, path: &Path) -> DownloadFileToolResult {
    DownloadFileToolResult {
        ok: false,
        file_id: Some(file_id.to_string()),
        uri: Some(openai_file_uri(file_id)),
        file_name: None,
        mime_type: None,
        destination_path: Some(path.display().to_string()),
        bytes_written: None,
        error_code: Some("chatgpt_auth_required".to_string()),
        message: Some("chatgpt authentication is required to download files".to_string()),
        retryable: Some(false),
        http_status_code: None,
    }
}

fn transfer_network_denied(kind: TransferKind, request: &InternalFileTransferRequest) -> Value {
    match kind {
        TransferKind::Upload => serialize_output_value(UploadFileToolResult {
            ok: false,
            file_id: None,
            uri: None,
            file_name: None,
            file_size_bytes: None,
            mime_type: None,
            error_code: Some("network_denied".to_string()),
            message: Some(
                "network access for file transfer was denied by sandbox policy".to_string(),
            ),
            retryable: Some(false),
            http_status_code: None,
            path: request_path(request),
        }),
        TransferKind::Download => serialize_output_value(DownloadFileToolResult {
            ok: false,
            file_id: request_file_id(request),
            uri: request_file_id(request)
                .as_ref()
                .map(|file_id| openai_file_uri(file_id)),
            file_name: None,
            mime_type: None,
            destination_path: request_path(request),
            bytes_written: None,
            error_code: Some("network_denied".to_string()),
            message: Some(
                "network access for file transfer was denied by sandbox policy".to_string(),
            ),
            retryable: Some(false),
            http_status_code: None,
        }),
    }
}

fn transfer_internal_error(
    kind: TransferKind,
    request: &InternalFileTransferRequest,
    message: String,
) -> Value {
    match kind {
        TransferKind::Upload => serialize_output_value(UploadFileToolResult {
            ok: false,
            file_id: request_file_id(request),
            uri: request_file_id(request)
                .as_ref()
                .map(|file_id| openai_file_uri(file_id)),
            file_name: None,
            file_size_bytes: None,
            mime_type: None,
            error_code: Some("internal_helper_failed".to_string()),
            message: Some(message),
            retryable: Some(false),
            http_status_code: None,
            path: request_path(request),
        }),
        TransferKind::Download => serialize_output_value(DownloadFileToolResult {
            ok: false,
            file_id: request_file_id(request),
            uri: request_file_id(request)
                .as_ref()
                .map(|file_id| openai_file_uri(file_id)),
            file_name: None,
            mime_type: None,
            destination_path: request_path(request),
            bytes_written: None,
            error_code: Some("internal_helper_failed".to_string()),
            message: Some(message),
            retryable: Some(false),
            http_status_code: None,
        }),
    }
}

fn request_path(request: &InternalFileTransferRequest) -> Option<String> {
    match &request.request {
        FileTransferRequest::Upload { path } => Some(path.display().to_string()),
        FileTransferRequest::Download { path, .. } => Some(path.display().to_string()),
    }
}

fn request_file_id(request: &InternalFileTransferRequest) -> Option<String> {
    match &request.request {
        FileTransferRequest::Upload { .. } => None,
        FileTransferRequest::Download { file_id, .. } => Some(file_id.clone()),
    }
}

fn json_output<T: Serialize>(
    value: &T,
    success: bool,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let content = serde_json::to_string(value).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to encode file transfer output: {err}"))
    })?;
    Ok(FunctionToolOutput::from_text(content, Some(success)))
}

fn json_output_value(value: Value, success: bool) -> Result<FunctionToolOutput, FunctionCallError> {
    let content = serde_json::to_string(&value).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to encode file transfer output: {err}"))
    })?;
    Ok(FunctionToolOutput::from_text(content, Some(success)))
}

fn serialize_output_value<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|err| {
        Value::String(format!("failed to serialize file transfer output: {err}"))
    })
}

fn openai_file_uri(file_id: &str) -> String {
    format!("openai-file://v1/{file_id}")
}

#[derive(Clone, Copy, Debug)]
enum AccessKind {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug)]
enum TransferKind {
    Upload,
    Download,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::ExecToolCallOutput;
    use crate::exec::StreamOutput;
    use pretty_assertions::assert_eq;

    fn download_request() -> InternalFileTransferRequest {
        InternalFileTransferRequest {
            request: FileTransferRequest::Download {
                file_id: "file-123".to_string(),
                path: PathBuf::from("/tmp/output.txt"),
                path_is_directory: false,
            },
            cwd: PathBuf::from("/tmp"),
            env: HashMap::new(),
            network: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            codex_exe: None,
        }
    }

    #[test]
    fn parse_helper_output_preserves_request_context_on_nonzero_exit() {
        let output = ExecToolCallOutput {
            exit_code: 23,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new("boom".to_string()),
            aggregated_output: StreamOutput::new("boom".to_string()),
            duration: std::time::Duration::ZERO,
            timed_out: false,
        };

        let result =
            parse_helper_output(output, TransferKind::Download, &download_request()).unwrap();
        let payload: DownloadFileToolResult =
            serde_json::from_str(&result.into_text()).expect("valid json");
        assert_eq!(payload.ok, false);
        assert_eq!(payload.file_id, Some("file-123".to_string()));
        assert_eq!(
            payload.destination_path,
            Some("/tmp/output.txt".to_string())
        );
        assert_eq!(
            payload.error_code,
            Some("internal_helper_failed".to_string())
        );
        assert!(
            payload
                .message
                .as_deref()
                .is_some_and(|message| message.contains("status 23"))
        );
    }
}

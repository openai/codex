use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::io_spec::IO_NAMESPACE;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IoToolKind {
    ReadFile,
    WriteFile,
    EditFile,
    CreateDirectory,
    ListDirectory,
    GetFileInfo,
    ListAllowedDirectories,
}

impl IoToolKind {
    pub(crate) const ALL: [Self; 7] = [
        Self::ReadFile,
        Self::WriteFile,
        Self::EditFile,
        Self::CreateDirectory,
        Self::ListDirectory,
        Self::GetFileInfo,
        Self::ListAllowedDirectories,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::EditFile => "edit_file",
            Self::CreateDirectory => "create_directory",
            Self::ListDirectory => "list_directory",
            Self::GetFileInfo => "get_file_info",
            Self::ListAllowedDirectories => "list_allowed_directories",
        }
    }

    fn is_mutating(self) -> bool {
        matches!(
            self,
            Self::WriteFile | Self::EditFile | Self::CreateDirectory
        )
    }
}

pub struct IoToolHandler {
    kind: IoToolKind,
}

impl IoToolHandler {
    pub(crate) fn new(kind: IoToolKind) -> Self {
        Self { kind }
    }
}

impl ToolHandler for IoToolHandler {
    type Output = IoToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(IO_NAMESPACE, self.kind.name())
    }

    fn spec(&self) -> Option<ToolSpec> {
        None
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        !self.kind.is_mutating()
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn is_mutating(
        &self,
        _invocation: &ToolInvocation,
    ) -> impl std::future::Future<Output = bool> + Send {
        let is_mutating = self.kind.is_mutating();
        async move { is_mutating }
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "io.{} handler received unsupported payload",
                self.kind.name()
            )));
        };

        match self.kind {
            IoToolKind::ReadFile => read_file(invocation.turn.as_ref(), &arguments).await,
            IoToolKind::WriteFile => write_file(invocation.turn.as_ref(), &arguments).await,
            IoToolKind::EditFile => edit_file(invocation.turn.as_ref(), &arguments).await,
            IoToolKind::CreateDirectory => {
                create_directory(invocation.turn.as_ref(), &arguments).await
            }
            IoToolKind::ListDirectory => list_directory(invocation.turn.as_ref(), &arguments).await,
            IoToolKind::GetFileInfo => get_file_info(invocation.turn.as_ref(), &arguments).await,
            IoToolKind::ListAllowedDirectories => {
                list_allowed_directories(invocation.turn.as_ref(), &arguments).await
            }
        }
    }
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
    #[serde(default)]
    environment_id: Option<String>,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
    #[serde(default)]
    environment_id: Option<String>,
}

#[derive(Deserialize)]
struct CreateDirectoryArgs {
    path: String,
    #[serde(default)]
    recursive: Option<bool>,
    #[serde(default)]
    environment_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditFileArgs {
    path: String,
    edits: Vec<TextEdit>,
    #[serde(default)]
    dry_run: Option<bool>,
    #[serde(default)]
    environment_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextEdit {
    old_text: String,
    new_text: String,
}

#[derive(Deserialize)]
struct ListAllowedDirectoriesArgs {
    #[serde(default)]
    environment_id: Option<String>,
}

struct ResolvedIoPath {
    environment_id: String,
    path: AbsolutePathBuf,
    fs: Arc<dyn ExecutorFileSystem>,
    sandbox: FileSystemSandboxContext,
}

pub struct IoToolOutput {
    display: String,
    result: Value,
}

impl IoToolOutput {
    fn text(display: String, result: Value) -> Self {
        Self { display, result }
    }
}

impl ToolOutput for IoToolOutput {
    fn log_preview(&self) -> String {
        self.display.clone()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(self.display.clone()),
                success: Some(true),
            },
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        self.result.clone()
    }
}

async fn read_file(turn: &TurnContext, arguments: &str) -> Result<IoToolOutput, FunctionCallError> {
    let args: PathArgs = parse_arguments(arguments)?;
    let resolved = resolve_io_path(turn, &args.path, args.environment_id.as_deref())?;
    let content = resolved
        .fs
        .read_file_text(&resolved.path, Some(&resolved.sandbox))
        .await
        .map_err(|error| io_error("read_file", &resolved.path, error))?;

    Ok(IoToolOutput::text(content.clone(), Value::String(content)))
}

async fn write_file(
    turn: &TurnContext,
    arguments: &str,
) -> Result<IoToolOutput, FunctionCallError> {
    let args: WriteFileArgs = parse_arguments(arguments)?;
    let resolved = resolve_io_path(turn, &args.path, args.environment_id.as_deref())?;
    let bytes = args.content.into_bytes();
    let bytes_written = bytes.len();
    resolved
        .fs
        .write_file(&resolved.path, bytes, Some(&resolved.sandbox))
        .await
        .map_err(|error| io_error("write_file", &resolved.path, error))?;

    let environment_id = resolved.environment_id.clone();
    let result = json!({
        "path": path_string(&resolved.path),
        "environment_id": environment_id,
        "bytes_written": bytes_written,
    });
    Ok(IoToolOutput::text(
        format!(
            "Wrote {bytes_written} bytes to `{}` in environment `{}`.",
            resolved.path.display(),
            resolved.environment_id
        ),
        result,
    ))
}

async fn edit_file(turn: &TurnContext, arguments: &str) -> Result<IoToolOutput, FunctionCallError> {
    let args: EditFileArgs = parse_arguments(arguments)?;
    if args.edits.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "io.edit_file requires at least one edit".to_string(),
        ));
    }

    let resolved = resolve_io_path(turn, &args.path, args.environment_id.as_deref())?;
    let original = resolved
        .fs
        .read_file_text(&resolved.path, Some(&resolved.sandbox))
        .await
        .map_err(|error| io_error("edit_file", &resolved.path, error))?;
    let mut updated = original.clone();
    for (index, edit) in args.edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Err(FunctionCallError::RespondToModel(format!(
                "io.edit_file edit {} has empty oldText",
                index + 1
            )));
        }
        let matches = updated.matches(&edit.old_text).count();
        match matches {
            0 => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "io.edit_file edit {} did not match `{}`",
                    index + 1,
                    resolved.path.display()
                )));
            }
            1 => {
                updated = updated.replacen(&edit.old_text, &edit.new_text, 1);
            }
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "io.edit_file edit {} matched `{}` {matches} times; oldText must match exactly once",
                    index + 1,
                    resolved.path.display()
                )));
            }
        }
    }

    let dry_run = args.dry_run.unwrap_or(false);
    if !dry_run {
        resolved
            .fs
            .write_file(
                &resolved.path,
                updated.clone().into_bytes(),
                Some(&resolved.sandbox),
            )
            .await
            .map_err(|error| io_error("edit_file", &resolved.path, error))?;
    }

    let environment_id = resolved.environment_id.clone();
    let result = json!({
        "path": path_string(&resolved.path),
        "environment_id": environment_id,
        "edits_applied": args.edits.len(),
        "dry_run": dry_run,
    });
    let verb = if dry_run { "Validated" } else { "Applied" };
    Ok(IoToolOutput::text(
        format!(
            "{verb} {} edit(s) for `{}` in environment `{}`.",
            args.edits.len(),
            resolved.path.display(),
            resolved.environment_id
        ),
        result,
    ))
}

async fn create_directory(
    turn: &TurnContext,
    arguments: &str,
) -> Result<IoToolOutput, FunctionCallError> {
    let args: CreateDirectoryArgs = parse_arguments(arguments)?;
    let resolved = resolve_io_path(turn, &args.path, args.environment_id.as_deref())?;
    let recursive = args.recursive.unwrap_or(true);
    resolved
        .fs
        .create_directory(
            &resolved.path,
            CreateDirectoryOptions { recursive },
            Some(&resolved.sandbox),
        )
        .await
        .map_err(|error| io_error("create_directory", &resolved.path, error))?;

    let environment_id = resolved.environment_id.clone();
    let result = json!({
        "path": path_string(&resolved.path),
        "environment_id": environment_id,
        "recursive": recursive,
    });
    Ok(IoToolOutput::text(
        format!(
            "Created directory `{}` in environment `{}`.",
            resolved.path.display(),
            resolved.environment_id
        ),
        result,
    ))
}

async fn list_directory(
    turn: &TurnContext,
    arguments: &str,
) -> Result<IoToolOutput, FunctionCallError> {
    let args: PathArgs = parse_arguments(arguments)?;
    let resolved = resolve_io_path(turn, &args.path, args.environment_id.as_deref())?;
    let entries = resolved
        .fs
        .read_directory(&resolved.path, Some(&resolved.sandbox))
        .await
        .map_err(|error| io_error("list_directory", &resolved.path, error))?;
    let result = Value::Array(entries.iter().map(entry_to_json).collect());
    Ok(IoToolOutput::text(pretty_json(&result), result))
}

async fn get_file_info(
    turn: &TurnContext,
    arguments: &str,
) -> Result<IoToolOutput, FunctionCallError> {
    let args: PathArgs = parse_arguments(arguments)?;
    let resolved = resolve_io_path(turn, &args.path, args.environment_id.as_deref())?;
    let metadata = resolved
        .fs
        .get_metadata(&resolved.path, Some(&resolved.sandbox))
        .await
        .map_err(|error| io_error("get_file_info", &resolved.path, error))?;
    let result = metadata_to_json(&resolved, &metadata);
    Ok(IoToolOutput::text(pretty_json(&result), result))
}

async fn list_allowed_directories(
    turn: &TurnContext,
    arguments: &str,
) -> Result<IoToolOutput, FunctionCallError> {
    let args: ListAllowedDirectoriesArgs = parse_arguments(arguments)?;
    let Some(turn_environment) = resolve_tool_environment(turn, args.environment_id.as_deref())?
    else {
        return Err(FunctionCallError::RespondToModel(
            "io.list_allowed_directories is unavailable in this session".to_string(),
        ));
    };
    let policy = turn.file_system_sandbox_policy();
    let cwd = turn_environment.cwd.clone();
    let readable_roots = policy
        .get_readable_roots_with_cwd(cwd.as_path())
        .into_iter()
        .map(|path| path_string(&path))
        .collect::<Vec<_>>();
    let writable_roots = policy
        .get_writable_roots_with_cwd(cwd.as_path())
        .into_iter()
        .map(|root| path_string(&root.root))
        .collect::<Vec<_>>();
    let result = json!({
        "environment_id": turn_environment.environment_id.clone(),
        "cwd": path_string(&cwd),
        "readable_roots": readable_roots,
        "writable_roots": writable_roots,
    });
    Ok(IoToolOutput::text(pretty_json(&result), result))
}

fn resolve_io_path(
    turn: &TurnContext,
    path: &str,
    environment_id: Option<&str>,
) -> Result<ResolvedIoPath, FunctionCallError> {
    let (environment_id, path) = parse_path_ref(path, environment_id)?;
    let Some(turn_environment) = resolve_tool_environment(turn, environment_id)? else {
        return Err(FunctionCallError::RespondToModel(
            "io filesystem tools are unavailable in this session".to_string(),
        ));
    };
    let abs_path = resolve_environment_path(turn_environment, path)?;
    let mut sandbox = turn.file_system_sandbox_context(/*additional_permissions*/ None);
    sandbox.cwd = Some(turn_environment.cwd.clone());
    Ok(ResolvedIoPath {
        environment_id: turn_environment.environment_id.clone(),
        path: abs_path,
        fs: turn_environment.environment.get_filesystem(),
        sandbox,
    })
}

fn parse_path_ref<'a>(
    path: &'a str,
    environment_id: Option<&'a str>,
) -> Result<(Option<&'a str>, &'a str), FunctionCallError> {
    let Some(rest) = path.strip_prefix("env://") else {
        return Ok((environment_id, path));
    };
    let (authority, path_after_authority) = match rest.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (rest, ""),
    };
    if authority.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "env filesystem refs must include an environment, for example `env://current/path`"
                .to_string(),
        ));
    }
    if Path::new(path_after_authority).is_absolute() {
        return Err(FunctionCallError::RespondToModel(format!(
            "env filesystem ref `{path}` must not contain an absolute path after the environment id"
        )));
    }
    if authority == "current" {
        return Ok((environment_id, path_after_authority));
    }
    if let Some(argument_environment_id) = environment_id
        && argument_environment_id != authority
    {
        return Err(FunctionCallError::RespondToModel(format!(
            "path `{path}` targets environment `{authority}` but environment_id is `{argument_environment_id}`"
        )));
    }
    Ok((Some(authority), path_after_authority))
}

fn resolve_environment_path(
    turn_environment: &TurnEnvironment,
    path: &str,
) -> Result<AbsolutePathBuf, FunctionCallError> {
    if path.is_empty() {
        return Ok(turn_environment.cwd.clone());
    }
    Ok(turn_environment.cwd.join(path))
}

fn entry_to_json(entry: &ReadDirectoryEntry) -> Value {
    json!({
        "name": entry.file_name,
        "is_directory": entry.is_directory,
        "is_file": entry.is_file,
    })
}

fn metadata_to_json(resolved: &ResolvedIoPath, metadata: &FileMetadata) -> Value {
    json!({
        "path": path_string(&resolved.path),
        "environment_id": resolved.environment_id.clone(),
        "is_directory": metadata.is_directory,
        "is_file": metadata.is_file,
        "is_symlink": metadata.is_symlink,
        "created_at_ms": metadata.created_at_ms,
        "modified_at_ms": metadata.modified_at_ms,
    })
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| format!("failed to serialize io result: {error}"))
}

fn io_error(operation: &str, path: &AbsolutePathBuf, error: std::io::Error) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!(
        "io.{operation} failed for `{}`: {error}",
        path.display()
    ))
}

fn path_string(path: &AbsolutePathBuf) -> String {
    path.to_string_lossy().into_owned()
}

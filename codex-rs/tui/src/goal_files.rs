//! File materialization helpers for TUI goal objectives.
//!
//! Long objectives, pasted text, and local images are written under the app
//! server's Codex home directory. The persisted goal objective keeps references
//! to those files so later continuations can read them by path.

use std::fs;
use std::path::Path;

use crate::app_server_session::AppServerSession;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::LocalImageAttachment;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::de::DeserializeOwned;
use serde_json::json;
use uuid::Uuid;

const GOAL_ATTACHMENT_DIR: &str = "attachments";
const GOAL_FILE_PREFIX: &str = "Codex goal objective file: ";
const GOAL_FILE_INSTRUCTION: &str = "Read that Codex-created file before continuing.";
const GOAL_FILE_NAME: &str = "goal-objective.md";

#[derive(Clone, Debug, Default)]
pub(crate) struct GoalDraft {
    pub(crate) objective: String,
    pub(crate) text_elements: Vec<TextElement>,
    pub(crate) pending_pastes: Vec<(String, String)>,
    pub(crate) local_images: Vec<LocalImageAttachment>,
    pub(crate) remote_image_urls: Vec<String>,
}

/// Host-side file operations needed to materialize goal inputs.
///
/// Implementations must operate on the same filesystem that the app server and
/// agent will use to resolve persisted goal file references.
pub(crate) trait GoalFileStore {
    fn create_directory(
        &mut self,
        path: GoalFilePath,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn write_file(
        &mut self,
        path: GoalFilePath,
        bytes: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn read_file(
        &mut self,
        path: GoalFilePath,
    ) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send;
}

/// Path syntax for goal files has to match the app-server host, not the TUI
/// host, because remote fs APIs deserialize and resolve paths on the server.
pub(crate) type GoalFilePath = String;

impl GoalFileStore for AppServerSession {
    async fn create_directory(&mut self, path: GoalFilePath) -> Result<()> {
        let _: FsCreateDirectoryResponse = request_goal_fs(
            self,
            "fs/createDirectory",
            ClientRequest::FsCreateDirectory {
                request_id: goal_request_id(),
                params: FsCreateDirectoryParams {
                    path: local_goal_file_path(&path)?,
                    recursive: Some(true),
                },
            },
            json!({
                "path": path,
                "recursive": true,
            }),
        )
        .await?;
        Ok(())
    }

    async fn write_file(&mut self, path: GoalFilePath, bytes: Vec<u8>) -> Result<()> {
        let data_base64 = STANDARD.encode(bytes);
        let _: FsWriteFileResponse = request_goal_fs(
            self,
            "fs/writeFile",
            ClientRequest::FsWriteFile {
                request_id: goal_request_id(),
                params: FsWriteFileParams {
                    path: local_goal_file_path(&path)?,
                    data_base64: data_base64.clone(),
                },
            },
            json!({
                "path": path,
                "dataBase64": data_base64,
            }),
        )
        .await?;
        Ok(())
    }

    async fn read_file(&mut self, path: GoalFilePath) -> Result<Vec<u8>> {
        let response: FsReadFileResponse = request_goal_fs(
            self,
            "fs/readFile",
            ClientRequest::FsReadFile {
                request_id: goal_request_id(),
                params: FsReadFileParams {
                    path: local_goal_file_path(&path)?,
                },
            },
            json!({ "path": path }),
        )
        .await?;
        STANDARD
            .decode(response.data_base64)
            .context("fs/readFile returned invalid base64 data")
    }
}

async fn request_goal_fs<T>(
    app_server: &AppServerSession,
    method: &str,
    local_request: ClientRequest,
    remote_params: serde_json::Value,
) -> Result<T>
where
    T: DeserializeOwned,
{
    if app_server.uses_remote_workspace() {
        return remote_json_rpc_typed(app_server, method, remote_params).await;
    }
    app_server
        .request_handle()
        .request_typed(local_request)
        .await
        .with_context(|| format!("{method} failed in TUI"))
}

async fn remote_json_rpc_typed<T>(
    app_server: &AppServerSession,
    method: &str,
    params: serde_json::Value,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let AppServerRequestHandle::Remote(handle) = app_server.request_handle() else {
        bail!("raw JSON-RPC requests are only supported by the remote app-server client");
    };
    let response = handle
        .request_json_rpc(JSONRPCRequest {
            id: goal_request_id(),
            method: method.to_string(),
            params: Some(params),
            trace: None,
        })
        .await
        .with_context(|| format!("{method} failed in TUI"))?;
    let result =
        response.map_err(|source| anyhow::anyhow!("{method} failed in TUI: {}", source.message))?;
    serde_json::from_value(result).with_context(|| format!("{method} returned invalid data"))
}

fn goal_request_id() -> RequestId {
    RequestId::String(format!("goal-files-{}", Uuid::new_v4()))
}

pub(crate) fn codex_home_for_app_server(
    app_server: &AppServerSession,
    local_codex_home: &AbsolutePathBuf,
) -> Option<GoalFilePath> {
    if app_server.uses_remote_workspace() {
        app_server.remote_codex_home().map(str::to_string)
    } else {
        Some(local_codex_home.display().to_string())
    }
}

fn local_goal_file_path(path: &GoalFilePath) -> Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path_checked(path)
        .with_context(|| format!("invalid local goal file path {path}"))
}

fn join_goal_path(path: &str, segment: impl AsRef<str>) -> GoalFilePath {
    let separator = if is_windows_absolute_path(path) {
        '\\'
    } else {
        '/'
    };
    let mut path = path.trim_end_matches(['/', '\\']).to_string();
    if !path.ends_with(separator) {
        path.push(separator);
    }
    path.push_str(segment.as_ref());
    path
}

fn managed_goal_file_path(raw: &str) -> Option<GoalFilePath> {
    if !is_windows_absolute_path(raw) && !raw.starts_with('/') {
        return None;
    }
    let normalized = raw.replace('\\', "/");
    let mut parts = normalized.rsplit('/').filter(|part| !part.is_empty());
    let file_name = parts.next()?;
    let attachment_id = parts.next()?;
    let attachment_dir = parts.next()?;
    if file_name == GOAL_FILE_NAME
        && attachment_dir == GOAL_ATTACHMENT_DIR
        && Uuid::parse_str(attachment_id).is_ok()
    {
        Some(raw.to_string())
    } else {
        None
    }
}

pub(crate) async fn materialize_goal_draft(
    store: &mut impl GoalFileStore,
    codex_home: Option<&GoalFilePath>,
    draft: GoalDraft,
) -> Result<String> {
    let mut objective = draft.objective;
    if objective.trim().is_empty() {
        bail!("Goal objective must not be empty.");
    }
    let text_elements = draft.text_elements;

    let mut output_dir = None;
    let mut replacements = Vec::new();
    for (idx, (placeholder, text)) in draft.pending_pastes.iter().enumerate() {
        let path = join_goal_path(
            &ensure_output_dir(store, codex_home, &mut output_dir).await?,
            format!("pasted-text-{}.txt", idx + 1),
        );
        write_file(store, path.clone(), text.as_bytes().to_vec()).await?;

        if !placeholder.is_empty() {
            replacements.push((placeholder.clone(), format!("pasted text file: {path}")));
        }
    }

    let mut image_lines = Vec::new();
    for (idx, image) in draft.local_images.iter().enumerate() {
        let extension = image_extension(&image.path);
        let path = join_goal_path(
            &ensure_output_dir(store, codex_home, &mut output_dir).await?,
            format!("image-{}.{}", idx + 1, extension),
        );
        let bytes = fs::read(&image.path)
            .with_context(|| format!("Could not read goal image {}", image.path.display()))?;
        write_file(store, path.clone(), bytes).await?;
        if image.placeholder.is_empty() {
            image_lines.push(format!("- [Image #{}]: {path}", idx + 1));
        } else {
            replacements.push((image.placeholder.clone(), format!("image file: {path}")));
        }
    }
    let (expanded_objective, _) =
        ChatComposer::expand_pending_pastes(&objective, text_elements, &replacements);
    objective = expanded_objective.trim().to_string();
    append_section(&mut objective, "Referenced image files:", image_lines);

    append_section(
        &mut objective,
        "Referenced image URLs:",
        draft
            .remote_image_urls
            .into_iter()
            .map(|url| format!("- {url}"))
            .collect(),
    );

    if objective.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        let path = join_goal_path(
            &ensure_output_dir(store, codex_home, &mut output_dir).await?,
            GOAL_FILE_NAME,
        );
        write_file(store, path.clone(), objective.as_bytes().to_vec()).await?;
        objective = objective_file_reference(&path)?;
    }

    Ok(objective)
}

pub(crate) async fn objective_text_for_edit(
    store: &mut impl GoalFileStore,
    objective: &str,
) -> Result<String> {
    let Some(path) = objective_file_path(objective) else {
        return Ok(objective.to_string());
    };
    let bytes = store
        .read_file(path.clone())
        .await
        .with_context(|| format!("Could not read goal objective file {path}"))?;
    String::from_utf8(bytes)
        .with_context(|| format!("Goal objective file {path} is not valid UTF-8"))
}

pub(crate) fn objective_file_path(objective: &str) -> Option<GoalFilePath> {
    let mut lines = objective.lines();
    let path = lines
        .next()?
        .strip_prefix(GOAL_FILE_PREFIX)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .and_then(managed_goal_file_path)?;
    if lines.next() != Some(GOAL_FILE_INSTRUCTION) {
        return None;
    }

    Some(path)
}

pub(crate) fn objective_file_reference(path: &GoalFilePath) -> Result<String> {
    let reference = format!("{GOAL_FILE_PREFIX}{path}\n{GOAL_FILE_INSTRUCTION}");
    let actual_chars = reference.chars().count();
    if actual_chars > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        bail!(
            "Goal objective file reference is too long: {actual_chars} characters. Limit: {MAX_THREAD_GOAL_OBJECTIVE_CHARS} characters."
        );
    }
    Ok(reference)
}

async fn ensure_output_dir(
    store: &mut impl GoalFileStore,
    codex_home: Option<&GoalFilePath>,
    output_dir: &mut Option<GoalFilePath>,
) -> Result<GoalFilePath> {
    if let Some(output_dir) = output_dir {
        return Ok(output_dir.clone());
    }
    let codex_home = codex_home
        .context("App server did not report $CODEX_HOME; cannot materialize goal files")?;
    let path = join_goal_path(
        &join_goal_path(codex_home, GOAL_ATTACHMENT_DIR),
        Uuid::new_v4().to_string(),
    );
    store
        .create_directory(path.clone())
        .await
        .with_context(|| format!("Could not create goal attachment directory {path}"))?;
    *output_dir = Some(path.clone());
    Ok(path)
}

async fn write_file(
    store: &mut impl GoalFileStore,
    path: GoalFilePath,
    bytes: Vec<u8>,
) -> Result<()> {
    store
        .write_file(path.clone(), bytes)
        .await
        .with_context(|| format!("Could not write goal file {path}"))
}

fn is_windows_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/'))
        || path.starts_with("\\\\")
        || path.starts_with("//")
}

fn append_section(objective: &mut String, heading: &str, lines: Vec<String>) {
    if lines.is_empty() {
        return;
    }
    if !objective.ends_with('\n') {
        objective.push_str("\n\n");
    }
    objective.push_str(heading);
    objective.push('\n');
    objective.push_str(&lines.join("\n"));
}

fn image_extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .take(8)
                .collect::<String>()
        })
        .filter(|extension| !extension.is_empty())
        .unwrap_or_else(|| "png".to_string())
}

#[cfg(test)]
#[path = "goal_files_tests.rs"]
mod tests;

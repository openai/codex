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
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use codex_utils_absolute_path::AbsolutePathBuf;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GoalFilePath {
    raw: String,
    separator: char,
}

impl GoalFilePath {
    pub(crate) fn from_local(path: &AbsolutePathBuf) -> Self {
        Self {
            raw: path.display().to_string(),
            separator: std::path::MAIN_SEPARATOR,
        }
    }

    pub(crate) fn from_remote(path: &str, platform_family: Option<&str>) -> Self {
        Self {
            raw: path.to_string(),
            separator: if platform_family == Some("windows") || is_windows_absolute_path(path) {
                '\\'
            } else {
                '/'
            },
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.raw
    }

    pub(crate) fn display(&self) -> &str {
        &self.raw
    }

    fn join(&self, segment: impl AsRef<str>) -> Self {
        let segment = segment.as_ref();
        let trimmed = self.raw.trim_end_matches(['/', '\\']);
        let mut raw = if trimmed.is_empty() {
            self.separator.to_string()
        } else {
            trimmed.to_string()
        };
        if !raw.ends_with(self.separator) {
            raw.push(self.separator);
        }
        raw.push_str(segment);
        Self {
            raw,
            separator: self.separator,
        }
    }

    fn from_managed_reference(raw: &str) -> Option<Self> {
        let separator = if is_windows_absolute_path(raw) {
            '\\'
        } else if raw.starts_with('/') {
            '/'
        } else {
            return None;
        };
        let normalized = raw.replace('\\', "/");
        let parts = normalized
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 3 {
            return None;
        }
        let file_name = parts.last()?;
        let attachment_id = parts.get(parts.len() - 2)?;
        let attachment_dir = parts.get(parts.len() - 3)?;
        if *file_name == GOAL_FILE_NAME
            && *attachment_dir == GOAL_ATTACHMENT_DIR
            && Uuid::parse_str(attachment_id).is_ok()
        {
            Some(Self {
                raw: raw.to_string(),
                separator,
            })
        } else {
            None
        }
    }
}

impl GoalFileStore for AppServerSession {
    async fn create_directory(&mut self, path: GoalFilePath) -> Result<()> {
        if !self.uses_remote_workspace() {
            return self
                .fs_create_directory(local_goal_file_path(&path)?)
                .await
                .map_err(|err| anyhow::anyhow!("{err}"));
        }

        let _: FsCreateDirectoryResponse = self
            .request_json_rpc_typed(
                "fs/createDirectory",
                json!({
                    "path": path.as_str(),
                    "recursive": true,
                }),
            )
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        Ok(())
    }

    async fn write_file(&mut self, path: GoalFilePath, bytes: Vec<u8>) -> Result<()> {
        if !self.uses_remote_workspace() {
            return self
                .fs_write_file(local_goal_file_path(&path)?, bytes)
                .await
                .map_err(|err| anyhow::anyhow!("{err}"));
        }

        let _: FsWriteFileResponse = self
            .request_json_rpc_typed(
                "fs/writeFile",
                json!({
                    "path": path.as_str(),
                    "dataBase64": STANDARD.encode(bytes),
                }),
            )
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        Ok(())
    }

    async fn read_file(&mut self, path: GoalFilePath) -> Result<Vec<u8>> {
        if !self.uses_remote_workspace() {
            return self
                .fs_read_file(local_goal_file_path(&path)?)
                .await
                .map_err(|err| anyhow::anyhow!("{err}"));
        }

        let response: FsReadFileResponse = self
            .request_json_rpc_typed("fs/readFile", json!({ "path": path.as_str() }))
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        STANDARD
            .decode(response.data_base64)
            .context("fs/readFile returned invalid base64 data")
    }
}

pub(crate) fn codex_home_for_app_server(
    app_server: &AppServerSession,
    local_codex_home: &AbsolutePathBuf,
) -> Option<GoalFilePath> {
    if app_server.uses_remote_workspace() {
        app_server
            .remote_codex_home()
            .map(|path| GoalFilePath::from_remote(path, app_server.remote_platform_family()))
    } else {
        Some(GoalFilePath::from_local(local_codex_home))
    }
}

fn local_goal_file_path(path: &GoalFilePath) -> Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path_checked(path.as_str())
        .with_context(|| format!("invalid local goal file path {}", path.display()))
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
    let (validation_objective, _) = ChatComposer::expand_pending_pastes(
        &objective,
        text_elements.clone(),
        &draft.pending_pastes,
    );
    if validation_objective.trim().is_empty() {
        bail!("Goal objective must not be empty.");
    }

    let mut output_dir = None;
    let mut materialized_pastes = Vec::new();
    for (idx, (placeholder, text)) in draft.pending_pastes.iter().enumerate() {
        let path = ensure_output_dir(store, codex_home, &mut output_dir)
            .await?
            .join(format!("pasted-text-{}.txt", idx + 1));
        write_file(store, path.clone(), text.as_bytes().to_vec()).await?;

        if !placeholder.is_empty() {
            materialized_pastes.push((
                placeholder.clone(),
                format!("pasted text file: {}", path.display()),
            ));
        }
    }
    let (expanded_objective, text_elements) =
        ChatComposer::expand_pending_pastes(&objective, text_elements, &materialized_pastes);
    objective = expanded_objective;

    let mut image_lines = Vec::new();
    let mut materialized_images = Vec::new();
    for (idx, image) in draft.local_images.iter().enumerate() {
        let extension = image_extension(&image.path);
        let path = ensure_output_dir(store, codex_home, &mut output_dir)
            .await?
            .join(format!("image-{}.{}", idx + 1, extension));
        let bytes = fs::read(&image.path)
            .with_context(|| format!("Could not read goal image {}", image.path.display()))?;
        write_file(store, path.clone(), bytes).await?;
        if image.placeholder.is_empty() {
            image_lines.push(format!("- [Image #{}]: {}", idx + 1, path.display()));
        } else {
            materialized_images.push((
                image.placeholder.clone(),
                format!("image file: {}", path.display()),
            ));
        }
    }
    let (expanded_objective, _) =
        ChatComposer::expand_pending_pastes(&objective, text_elements, &materialized_images);
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
        let path = ensure_output_dir(store, codex_home, &mut output_dir)
            .await?
            .join(GOAL_FILE_NAME);
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
        .with_context(|| format!("Could not read goal objective file {}", path.display()))?;
    String::from_utf8(bytes)
        .with_context(|| format!("Goal objective file {} is not valid UTF-8", path.display()))
}

pub(crate) fn objective_file_path(objective: &str) -> Option<GoalFilePath> {
    let mut lines = objective.lines();
    let path = lines
        .next()?
        .strip_prefix(GOAL_FILE_PREFIX)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .and_then(GoalFilePath::from_managed_reference)?;
    if lines.next() != Some(GOAL_FILE_INSTRUCTION) {
        return None;
    }

    Some(path)
}

pub(crate) fn objective_file_reference(path: &GoalFilePath) -> Result<String> {
    let reference = format!(
        "{GOAL_FILE_PREFIX}{}\n{GOAL_FILE_INSTRUCTION}",
        path.display()
    );
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
    let path = codex_home
        .join(GOAL_ATTACHMENT_DIR)
        .join(Uuid::new_v4().to_string());
    store
        .create_directory(path.clone())
        .await
        .with_context(|| {
            format!(
                "Could not create goal attachment directory {}",
                path.display()
            )
        })?;
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
        .with_context(|| format!("Could not write goal file {}", path.display()))
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

fn image_extension(path: &Path) -> &str {
    path.extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("png")
}

#[cfg(test)]
#[path = "goal_files_tests.rs"]
mod tests;

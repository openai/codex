//! File materialization helpers for TUI goal objectives.
//!
//! Long objectives are written under the app server's Codex home directory.
//! The persisted goal objective keeps a file reference so later continuations
//! can read the long objective by path.

use crate::app_server_session::AppServerSession;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_client::AppServerPath;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use uuid::Uuid;

const GOAL_ATTACHMENT_DIR: &str = "attachments";
const GOAL_FILE_PREFIX: &str = "Codex goal objective file: ";
const GOAL_FILE_INSTRUCTION: &str = "Read that Codex-created file before continuing.";
const GOAL_FILE_NAME: &str = "goal-objective.md";

#[derive(Clone, Debug, Default)]
pub(crate) struct GoalDraft {
    pub(crate) objective: String,
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

pub(crate) type GoalFilePath = AppServerPath;

impl GoalFileStore for AppServerSession {
    async fn create_directory(&mut self, path: GoalFilePath) -> Result<()> {
        self.fs_create_directory_path(&path, /*recursive*/ true)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))
    }

    async fn write_file(&mut self, path: GoalFilePath, bytes: Vec<u8>) -> Result<()> {
        self.fs_write_file_path(&path, bytes)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))
    }

    async fn read_file(&mut self, path: GoalFilePath) -> Result<Vec<u8>> {
        self.fs_read_file_path(&path)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))
    }
}

pub(crate) async fn materialize_goal_draft(
    store: &mut impl GoalFileStore,
    codex_home: Option<&GoalFilePath>,
    draft: GoalDraft,
) -> Result<String> {
    let mut objective = draft.objective.trim().to_string();
    if objective.is_empty() {
        bail!("Goal objective must not be empty.");
    }

    if objective.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        let output_dir = create_goal_output_dir(store, codex_home).await?;
        let path = output_dir.join(GOAL_FILE_NAME);
        write_goal_file(store, path.clone(), objective.as_bytes().to_vec()).await?;
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

fn managed_goal_file_path(raw: &str) -> Option<GoalFilePath> {
    let path = AppServerPath::from_absolute_str(raw)?;
    let parts = path.components();
    let file_name = parts.last()?;
    let attachment_id = parts.get(parts.len().checked_sub(2)?)?;
    let attachment_dir = parts.get(parts.len().checked_sub(3)?)?;
    if *file_name == GOAL_FILE_NAME
        && *attachment_dir == GOAL_ATTACHMENT_DIR
        && Uuid::parse_str(attachment_id).is_ok()
    {
        Some(path)
    } else {
        None
    }
}

async fn create_goal_output_dir(
    store: &mut impl GoalFileStore,
    codex_home: Option<&GoalFilePath>,
) -> Result<GoalFilePath> {
    let codex_home = codex_home
        .context("App server did not report $CODEX_HOME; cannot materialize goal files")?;
    let path = codex_home
        .join(GOAL_ATTACHMENT_DIR)
        .join(Uuid::new_v4().to_string());
    store
        .create_directory(path.clone())
        .await
        .with_context(|| format!("Could not create goal attachment directory {path}"))?;
    Ok(path)
}

async fn write_goal_file(
    store: &mut impl GoalFileStore,
    path: GoalFilePath,
    bytes: Vec<u8>,
) -> Result<()> {
    store
        .write_file(path.clone(), bytes)
        .await
        .with_context(|| format!("Could not write goal file {path}"))
}

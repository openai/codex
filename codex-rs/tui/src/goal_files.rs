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

pub(crate) type GoalFilePath = AppServerPath;

pub(crate) async fn materialize_goal_objective(
    app_server: &mut AppServerSession,
    codex_home: Option<&GoalFilePath>,
    objective: String,
) -> Result<String> {
    let mut objective = objective.trim().to_string();
    if objective.is_empty() {
        bail!("Goal objective must not be empty.");
    }

    if objective.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        let output_dir = create_goal_output_dir(app_server, codex_home).await?;
        let path = output_dir.join(GOAL_FILE_NAME);
        write_goal_file(app_server, path.clone(), objective.as_bytes().to_vec()).await?;
        objective = objective_file_reference(&path)?;
    }

    Ok(objective)
}

pub(crate) async fn objective_text_for_edit(
    app_server: &mut AppServerSession,
    codex_home: Option<&GoalFilePath>,
    objective: &str,
) -> Result<String> {
    let Some(path) = objective_file_path(objective, codex_home) else {
        return Ok(objective.to_string());
    };
    let bytes = app_server
        .fs_read_file_path(&path)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not read goal objective file {path}"))?;
    String::from_utf8(bytes)
        .with_context(|| format!("Goal objective file {path} is not valid UTF-8"))
}

pub(crate) fn objective_file_path(
    objective: &str,
    codex_home: Option<&GoalFilePath>,
) -> Option<GoalFilePath> {
    let path = parse_objective_file_path(objective)?;
    let codex_home = codex_home?;
    let codex_home_parts = codex_home.components();
    let path_parts = path.components();
    (!codex_home_parts.is_empty()
        && !has_normalization_component(&codex_home_parts)
        && !has_normalization_component(&path_parts)
        && path_parts.starts_with(&codex_home_parts))
    .then_some(path)
}

fn has_normalization_component(parts: &[&str]) -> bool {
    parts.iter().any(|part| matches!(*part, "." | ".."))
}

fn parse_objective_file_path(objective: &str) -> Option<GoalFilePath> {
    let mut lines = objective.lines();
    let path = lines
        .next()?
        .strip_prefix(GOAL_FILE_PREFIX)
        .map(str::trim)
        .filter(|path| !path.is_empty())?;
    if lines.next() != Some(GOAL_FILE_INSTRUCTION) {
        return None;
    }

    let path = AppServerPath::from_absolute_str(path)?;
    let parts = path.components();
    let file_name = parts.last()?;
    let attachment_id = parts.get(parts.len().checked_sub(2)?)?;
    let attachment_dir = parts.get(parts.len().checked_sub(3)?)?;
    (*file_name == GOAL_FILE_NAME
        && *attachment_dir == GOAL_ATTACHMENT_DIR
        && Uuid::parse_str(attachment_id).is_ok())
    .then_some(path)
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

async fn create_goal_output_dir(
    app_server: &mut AppServerSession,
    codex_home: Option<&GoalFilePath>,
) -> Result<GoalFilePath> {
    let codex_home = codex_home
        .context("App server did not report $CODEX_HOME; cannot materialize goal files")?;
    let path = codex_home
        .join(GOAL_ATTACHMENT_DIR)
        .join(Uuid::new_v4().to_string());
    app_server
        .fs_create_directory_all_path(&path)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not create goal attachment directory {path}"))?;
    Ok(path)
}

async fn write_goal_file(
    app_server: &mut AppServerSession,
    path: GoalFilePath,
    bytes: Vec<u8>,
) -> Result<()> {
    app_server
        .fs_write_file_path(&path, bytes)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not write goal file {path}"))
}

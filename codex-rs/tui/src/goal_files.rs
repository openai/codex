//! Materializes oversized TUI goal objectives as app-server-host files.

use crate::app_server_session::AppServerSession;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_client::AppServerPath;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use uuid::Uuid;

const GOAL_ATTACHMENT_DIR: &str = "attachments";
const GOAL_FILE_PREFIX: &str = "Read the Codex goal objective file at ";
const GOAL_FILE_SUFFIX: &str = " before continuing.";
const GOAL_FILE_NAME: &str = "goal-objective.md";

pub(crate) type GoalFilePath = AppServerPath;

pub(crate) async fn materialize_goal_objective(
    app_server: &mut AppServerSession,
    codex_home: Option<&GoalFilePath>,
    objective: String,
) -> Result<(String, Option<GoalFilePath>)> {
    let objective = objective.trim().to_string();
    if objective.is_empty() {
        bail!("Goal objective must not be empty.");
    }

    if objective.chars().count() <= MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        return Ok((objective, None));
    }

    let codex_home = codex_home
        .context("App server did not report $CODEX_HOME; cannot materialize goal files")?;
    let output_dir = codex_home
        .join(GOAL_ATTACHMENT_DIR)
        .join(Uuid::new_v4().to_string());
    app_server
        .fs_create_directory_all_path(&output_dir)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not create goal attachment directory {output_dir}"))?;

    let path = output_dir.join(GOAL_FILE_NAME);
    app_server
        .fs_write_file_path(&path, objective.as_bytes().to_vec())
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not write goal file {path}"))?;
    Ok((objective_file_reference(&path)?, Some(output_dir)))
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
    let path = objective
        .strip_prefix(GOAL_FILE_PREFIX)
        .and_then(|path| path.strip_suffix(GOAL_FILE_SUFFIX))?;
    let path = AppServerPath::from_absolute_str(path)?;
    let parts = path.components();
    let attachment_id = parts.get(parts.len().checked_sub(2)?)?;
    let expected = codex_home?
        .join(GOAL_ATTACHMENT_DIR)
        .join(attachment_id)
        .join(GOAL_FILE_NAME);
    (path == expected && Uuid::parse_str(attachment_id).is_ok()).then_some(path)
}

pub(crate) fn objective_file_reference(path: &GoalFilePath) -> Result<String> {
    let reference = format!("{GOAL_FILE_PREFIX}{path}{GOAL_FILE_SUFFIX}");
    let actual_chars = reference.chars().count();
    if actual_chars > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        bail!(
            "Goal objective file reference is too long: {actual_chars} characters. Limit: {MAX_THREAD_GOAL_OBJECTIVE_CHARS} characters."
        );
    }
    Ok(reference)
}

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

pub(crate) struct MaterializedGoal {
    pub(crate) objective: String,
    pub(crate) output_dir: Option<GoalFilePath>,
}

pub(crate) async fn materialize_goal_objective(
    app_server: &mut AppServerSession,
    codex_home: Option<&GoalFilePath>,
    objective: String,
) -> Result<MaterializedGoal> {
    let mut objective = objective.trim().to_string();
    if objective.is_empty() {
        bail!("Goal objective must not be empty.");
    }

    let mut output_dir = None;
    if objective.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        let codex_home = codex_home
            .context("App server did not report $CODEX_HOME; cannot materialize goal files")?;
        let dir = codex_home
            .join(GOAL_ATTACHMENT_DIR)
            .join(Uuid::new_v4().to_string());
        app_server
            .fs_create_directory_all_path(&dir)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))
            .with_context(|| format!("Could not create goal attachment directory {dir}"))?;

        let path = dir.join(GOAL_FILE_NAME);
        app_server
            .fs_write_file_path(&path, objective.as_bytes().to_vec())
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))
            .with_context(|| format!("Could not write goal file {path}"))?;
        objective = objective_file_reference(&path)?;
        output_dir = Some(dir);
    }

    Ok(MaterializedGoal {
        objective,
        output_dir,
    })
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
    let has_normalization_component =
        |parts: &[&str]| parts.iter().any(|part| matches!(*part, "." | ".."));
    (!codex_home_parts.is_empty()
        && !has_normalization_component(&codex_home_parts)
        && !has_normalization_component(&path_parts)
        && path_parts.len() == codex_home_parts.len() + 3
        && path_parts.starts_with(&codex_home_parts))
    .then_some(path)
}

fn parse_objective_file_path(objective: &str) -> Option<GoalFilePath> {
    let (path, instruction) = objective.split_once('\n')?;
    if instruction != GOAL_FILE_INSTRUCTION {
        return None;
    }
    let path = path
        .strip_prefix(GOAL_FILE_PREFIX)
        .map(str::trim)
        .filter(|path| !path.is_empty())?;

    let path = AppServerPath::from_absolute_str(path)?;
    let parts = path.components();
    let [.., attachment_dir, attachment_id, file_name] = parts.as_slice() else {
        return None;
    };
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

//! Materializes oversized TUI goal objectives and pastes as app-server-host files.

use crate::app_server_session::AppServerSession;
use crate::bottom_pane::ChatComposer;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_client::AppServerPath;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use uuid::Uuid;

const GOAL_ATTACHMENT_DIR: &str = "attachments";
const GOAL_FILE_PREFIX: &str = "Read the Codex goal objective file at ";
const GOAL_FILE_SUFFIX: &str = " before continuing.";
const GOAL_FILE_NAME: &str = "goal-objective.md";

#[derive(Clone, Debug, Default)]
pub(crate) struct GoalDraft {
    pub(crate) objective: String,
    pub(crate) text_elements: Vec<TextElement>,
    pub(crate) pending_pastes: Vec<(String, String)>,
}

pub(crate) type GoalFilePath = AppServerPath;

pub(crate) async fn materialize_goal_draft(
    app_server: &mut AppServerSession,
    codex_home: Option<&GoalFilePath>,
    draft: GoalDraft,
) -> Result<(String, Option<GoalFilePath>)> {
    let mut objective = draft.objective;
    if objective.trim().is_empty() {
        bail!("Goal objective must not be empty.");
    }
    let text_elements = draft.text_elements;
    if !draft.pending_pastes.is_empty() {
        let (expanded_objective, _) = ChatComposer::expand_pending_pastes(
            &objective,
            text_elements.clone(),
            &draft.pending_pastes,
        );
        if expanded_objective.trim().is_empty() {
            bail!("Goal objective must not be empty.");
        }
    }

    let mut active_placeholders = text_elements
        .iter()
        .filter_map(|element| element.placeholder(&objective))
        .filter(|placeholder| !placeholder.is_empty())
        .collect::<Vec<_>>();
    let mut output_dir = None;
    let mut replacements = Vec::new();
    for (placeholder, text) in draft.pending_pastes.iter() {
        let Some(active_idx) = active_placeholders
            .iter()
            .position(|active| *active == placeholder.as_str())
        else {
            continue;
        };
        active_placeholders.swap_remove(active_idx);
        let path = ensure_goal_output_dir(app_server, codex_home, &mut output_dir)
            .await?
            .join(format!("pasted-text-{}.txt", replacements.len() + 1));
        write_goal_file(app_server, path.clone(), text.as_bytes().to_vec()).await?;

        replacements.push((
            placeholder.clone(),
            format!("pasted text file: {path}. Read this file before continuing."),
        ));
    }

    let (expanded_objective, _) =
        ChatComposer::expand_pending_pastes(&objective, text_elements, &replacements);
    objective = expanded_objective.trim().to_string();

    if objective.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        let path = ensure_goal_output_dir(app_server, codex_home, &mut output_dir)
            .await?
            .join(GOAL_FILE_NAME);
        let reference = match objective_file_reference(&path) {
            Ok(reference) => reference,
            Err(err) => {
                if let Some(output_dir) = output_dir.as_ref() {
                    let _ = app_server.fs_remove_path(output_dir).await;
                }
                return Err(err);
            }
        };
        write_goal_file(app_server, path.clone(), objective.as_bytes().to_vec()).await?;
        objective = reference;
    }
    Ok((objective, output_dir))
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

async fn ensure_goal_output_dir(
    app_server: &mut AppServerSession,
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
    app_server
        .fs_create_directory_all_path(&path)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not create goal attachment directory {path}"))?;
    *output_dir = Some(path.clone());
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

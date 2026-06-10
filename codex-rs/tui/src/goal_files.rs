//! File materialization helpers for TUI goal objectives.
//!
//! Long objectives, pasted text, and local images are written under the Codex
//! home directory. The persisted goal objective keeps references to those files
//! so later continuations can read them by path.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::LocalImageAttachment;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use uuid::Uuid;

const GOAL_ATTACHMENT_DIR: &str = "attachments";
const GOAL_FILE_PREFIX: &str = "Codex goal objective file: ";
const GOAL_FILE_INSTRUCTION: &str = "Read that file before continuing.";
const GOAL_FILE_NAME: &str = "goal-objective.md";

#[derive(Debug, Default)]
pub(crate) struct GoalDraft {
    pub(crate) objective: String,
    pub(crate) text_elements: Vec<TextElement>,
    pub(crate) pending_pastes: Vec<(String, String)>,
    pub(crate) local_images: Vec<LocalImageAttachment>,
    pub(crate) remote_image_urls: Vec<String>,
}

pub(crate) fn materialize_goal_draft(codex_home: &Path, draft: GoalDraft) -> Result<String> {
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
        let path = ensure_output_dir(codex_home, &mut output_dir)?
            .join(format!("pasted-text-{}.txt", idx + 1));
        write_file(&path, text)?;

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
        let path = ensure_output_dir(codex_home, &mut output_dir)?.join(format!(
            "image-{}.{}",
            idx + 1,
            extension
        ));
        fs::copy(&image.path, &path).with_context(|| {
            format!(
                "Could not copy goal image from {} to {}",
                image.path.display(),
                path.display()
            )
        })?;
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
        let path = ensure_output_dir(codex_home, &mut output_dir)?.join(GOAL_FILE_NAME);
        write_file(&path, &objective)?;
        objective = objective_file_reference(&path)?;
    }

    Ok(objective)
}

pub(crate) fn objective_text_for_edit(objective: &str) -> Result<String> {
    let Some(path) = objective_file_path(objective) else {
        return Ok(objective.to_string());
    };
    fs::read_to_string(&path)
        .with_context(|| format!("Could not read goal objective file {}", path.display()))
}

pub(crate) fn objective_file_path(objective: &str) -> Option<PathBuf> {
    let mut lines = objective.lines();
    let path = lines
        .next()?
        .strip_prefix(GOAL_FILE_PREFIX)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)?;
    if lines.next() != Some(GOAL_FILE_INSTRUCTION) {
        return None;
    }

    let parent = path.parent()?;
    let attachment_id = parent.file_name()?.to_str()?;
    let attachment_dir = parent.parent()?.file_name()?.to_str()?;
    if path.is_file()
        && path.file_name()?.to_str()? == GOAL_FILE_NAME
        && attachment_dir == GOAL_ATTACHMENT_DIR
        && Uuid::parse_str(attachment_id).is_ok()
    {
        Some(path)
    } else {
        None
    }
}

fn objective_file_reference(path: &Path) -> Result<String> {
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

fn ensure_output_dir(codex_home: &Path, output_dir: &mut Option<PathBuf>) -> Result<PathBuf> {
    if let Some(output_dir) = output_dir {
        return Ok(output_dir.clone());
    }
    let path = codex_home
        .join(GOAL_ATTACHMENT_DIR)
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&path).with_context(|| {
        format!(
            "Could not create goal attachment directory {}",
            path.display()
        )
    })?;
    *output_dir = Some(path.clone());
    Ok(path)
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content)
        .with_context(|| format!("Could not write goal file {}", path.display()))
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

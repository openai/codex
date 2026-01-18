use crate::text::offset_for_position;
use crate::uri::uri_to_file_path;
use codex_apply_patch::ApplyPatchAction;
use lsp_types::AnnotatedTextEdit;
use lsp_types::DocumentChangeOperation;
use lsp_types::DocumentChanges;
use lsp_types::OneOf;
use lsp_types::PositionEncodingKind;
use lsp_types::ResourceOp;
use lsp_types::TextEdit;
use lsp_types::WorkspaceEdit;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceEditError {
    #[error("workspace edit missing changes")]
    MissingChanges,
    #[error("invalid uri in workspace edit")]
    InvalidUri,
    #[error("failed to read file {0}")]
    ReadFailed(PathBuf),
    #[error("workspace edit applies overlapping edits")]
    OverlappingEdits,
    #[error("apply_patch conversion failed: {0}")]
    PatchFailed(String),
    #[error("unsupported workspace edit operation: {0}")]
    Unsupported(String),
}

#[derive(Debug)]
pub struct WorkspaceEditResult {
    pub action: ApplyPatchAction,
    pub patch: String,
}

#[derive(Debug)]
struct FileState {
    content: String,
    exists: bool,
}

pub async fn workspace_edit_to_apply_patch(
    edit: WorkspaceEdit,
    root: &Path,
    encoding: &PositionEncodingKind,
) -> Result<WorkspaceEditResult, WorkspaceEditError> {
    let mut patch_lines = Vec::new();
    patch_lines.push("*** Begin Patch".to_string());

    if let Some(document_changes) = edit.document_changes {
        match document_changes {
            DocumentChanges::Edits(edits) => {
                let mut edits_by_file: BTreeMap<PathBuf, Vec<TextEdit>> = BTreeMap::new();
                for edit in edits {
                    let path = uri_to_file_path(&edit.text_document.uri)
                        .ok_or(WorkspaceEditError::InvalidUri)?;
                    edits_by_file
                        .entry(path)
                        .or_default()
                        .extend(normalize_text_edits(edit.edits));
                }
                append_file_edits(&mut patch_lines, edits_by_file, root, encoding).await?;
            }
            DocumentChanges::Operations(ops) => {
                apply_operations_in_order(&mut patch_lines, ops, root, encoding).await?;
            }
        }
    } else if let Some(changes) = edit.changes {
        let mut edits_by_file: BTreeMap<PathBuf, Vec<TextEdit>> = BTreeMap::new();
        for (uri, edits) in changes {
            let path = uri_to_file_path(&uri).ok_or(WorkspaceEditError::InvalidUri)?;
            edits_by_file.entry(path).or_default().extend(edits);
        }
        append_file_edits(&mut patch_lines, edits_by_file, root, encoding).await?;
    } else {
        return Err(WorkspaceEditError::MissingChanges);
    }

    patch_lines.push("*** End Patch".to_string());

    let patch = patch_lines.join("\n");
    let command = vec!["apply_patch".to_string(), patch.clone()];
    match codex_apply_patch::maybe_parse_apply_patch_verified(&command, root) {
        codex_apply_patch::MaybeApplyPatchVerified::Body(action) => {
            Ok(WorkspaceEditResult { action, patch })
        }
        codex_apply_patch::MaybeApplyPatchVerified::CorrectnessError(err) => {
            Err(WorkspaceEditError::PatchFailed(format!("{err:?}")))
        }
        codex_apply_patch::MaybeApplyPatchVerified::ShellParseError(err) => {
            Err(WorkspaceEditError::PatchFailed(format!("{err:?}")))
        }
        codex_apply_patch::MaybeApplyPatchVerified::NotApplyPatch => {
            Err(WorkspaceEditError::PatchFailed("invalid patch".to_string()))
        }
    }
}

async fn append_file_edits(
    patch_lines: &mut Vec<String>,
    edits_by_file: BTreeMap<PathBuf, Vec<TextEdit>>,
    root: &Path,
    encoding: &PositionEncodingKind,
) -> Result<(), WorkspaceEditError> {
    for (path, edits) in edits_by_file {
        let (exists, original) = match tokio::fs::read_to_string(&path).await {
            Ok(original) => (true, original),
            Err(err) if err.kind() == ErrorKind::NotFound => (false, String::new()),
            Err(_) => return Err(WorkspaceEditError::ReadFailed(path.clone())),
        };
        let updated = apply_text_edits(&original, &edits, encoding)?;
        if original == updated {
            continue;
        }
        let path_str = path_to_patch_path(root, &path);
        if exists {
            patch_lines.push(format!("*** Update File: {path_str}"));
            patch_lines.push("@@".to_string());
            for line in split_lines(&original) {
                patch_lines.push(format!("-{line}"));
            }
            for line in split_lines(&updated) {
                patch_lines.push(format!("+{line}"));
            }
        } else {
            if updated.is_empty() {
                return Err(WorkspaceEditError::Unsupported(
                    "create empty file".to_string(),
                ));
            }
            patch_lines.push(format!("*** Add File: {path_str}"));
            for line in split_lines(&updated) {
                patch_lines.push(format!("+{line}"));
            }
        }
    }
    Ok(())
}

async fn apply_operations_in_order(
    patch_lines: &mut Vec<String>,
    ops: Vec<DocumentChangeOperation>,
    root: &Path,
    encoding: &PositionEncodingKind,
) -> Result<(), WorkspaceEditError> {
    let mut states: HashMap<PathBuf, FileState> = HashMap::new();

    for op in ops {
        match op {
            DocumentChangeOperation::Edit(edit) => {
                let path = uri_to_file_path(&edit.text_document.uri)
                    .ok_or(WorkspaceEditError::InvalidUri)?;
                load_file_state(&mut states, &path).await?;
                let state = states
                    .get_mut(&path)
                    .ok_or(WorkspaceEditError::ReadFailed(path.clone()))?;
                let original = state.content.clone();
                let updated =
                    apply_text_edits(&original, &normalize_text_edits(edit.edits), encoding)?;
                if original == updated {
                    continue;
                }
                let path_str = path_to_patch_path(root, &path);
                if state.exists {
                    patch_lines.push(format!("*** Update File: {path_str}"));
                    patch_lines.push("@@".to_string());
                    for line in split_lines(&original) {
                        patch_lines.push(format!("-{line}"));
                    }
                    for line in split_lines(&updated) {
                        patch_lines.push(format!("+{line}"));
                    }
                } else {
                    if updated.is_empty() {
                        return Err(WorkspaceEditError::Unsupported(
                            "create empty file".to_string(),
                        ));
                    }
                    patch_lines.push(format!("*** Add File: {path_str}"));
                    for line in split_lines(&updated) {
                        patch_lines.push(format!("+{line}"));
                    }
                }
                state.content = updated;
                state.exists = true;
            }
            DocumentChangeOperation::Op(ResourceOp::Delete(delete)) => {
                let path = uri_to_file_path(&delete.uri).ok_or(WorkspaceEditError::InvalidUri)?;
                load_file_state(&mut states, &path).await?;
                if let Some(state) = states.get_mut(&path) {
                    state.content.clear();
                    state.exists = false;
                }
                let path_str = path_to_patch_path(root, &path);
                patch_lines.push(format!("*** Delete File: {path_str}"));
            }
            DocumentChangeOperation::Op(ResourceOp::Rename(rename)) => {
                let old_path =
                    uri_to_file_path(&rename.old_uri).ok_or(WorkspaceEditError::InvalidUri)?;
                let new_path =
                    uri_to_file_path(&rename.new_uri).ok_or(WorkspaceEditError::InvalidUri)?;
                load_file_state(&mut states, &old_path).await?;
                let original = {
                    let state = states
                        .get_mut(&old_path)
                        .ok_or(WorkspaceEditError::ReadFailed(old_path.clone()))?;
                    if !state.exists {
                        return Err(WorkspaceEditError::ReadFailed(old_path.clone()));
                    }
                    let original = state.content.clone();
                    state.content.clear();
                    state.exists = false;
                    original
                };
                let path_str = path_to_patch_path(root, &old_path);
                let move_str = path_to_patch_path(root, &new_path);
                patch_lines.push(format!("*** Update File: {path_str}"));
                patch_lines.push(format!("*** Move to: {move_str}"));
                patch_lines.push("@@".to_string());
                for line in split_lines(&original) {
                    patch_lines.push(format!(" {line}"));
                }
                states.insert(
                    new_path,
                    FileState {
                        content: original,
                        exists: true,
                    },
                );
            }
            DocumentChangeOperation::Op(ResourceOp::Create(_)) => {
                return Err(WorkspaceEditError::Unsupported(
                    "create file operation".to_string(),
                ));
            }
        }
    }

    Ok(())
}

async fn load_file_state(
    states: &mut HashMap<PathBuf, FileState>,
    path: &Path,
) -> Result<(), WorkspaceEditError> {
    if states.contains_key(path) {
        return Ok(());
    }
    let (exists, content) = match tokio::fs::read_to_string(path).await {
        Ok(content) => (true, content),
        Err(err) if err.kind() == ErrorKind::NotFound => (false, String::new()),
        Err(_) => return Err(WorkspaceEditError::ReadFailed(path.to_path_buf())),
    };
    states.insert(path.to_path_buf(), FileState { content, exists });
    Ok(())
}

fn path_to_patch_path(root: &Path, path: &Path) -> String {
    pathdiff::diff_paths(path, root)
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn split_lines(contents: &str) -> Vec<String> {
    contents.split('\n').map(ToString::to_string).collect()
}

fn apply_text_edits(
    text: &str,
    edits: &[TextEdit],
    encoding: &PositionEncodingKind,
) -> Result<String, WorkspaceEditError> {
    let mut ranges = Vec::new();
    for edit in edits {
        let start = offset_for_position(text, edit.range.start, encoding)
            .ok_or_else(|| WorkspaceEditError::Unsupported("invalid edit range".to_string()))?;
        let end = offset_for_position(text, edit.range.end, encoding)
            .ok_or_else(|| WorkspaceEditError::Unsupported("invalid edit range".to_string()))?;
        ranges.push((start, end, edit.new_text.clone()));
    }

    ranges.sort_by(|a, b| b.0.cmp(&a.0));
    let mut last_start = None;
    for (start, end, _) in &ranges {
        if let Some(prev_start) = last_start
            && end > &prev_start
        {
            return Err(WorkspaceEditError::OverlappingEdits);
        }
        last_start = Some(*start);
    }

    let mut updated = text.to_string();
    for (start, end, replacement) in ranges {
        if start > end || end > updated.len() {
            return Err(WorkspaceEditError::Unsupported(
                "invalid edit bounds".to_string(),
            ));
        }
        updated.replace_range(start..end, &replacement);
    }
    Ok(updated)
}

fn normalize_text_edits(edits: Vec<OneOf<TextEdit, AnnotatedTextEdit>>) -> Vec<TextEdit> {
    edits
        .into_iter()
        .map(|edit| match edit {
            OneOf::Left(edit) => edit,
            OneOf::Right(edit) => TextEdit {
                range: edit.text_edit.range,
                new_text: edit.text_edit.new_text,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Position;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn apply_text_edits_sorts_descending() {
        let text = "hello world";
        let edits = vec![
            TextEdit {
                range: lsp_types::Range {
                    start: Position {
                        line: 0,
                        character: 6,
                    },
                    end: Position {
                        line: 0,
                        character: 11,
                    },
                },
                new_text: "codex".to_string(),
            },
            TextEdit {
                range: lsp_types::Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 5,
                    },
                },
                new_text: "hello".to_string(),
            },
        ];
        let updated = apply_text_edits(text, &edits, &PositionEncodingKind::UTF16).unwrap();
        assert_eq!(updated, "hello codex");
    }
}

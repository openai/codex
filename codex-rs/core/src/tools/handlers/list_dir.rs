use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::FileType;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_string::take_bytes_at_char_boundary;
use serde::Deserialize;
use tokio::fs;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ListDirHandler;

const DENY_READ_POLICY_MESSAGE: &str =
    "access denied: reading this path is blocked by filesystem deny_read policy";
const MAX_ENTRY_LENGTH: usize = 500;
const INDENTATION_SPACES: usize = 2;

fn default_offset() -> usize {
    1
}

fn default_limit() -> usize {
    25
}

fn default_depth() -> usize {
    2
}

#[derive(Deserialize)]
struct ListDirArgs {
    dir_path: String,
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_depth")]
    depth: usize,
}

impl ToolHandler for ListDirHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "list_dir handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ListDirArgs = parse_arguments(&arguments)?;

        let ListDirArgs {
            dir_path,
            offset,
            limit,
            depth,
        } = args;

        if offset == 0 {
            return Err(FunctionCallError::RespondToModel(
                "offset must be a 1-indexed entry number".to_string(),
            ));
        }

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        if depth == 0 {
            return Err(FunctionCallError::RespondToModel(
                "depth must be greater than zero".to_string(),
            ));
        }

        let path = PathBuf::from(&dir_path);
        if !path.is_absolute() {
            return Err(FunctionCallError::RespondToModel(
                "dir_path must be an absolute path".to_string(),
            ));
        }
        let read_deny_matcher = ReadDenyMatcher::new(&turn.file_system_sandbox_policy, turn.cwd());
        if read_deny_matcher
            .as_ref()
            .is_some_and(|matcher| matcher.is_read_denied(&path))
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "{DENY_READ_POLICY_MESSAGE}: `{}`",
                path.display()
            )));
        }

        let entries =
            list_dir_slice_with_policy(&path, offset, limit, depth, read_deny_matcher.as_ref())
                .await?;
        let mut output = Vec::with_capacity(entries.len() + 1);
        output.push(format!("Absolute path: {}", path.display()));
        output.extend(entries);
        Ok(FunctionToolOutput::from_text(output.join("\n"), Some(true)))
    }
}

async fn list_dir_slice_with_policy(
    path: &Path,
    offset: usize,
    limit: usize,
    depth: usize,
    read_deny_matcher: Option<&ReadDenyMatcher>,
) -> Result<Vec<String>, FunctionCallError> {
    let mut entries = Vec::new();
    collect_entries(path, Path::new(""), depth, read_deny_matcher, &mut entries).await?;

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    let start_index = offset - 1;
    if start_index >= entries.len() {
        return Err(FunctionCallError::RespondToModel(
            "offset exceeds directory entry count".to_string(),
        ));
    }

    let remaining_entries = entries.len() - start_index;
    let capped_limit = limit.min(remaining_entries);
    let end_index = start_index + capped_limit;
    let selected_entries = &entries[start_index..end_index];
    let mut formatted = Vec::with_capacity(selected_entries.len());

    for entry in selected_entries {
        formatted.push(format_entry_line(entry));
    }

    if end_index < entries.len() {
        formatted.push(format!("More than {capped_limit} entries found"));
    }

    Ok(formatted)
}

async fn collect_entries(
    dir_path: &Path,
    relative_prefix: &Path,
    depth: usize,
    read_deny_matcher: Option<&ReadDenyMatcher>,
    entries: &mut Vec<DirEntry>,
) -> Result<(), FunctionCallError> {
    let mut queue = VecDeque::new();
    queue.push_back((dir_path.to_path_buf(), relative_prefix.to_path_buf(), depth));

    while let Some((current_dir, prefix, remaining_depth)) = queue.pop_front() {
        let mut read_dir = fs::read_dir(&current_dir).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to read directory: {err}"))
        })?;

        let mut dir_entries = Vec::new();

        while let Some(entry) = read_dir.next_entry().await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to read directory: {err}"))
        })? {
            let entry_path = entry.path();
            if let Some(read_deny_matcher) = read_deny_matcher
                && read_deny_matcher.is_read_denied(&entry_path)
            {
                continue;
            }

            let file_type = entry.file_type().await.map_err(|err| {
                FunctionCallError::RespondToModel(format!("failed to inspect entry: {err}"))
            })?;

            let file_name = entry.file_name();
            let relative_path = if prefix.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                prefix.join(&file_name)
            };

            let display_name = format_entry_component(&file_name);
            let display_depth = prefix.components().count();
            let sort_key = format_entry_name(&relative_path);
            let kind = DirEntryKind::from(&file_type);
            dir_entries.push((
                entry_path,
                relative_path,
                kind,
                DirEntry {
                    name: sort_key,
                    display_name,
                    depth: display_depth,
                    kind,
                },
            ));
        }

        dir_entries.sort_unstable_by(|a, b| a.3.name.cmp(&b.3.name));

        for (entry_path, relative_path, kind, dir_entry) in dir_entries {
            if kind == DirEntryKind::Directory && remaining_depth > 1 {
                queue.push_back((entry_path, relative_path, remaining_depth - 1));
            }
            entries.push(dir_entry);
        }
    }

    Ok(())
}

fn format_entry_name(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace("\\", "/");
    if normalized.len() > MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized
    }
}

fn format_entry_component(name: &OsStr) -> String {
    let normalized = name.to_string_lossy();
    if normalized.len() > MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized.to_string()
    }
}

fn format_entry_line(entry: &DirEntry) -> String {
    let indent = " ".repeat(entry.depth * INDENTATION_SPACES);
    let mut name = entry.display_name.clone();
    match entry.kind {
        DirEntryKind::Directory => name.push('/'),
        DirEntryKind::Symlink => name.push('@'),
        DirEntryKind::Other => name.push('?'),
        DirEntryKind::File => {}
    }
    format!("{indent}{name}")
}

#[derive(Clone)]
struct DirEntry {
    name: String,
    display_name: String,
    depth: usize,
    kind: DirEntryKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<&FileType> for DirEntryKind {
    fn from(file_type: &FileType) -> Self {
        if file_type.is_symlink() {
            DirEntryKind::Symlink
        } else if file_type.is_dir() {
            DirEntryKind::Directory
        } else if file_type.is_file() {
            DirEntryKind::File
        } else {
            DirEntryKind::Other
        }
    }
}

#[cfg(test)]
#[path = "list_dir_tests.rs"]
mod tests;

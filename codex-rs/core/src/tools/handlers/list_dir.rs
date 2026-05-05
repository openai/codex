use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_string::take_bytes_at_char_boundary;
use serde::Deserialize;

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
    #[serde(default)]
    environment_id: Option<String>,
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
            environment_id,
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
        let path = AbsolutePathBuf::from_absolute_path_checked(&path).map_err(|_| {
            FunctionCallError::RespondToModel("dir_path must be an absolute path".to_string())
        })?;
        let Some(turn_environment) = turn.environments.select(environment_id.as_deref()) else {
            let message = environment_id.map_or_else(
                || "list_dir is unavailable in this session".to_string(),
                |environment_id| format!("unknown turn environment id `{environment_id}`"),
            );
            return Err(FunctionCallError::RespondToModel(message));
        };
        let cwd = turn_environment.cwd.clone();
        let file_system_sandbox_policy = turn.file_system_sandbox_policy();
        let read_deny_matcher = ReadDenyMatcher::new(&file_system_sandbox_policy, &cwd);
        if read_deny_matcher
            .as_ref()
            .is_some_and(|matcher| matcher.is_read_denied(path.as_path()))
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "{DENY_READ_POLICY_MESSAGE}: `{}`",
                path.display()
            )));
        }

        let sandbox = turn_environment.environment.is_remote().then(|| {
            turn.file_system_sandbox_context_for_cwd(&cwd, /*additional_permissions*/ None)
        });
        let fs = turn_environment.environment.get_filesystem();
        let entries = list_dir_slice_with_policy(
            fs,
            &path,
            offset,
            limit,
            depth,
            read_deny_matcher.as_ref(),
            sandbox.as_ref(),
        )
        .await?;
        let mut output = Vec::with_capacity(entries.len() + 1);
        output.push(format!("Absolute path: {}", path.display()));
        output.extend(entries);
        Ok(FunctionToolOutput::from_text(output.join("\n"), Some(true)))
    }
}

async fn list_dir_slice_with_policy(
    fs: Arc<dyn ExecutorFileSystem>,
    path: &AbsolutePathBuf,
    offset: usize,
    limit: usize,
    depth: usize,
    read_deny_matcher: Option<&ReadDenyMatcher>,
    sandbox: Option<&FileSystemSandboxContext>,
) -> Result<Vec<String>, FunctionCallError> {
    let mut entries = Vec::new();
    collect_entries(
        fs,
        path,
        Path::new(""),
        depth,
        read_deny_matcher,
        sandbox,
        &mut entries,
    )
    .await?;

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
    fs: Arc<dyn ExecutorFileSystem>,
    dir_path: &AbsolutePathBuf,
    relative_prefix: &Path,
    depth: usize,
    read_deny_matcher: Option<&ReadDenyMatcher>,
    sandbox: Option<&FileSystemSandboxContext>,
    entries: &mut Vec<DirEntry>,
) -> Result<(), FunctionCallError> {
    let mut queue = VecDeque::new();
    queue.push_back((dir_path.clone(), relative_prefix.to_path_buf(), depth));

    while let Some((current_dir, prefix, remaining_depth)) = queue.pop_front() {
        let mut dir_entries = Vec::new();

        for entry in fs
            .read_directory(&current_dir, sandbox)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("failed to read directory: {err}"))
            })?
        {
            let entry_path = current_dir.join(&entry.file_name);
            if let Some(read_deny_matcher) = read_deny_matcher
                && read_deny_matcher.is_read_denied(entry_path.as_path())
            {
                continue;
            }
            let relative_path = if prefix.as_os_str().is_empty() {
                PathBuf::from(&entry.file_name)
            } else {
                prefix.join(&entry.file_name)
            };

            let display_name = format_entry_component(OsStr::new(&entry.file_name));
            let display_depth = prefix.components().count();
            let sort_key = format_entry_name(&relative_path);
            let kind = DirEntryKind::from(&entry);
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

impl From<&ReadDirectoryEntry> for DirEntryKind {
    fn from(entry: &ReadDirectoryEntry) -> Self {
        if entry.is_symlink {
            DirEntryKind::Symlink
        } else if entry.is_directory {
            DirEntryKind::Directory
        } else if entry.is_file {
            DirEntryKind::File
        } else {
            DirEntryKind::Other
        }
    }
}

#[cfg(test)]
#[path = "list_dir_tests.rs"]
mod tests;

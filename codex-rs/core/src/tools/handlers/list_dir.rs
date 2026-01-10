use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::FileType;
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use codex_utils_string::take_bytes_at_char_boundary;
use serde::Deserialize;
use tokio::fs;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ListDirHandler;

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

#[async_trait]
impl ToolHandler for ListDirHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

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

        let entries = list_dir_slice(&path, offset, limit, depth).await?;
        let mut output = Vec::with_capacity(entries.len() + 1);
        output.push(format!("Absolute path: {}", path.display()));
        output.extend(entries);
        Ok(ToolOutput::Function {
            content: output.join("\n"),
            content_items: None,
            success: Some(true),
        })
    }
}

async fn list_dir_slice(
    path: &Path,
    offset: usize,
    limit: usize,
    depth: usize,
) -> Result<Vec<String>, FunctionCallError> {
    let mut entries = Vec::new();
    collect_entries(path, Path::new(""), depth, &mut entries).await?;

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
                entry.path(),
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
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[tokio::test]
    async fn lists_directory_entries() {
        let temp = tempdir().expect("create tempdir");
        let dir_path = temp.path();

        let sub_dir = dir_path.join("nested");
        tokio::fs::create_dir(&sub_dir)
            .await
            .expect("create sub dir");

        let deeper_dir = sub_dir.join("deeper");
        tokio::fs::create_dir(&deeper_dir)
            .await
            .expect("create deeper dir");

        tokio::fs::write(dir_path.join("entry.txt"), b"content")
            .await
            .expect("write file");
        tokio::fs::write(sub_dir.join("child.txt"), b"child")
            .await
            .expect("write child");
        tokio::fs::write(deeper_dir.join("grandchild.txt"), b"grandchild")
            .await
            .expect("write grandchild");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link_path = dir_path.join("link");
            symlink(dir_path.join("entry.txt"), &link_path).expect("create symlink");
        }

        let entries = list_dir_slice(dir_path, 1, 20, 3)
            .await
            .expect("list directory");

        #[cfg(unix)]
        let expected = vec![
            "entry.txt".to_string(),
            "link@".to_string(),
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "    grandchild.txt".to_string(),
        ];

        #[cfg(not(unix))]
        let expected = vec![
            "entry.txt".to_string(),
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "    grandchild.txt".to_string(),
        ];

        assert_eq!(entries, expected);
    }

    #[tokio::test]
    async fn errors_when_offset_exceeds_entries() {
        let temp = tempdir().expect("create tempdir");
        let dir_path = temp.path();
        tokio::fs::create_dir(dir_path.join("nested"))
            .await
            .expect("create sub dir");

        let err = list_dir_slice(dir_path, 10, 1, 2)
            .await
            .expect_err("offset exceeds entries");
        assert_eq!(
            err,
            FunctionCallError::RespondToModel("offset exceeds directory entry count".to_string())
        );
    }

    #[tokio::test]
    async fn respects_depth_parameter() {
        let temp = tempdir().expect("create tempdir");
        let dir_path = temp.path();
        let nested = dir_path.join("nested");
        let deeper = nested.join("deeper");
        tokio::fs::create_dir(&nested).await.expect("create nested");
        tokio::fs::create_dir(&deeper).await.expect("create deeper");
        tokio::fs::write(dir_path.join("root.txt"), b"root")
            .await
            .expect("write root");
        tokio::fs::write(nested.join("child.txt"), b"child")
            .await
            .expect("write nested");
        tokio::fs::write(deeper.join("grandchild.txt"), b"deep")
            .await
            .expect("write deeper");

        let entries_depth_one = list_dir_slice(dir_path, 1, 10, 1)
            .await
            .expect("list depth 1");
        assert_eq!(
            entries_depth_one,
            vec!["nested/".to_string(), "root.txt".to_string(),]
        );

        let entries_depth_two = list_dir_slice(dir_path, 1, 20, 2)
            .await
            .expect("list depth 2");
        assert_eq!(
            entries_depth_two,
            vec![
                "nested/".to_string(),
                "  child.txt".to_string(),
                "  deeper/".to_string(),
                "root.txt".to_string(),
            ]
        );

        let entries_depth_three = list_dir_slice(dir_path, 1, 30, 3)
            .await
            .expect("list depth 3");
        assert_eq!(
            entries_depth_three,
            vec![
                "nested/".to_string(),
                "  child.txt".to_string(),
                "  deeper/".to_string(),
                "    grandchild.txt".to_string(),
                "root.txt".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn paginates_in_sorted_order() {
        let temp = tempdir().expect("create tempdir");
        let dir_path = temp.path();

        let dir_a = dir_path.join("a");
        let dir_b = dir_path.join("b");
        tokio::fs::create_dir(&dir_a).await.expect("create a");
        tokio::fs::create_dir(&dir_b).await.expect("create b");

        tokio::fs::write(dir_a.join("a_child.txt"), b"a")
            .await
            .expect("write a child");
        tokio::fs::write(dir_b.join("b_child.txt"), b"b")
            .await
            .expect("write b child");

        let first_page = list_dir_slice(dir_path, 1, 2, 2)
            .await
            .expect("list page one");
        assert_eq!(
            first_page,
            vec![
                "a/".to_string(),
                "  a_child.txt".to_string(),
                "More than 2 entries found".to_string()
            ]
        );

        let second_page = list_dir_slice(dir_path, 3, 2, 2)
            .await
            .expect("list page two");
        assert_eq!(
            second_page,
            vec!["b/".to_string(), "  b_child.txt".to_string()]
        );
    }

    #[tokio::test]
    async fn handles_large_limit_without_overflow() {
        let temp = tempdir().expect("create tempdir");
        let dir_path = temp.path();
        tokio::fs::write(dir_path.join("alpha.txt"), b"alpha")
            .await
            .expect("write alpha");
        tokio::fs::write(dir_path.join("beta.txt"), b"beta")
            .await
            .expect("write beta");
        tokio::fs::write(dir_path.join("gamma.txt"), b"gamma")
            .await
            .expect("write gamma");

        let entries = list_dir_slice(dir_path, 2, usize::MAX, 1)
            .await
            .expect("list without overflow");
        assert_eq!(
            entries,
            vec!["beta.txt".to_string(), "gamma.txt".to_string(),]
        );
    }

    #[tokio::test]
    async fn indicates_truncated_results() {
        let temp = tempdir().expect("create tempdir");
        let dir_path = temp.path();

        for idx in 0..40 {
            let file = dir_path.join(format!("file_{idx:02}.txt"));
            tokio::fs::write(file, b"content")
                .await
                .expect("write file");
        }

        let entries = list_dir_slice(dir_path, 1, 25, 1)
            .await
            .expect("list directory");
        assert_eq!(entries.len(), 26);
        assert_eq!(
            entries.last(),
            Some(&"More than 25 entries found".to_string())
        );
    }

    #[tokio::test]
    async fn truncation_respects_sorted_order() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();
        let nested = dir_path.join("nested");
        let deeper = nested.join("deeper");
        tokio::fs::create_dir(&nested).await?;
        tokio::fs::create_dir(&deeper).await?;
        tokio::fs::write(dir_path.join("root.txt"), b"root").await?;
        tokio::fs::write(nested.join("child.txt"), b"child").await?;
        tokio::fs::write(deeper.join("grandchild.txt"), b"deep").await?;

        let entries_depth_three = list_dir_slice(dir_path, 1, 3, 3).await?;
        assert_eq!(
            entries_depth_three,
            vec![
                "nested/".to_string(),
                "  child.txt".to_string(),
                "  deeper/".to_string(),
                "More than 3 entries found".to_string()
            ]
        );

        Ok(())
    }
}

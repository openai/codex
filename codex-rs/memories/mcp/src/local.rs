use crate::backend::DEFAULT_READ_MAX_TOKENS;
use crate::backend::ListMemoriesRequest;
use crate::backend::ListMemoriesResponse;
use crate::backend::MAX_LIST_RESULTS;
use crate::backend::MAX_SEARCH_RESULTS;
use crate::backend::MemoriesBackend;
use crate::backend::MemoriesBackendError;
use crate::backend::MemoryEntry;
use crate::backend::MemoryEntryType;
use crate::backend::MemorySearchMatch;
use crate::backend::ReadMemoryRequest;
use crate::backend::ReadMemoryResponse;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LocalMemoriesBackend {
    root: PathBuf,
}

impl LocalMemoriesBackend {
    pub fn from_codex_home(codex_home: &AbsolutePathBuf) -> Self {
        Self::from_memory_root(codex_home.join("memories").to_path_buf())
    }

    pub fn from_memory_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve_scoped_path(
        &self,
        relative_path: Option<&str>,
    ) -> Result<PathBuf, MemoriesBackendError> {
        let Some(relative_path) = relative_path else {
            return Ok(self.root.clone());
        };
        let relative = Path::new(relative_path);
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(MemoriesBackendError::invalid_path(
                relative_path,
                "must stay within the memories root",
            ));
        }
        Ok(self.root.join(relative))
    }

    async fn metadata_or_none(
        path: &Path,
    ) -> Result<Option<std::fs::Metadata>, MemoriesBackendError> {
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => Ok(Some(metadata)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

impl MemoriesBackend for LocalMemoriesBackend {
    async fn list(
        &self,
        request: ListMemoriesRequest,
    ) -> Result<ListMemoriesResponse, MemoriesBackendError> {
        let max_results = request.max_results.min(MAX_LIST_RESULTS);
        let start = self.resolve_scoped_path(request.path.as_deref())?;
        let start_index = match request.cursor.as_deref() {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| {
                MemoriesBackendError::invalid_cursor(cursor, "must be a non-negative integer")
            })?,
            None => 0,
        };
        let Some(metadata) = Self::metadata_or_none(&start).await? else {
            return Ok(ListMemoriesResponse {
                path: request.path,
                entries: Vec::new(),
                next_cursor: None,
                truncated: false,
            });
        };
        reject_symlink(&display_relative_path(&self.root, &start), &metadata)?;

        let mut entries = if metadata.is_file() {
            vec![MemoryEntry {
                path: display_relative_path(&self.root, &start),
                entry_type: MemoryEntryType::File,
            }]
        } else if metadata.is_dir() {
            let mut entries = Vec::new();
            for path in read_sorted_dir_paths(&start).await? {
                let Some(metadata) = Self::metadata_or_none(&path).await? else {
                    continue;
                };
                if metadata.file_type().is_symlink() {
                    continue;
                }

                let entry_type = if metadata.is_dir() {
                    MemoryEntryType::Directory
                } else if metadata.is_file() {
                    MemoryEntryType::File
                } else {
                    continue;
                };
                entries.push(MemoryEntry {
                    path: display_relative_path(&self.root, &path),
                    entry_type,
                });
            }
            entries
        } else {
            Vec::new()
        };
        if start_index > entries.len() {
            return Err(MemoriesBackendError::invalid_cursor(
                start_index.to_string(),
                "exceeds result count",
            ));
        }

        let end_index = start_index.saturating_add(max_results).min(entries.len());
        let next_cursor = (end_index < entries.len()).then(|| end_index.to_string());
        let truncated = next_cursor.is_some();
        Ok(ListMemoriesResponse {
            path: request.path,
            entries: entries.drain(start_index..end_index).collect(),
            next_cursor,
            truncated,
        })
    }

    async fn read(
        &self,
        request: ReadMemoryRequest,
    ) -> Result<ReadMemoryResponse, MemoriesBackendError> {
        if request.line_offset == 0 {
            return Err(MemoriesBackendError::InvalidLineOffset);
        }
        if request.max_lines == Some(0) {
            return Err(MemoriesBackendError::InvalidMaxLines);
        }

        let path = self.resolve_scoped_path(Some(request.path.as_str()))?;
        let Some(metadata) = Self::metadata_or_none(&path).await? else {
            return Err(MemoriesBackendError::NotFile { path: request.path });
        };
        reject_symlink(&request.path, &metadata)?;
        if !metadata.is_file() {
            return Err(MemoriesBackendError::NotFile { path: request.path });
        }

        let original_content = tokio::fs::read_to_string(&path).await?;
        let start_byte = line_start_byte_offset(&original_content, request.line_offset)?;
        let end_byte = line_end_byte_offset(&original_content, start_byte, request.max_lines);
        let content_from_offset = &original_content[start_byte..end_byte];
        let max_tokens = if request.max_tokens == 0 {
            DEFAULT_READ_MAX_TOKENS
        } else {
            request.max_tokens
        };
        let content = truncate_text(content_from_offset, TruncationPolicy::Tokens(max_tokens));
        let truncated = end_byte < original_content.len() || content != content_from_offset;
        Ok(ReadMemoryResponse {
            path: request.path,
            start_line_number: request.line_offset,
            content,
            truncated,
        })
    }

    async fn search(
        &self,
        request: SearchMemoriesRequest,
    ) -> Result<SearchMemoriesResponse, MemoriesBackendError> {
        let query = request.query.trim();
        if query.is_empty() {
            return Err(MemoriesBackendError::EmptyQuery);
        }

        let max_results = request.max_results.min(MAX_SEARCH_RESULTS);
        let start = self.resolve_scoped_path(request.path.as_deref())?;
        let start_index = match request.cursor.as_deref() {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| {
                MemoriesBackendError::invalid_cursor(cursor, "must be a non-negative integer")
            })?,
            None => 0,
        };
        let Some(metadata) = Self::metadata_or_none(&start).await? else {
            return Ok(SearchMemoriesResponse {
                query: request.query,
                path: request.path,
                matches: Vec::new(),
                next_cursor: None,
                truncated: false,
            });
        };
        reject_symlink(&display_relative_path(&self.root, &start), &metadata)?;

        let mut matches = Vec::new();
        search_entries(&self.root, &start, &metadata, query, &mut matches).await?;
        matches.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line_number.cmp(&right.line_number))
        });
        if start_index > matches.len() {
            return Err(MemoriesBackendError::invalid_cursor(
                start_index.to_string(),
                "exceeds result count",
            ));
        }
        let end_index = start_index.saturating_add(max_results).min(matches.len());
        let next_cursor = (end_index < matches.len()).then(|| end_index.to_string());
        let truncated = next_cursor.is_some();
        Ok(SearchMemoriesResponse {
            query: request.query,
            path: request.path,
            matches: matches.drain(start_index..end_index).collect(),
            next_cursor,
            truncated,
        })
    }
}

async fn search_entries(
    root: &Path,
    current: &Path,
    current_metadata: &std::fs::Metadata,
    query: &str,
    matches: &mut Vec<MemorySearchMatch>,
) -> Result<(), MemoriesBackendError> {
    if current_metadata.is_file() {
        search_file(root, current, query, matches).await?;
        return Ok(());
    }
    if !current_metadata.is_dir() {
        return Ok(());
    }

    let mut pending = vec![current.to_path_buf()];
    while let Some(dir_path) = pending.pop() {
        for path in read_sorted_dir_paths(&dir_path).await? {
            let Some(metadata) = LocalMemoriesBackend::metadata_or_none(&path).await? else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                search_file(root, &path, query, matches).await?;
            }
        }
    }

    Ok(())
}

async fn search_file(
    root: &Path,
    path: &Path,
    query: &str,
    matches: &mut Vec<MemorySearchMatch>,
) -> Result<(), MemoriesBackendError> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::InvalidData => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    for (idx, line) in content.lines().enumerate() {
        if line.contains(query) {
            matches.push(MemorySearchMatch {
                path: display_relative_path(root, path),
                line_number: idx + 1,
                line: line.to_string(),
            });
        }
    }
    Ok(())
}

async fn read_sorted_dir_paths(dir_path: &Path) -> Result<Vec<PathBuf>, MemoriesBackendError> {
    let mut dir = match tokio::fs::read_dir(dir_path).await {
        Ok(dir) => dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut paths = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

fn reject_symlink(path: &str, metadata: &std::fs::Metadata) -> Result<(), MemoriesBackendError> {
    if metadata.file_type().is_symlink() {
        return Err(MemoriesBackendError::invalid_path(
            path,
            "must not be a symlink",
        ));
    }
    Ok(())
}

fn display_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn line_start_byte_offset(
    content: &str,
    line_offset: usize,
) -> Result<usize, MemoriesBackendError> {
    if line_offset == 1 {
        return Ok(0);
    }

    let mut current_line = 1;
    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == line_offset {
                return Ok(idx + 1);
            }
        }
    }

    Err(MemoriesBackendError::LineOffsetExceedsFileLength)
}

fn line_end_byte_offset(content: &str, start_byte: usize, max_lines: Option<usize>) -> usize {
    let Some(max_lines) = max_lines else {
        return content.len();
    };

    let mut lines_seen = 1;
    for (relative_idx, ch) in content[start_byte..].char_indices() {
        if ch == '\n' {
            if lines_seen == max_lines {
                return start_byte + relative_idx + 1;
            }
            lines_seen += 1;
        }
    }

    content.len()
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;

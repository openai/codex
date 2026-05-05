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
use crate::backend::SearchMatchMode;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use std::borrow::Cow;
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

    async fn resolve_scoped_path(
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
        if relative.components().any(is_hidden_component) {
            return Err(MemoriesBackendError::NotFound {
                path: relative_path.to_string(),
            });
        }

        let components = relative.components().collect::<Vec<_>>();
        let mut scoped_path = self.root.clone();
        for (idx, component) in components.iter().enumerate() {
            scoped_path.push(component.as_os_str());

            let Some(metadata) = Self::metadata_or_none(&scoped_path).await? else {
                for remaining_component in components.iter().skip(idx + 1) {
                    scoped_path.push(remaining_component.as_os_str());
                }
                return Ok(scoped_path);
            };

            reject_symlink(&display_relative_path(&self.root, &scoped_path), &metadata)?;
            if idx + 1 < components.len() && !metadata.is_dir() {
                return Err(MemoriesBackendError::invalid_path(
                    relative_path,
                    "traverses through a non-directory path component",
                ));
            }
        }

        Ok(scoped_path)
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
        let start = self.resolve_scoped_path(request.path.as_deref()).await?;
        let start_index = match request.cursor.as_deref() {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| {
                MemoriesBackendError::invalid_cursor(cursor, "must be a non-negative integer")
            })?,
            None => 0,
        };
        let Some(metadata) = Self::metadata_or_none(&start).await? else {
            return Err(MemoriesBackendError::NotFound {
                path: request.path.unwrap_or_default(),
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
                if is_hidden_path(&path) {
                    continue;
                }
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

        let path = self
            .resolve_scoped_path(Some(request.path.as_str()))
            .await?;
        let Some(metadata) = Self::metadata_or_none(&path).await? else {
            return Err(MemoriesBackendError::NotFound { path: request.path });
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
        let queries = request
            .queries
            .iter()
            .map(|query| query.trim().to_string())
            .collect::<Vec<_>>();
        if queries.is_empty() || queries.iter().any(std::string::String::is_empty) {
            return Err(MemoriesBackendError::EmptyQuery);
        }
        if matches!(
            request.match_mode,
            SearchMatchMode::AllWithinLines { line_count: 0 }
        ) {
            return Err(MemoriesBackendError::InvalidMatchWindow);
        }

        let max_results = request.max_results.min(MAX_SEARCH_RESULTS);
        let start = self.resolve_scoped_path(request.path.as_deref()).await?;
        let start_index = match request.cursor.as_deref() {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| {
                MemoriesBackendError::invalid_cursor(cursor, "must be a non-negative integer")
            })?,
            None => 0,
        };
        let Some(metadata) = Self::metadata_or_none(&start).await? else {
            return Err(MemoriesBackendError::NotFound {
                path: request.path.unwrap_or_default(),
            });
        };
        reject_symlink(&display_relative_path(&self.root, &start), &metadata)?;

        let matcher = SearchMatcher::new(
            queries.clone(),
            request.match_mode.clone(),
            request.case_sensitive,
        );
        let mut matches = Vec::new();
        search_entries(
            &self.root,
            &start,
            &metadata,
            &matcher,
            request.context_lines,
            &mut matches,
        )
        .await?;
        matches.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.match_line_number.cmp(&right.match_line_number))
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
            queries,
            match_mode: request.match_mode,
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
    matcher: &SearchMatcher,
    context_lines: usize,
    matches: &mut Vec<MemorySearchMatch>,
) -> Result<(), MemoriesBackendError> {
    if current_metadata.is_file() {
        search_file(root, current, matcher, context_lines, matches).await?;
        return Ok(());
    }
    if !current_metadata.is_dir() {
        return Ok(());
    }

    let mut pending = vec![current.to_path_buf()];
    while let Some(dir_path) = pending.pop() {
        for path in read_sorted_dir_paths(&dir_path).await? {
            if is_hidden_path(&path) {
                continue;
            }
            let Some(metadata) = LocalMemoriesBackend::metadata_or_none(&path).await? else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                search_file(root, &path, matcher, context_lines, matches).await?;
            }
        }
    }

    Ok(())
}

async fn search_file(
    root: &Path,
    path: &Path,
    matcher: &SearchMatcher,
    context_lines: usize,
    matches: &mut Vec<MemorySearchMatch>,
) -> Result<(), MemoriesBackendError> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::InvalidData => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let lines = content.lines().collect::<Vec<_>>();
    let line_matches = lines
        .iter()
        .map(|line| matcher.matched_query_flags(line))
        .collect::<Vec<_>>();
    match &matcher.match_mode {
        SearchMatchMode::Any => {
            for (idx, matched_query_flags) in line_matches.iter().enumerate() {
                if matched_query_flags.iter().any(|matched| *matched) {
                    matches.push(build_search_match(
                        root,
                        path,
                        &lines,
                        idx,
                        idx,
                        context_lines,
                        matcher.matched_queries(matched_query_flags),
                    ));
                }
            }
        }
        SearchMatchMode::AllOnSameLine => {
            for (idx, matched_query_flags) in line_matches.iter().enumerate() {
                if matched_query_flags.iter().all(|matched| *matched) {
                    matches.push(build_search_match(
                        root,
                        path,
                        &lines,
                        idx,
                        idx,
                        context_lines,
                        matcher.matched_queries(matched_query_flags),
                    ));
                }
            }
        }
        SearchMatchMode::AllWithinLines { line_count } => {
            let mut windows = Vec::new();
            for start_index in 0..lines.len() {
                if !line_matches[start_index].iter().any(|matched| *matched) {
                    continue;
                }
                let last_allowed_index = start_index
                    .saturating_add(line_count.saturating_sub(1))
                    .min(lines.len().saturating_sub(1));
                let mut matched_query_flags = vec![false; matcher.queries.len()];
                for (end_index, line_match_flags) in line_matches
                    .iter()
                    .enumerate()
                    .take(last_allowed_index + 1)
                    .skip(start_index)
                {
                    for (idx, matched) in line_match_flags.iter().enumerate() {
                        matched_query_flags[idx] |= matched;
                    }
                    if matched_query_flags.iter().all(|matched| *matched) {
                        windows.push((start_index, end_index, matched_query_flags));
                        break;
                    }
                }
            }
            for (idx, (start_index, end_index, matched_query_flags)) in windows.iter().enumerate() {
                let strictly_contains_another_window = windows.iter().enumerate().any(
                    |(other_idx, (other_start_index, other_end_index, _))| {
                        idx != other_idx
                            && start_index <= other_start_index
                            && end_index >= other_end_index
                            && (start_index != other_start_index || end_index != other_end_index)
                    },
                );
                if strictly_contains_another_window {
                    continue;
                }
                matches.push(build_search_match(
                    root,
                    path,
                    &lines,
                    *start_index,
                    *end_index,
                    context_lines,
                    matcher.matched_queries(matched_query_flags),
                ));
            }
        }
    }
    Ok(())
}

fn build_search_match(
    root: &Path,
    path: &Path,
    lines: &[&str],
    match_start_index: usize,
    match_end_index: usize,
    context_lines: usize,
    matched_queries: Vec<String>,
) -> MemorySearchMatch {
    let content_start_index = match_start_index.saturating_sub(context_lines);
    let content_end_index = match_end_index
        .saturating_add(context_lines)
        .saturating_add(1)
        .min(lines.len());
    MemorySearchMatch {
        path: display_relative_path(root, path),
        match_line_number: match_start_index + 1,
        content_start_line_number: content_start_index + 1,
        content: lines[content_start_index..content_end_index].join("\n"),
        matched_queries,
    }
}

struct SearchMatcher {
    queries: Vec<String>,
    normalized_queries: Option<Vec<String>>,
    match_mode: SearchMatchMode,
}

impl SearchMatcher {
    fn new(queries: Vec<String>, match_mode: SearchMatchMode, case_sensitive: bool) -> Self {
        let normalized_queries = (!case_sensitive).then(|| {
            queries
                .iter()
                .map(|query| query.to_lowercase())
                .collect::<Vec<_>>()
        });
        Self {
            queries,
            normalized_queries,
            match_mode,
        }
    }

    fn matched_query_flags(&self, line: &str) -> Vec<bool> {
        let line = match self.normalized_queries.as_ref() {
            Some(_) => Cow::Owned(line.to_lowercase()),
            None => Cow::Borrowed(line),
        };
        let queries = self.normalized_queries.as_deref().unwrap_or(&self.queries);
        queries
            .iter()
            .map(|query| line.as_ref().contains(query))
            .collect()
    }

    fn matched_queries(&self, matched_query_flags: &[bool]) -> Vec<String> {
        self.queries
            .iter()
            .zip(matched_query_flags)
            .filter_map(|(query, matched)| matched.then_some(query.clone()))
            .collect()
    }
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

fn is_hidden_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Normal(name) if name.to_string_lossy().starts_with('.')
    )
}

fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
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

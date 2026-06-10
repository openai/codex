use std::borrow::Cow;
use std::path::Path;

use crate::MAX_SEARCH_RESULTS;
use crate::backend::MemoriesBackendError;
use crate::backend::MemorySearchMatch;
use crate::backend::SearchMatchMode;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;

use super::LocalMemoriesBackend;
use super::path::display_relative_path;
use super::path::is_hidden_path;
use super::path::read_sorted_dir_paths;
use super::path::reject_symlink;

pub(super) async fn search(
    backend: &LocalMemoriesBackend,
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
    let start = backend.resolve_scoped_path(request.path.as_deref()).await?;
    let start_index = match request.cursor.as_deref() {
        Some(cursor) => cursor.parse::<usize>().map_err(|_| {
            MemoriesBackendError::invalid_cursor(cursor, "must be a non-negative integer")
        })?,
        None => 0,
    };
    let Some(metadata) = LocalMemoriesBackend::metadata_or_none(&start).await? else {
        return Err(MemoriesBackendError::NotFound {
            path: request.path.unwrap_or_default(),
        });
    };
    reject_symlink(&display_relative_path(&backend.root, &start), &metadata)?;

    let matcher = SearchMatcher::new(
        queries.clone(),
        request.match_mode.clone(),
        request.case_sensitive,
        request.normalized,
    )?;
    let mut matches = Vec::new();
    search_entries(
        &backend.root,
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
    prepared_queries: Vec<String>,
    comparison: SearchComparison,
    match_mode: SearchMatchMode,
}

impl SearchMatcher {
    fn new(
        queries: Vec<String>,
        match_mode: SearchMatchMode,
        case_sensitive: bool,
        normalized: bool,
    ) -> Result<Self, MemoriesBackendError> {
        let comparison = SearchComparison::new(case_sensitive, normalized);
        let prepared_queries = queries
            .iter()
            .map(|query| comparison.prepare(query))
            .map(Cow::into_owned)
            .collect::<Vec<_>>();
        if prepared_queries.iter().any(std::string::String::is_empty) {
            return Err(MemoriesBackendError::EmptyQuery);
        }
        Ok(Self {
            queries,
            prepared_queries,
            comparison,
            match_mode,
        })
    }

    fn matched_query_flags(&self, line: &str) -> Vec<bool> {
        let line = self.comparison.prepare(line);
        self.prepared_queries
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

#[derive(Clone, Copy)]
struct SearchComparison {
    case_sensitive: bool,
    normalized: bool,
}

impl SearchComparison {
    fn new(case_sensitive: bool, normalized: bool) -> Self {
        Self {
            case_sensitive,
            normalized,
        }
    }

    fn prepare<'a>(self, value: &'a str) -> Cow<'a, str> {
        if self.case_sensitive && !self.normalized {
            return Cow::Borrowed(value);
        }

        let value = if self.case_sensitive {
            Cow::Borrowed(value)
        } else {
            Cow::Owned(value.to_lowercase())
        };
        if !self.normalized {
            return value;
        }

        Cow::Owned(
            value
                .chars()
                .filter(|ch| ch.is_alphanumeric())
                .collect::<String>(),
        )
    }
}

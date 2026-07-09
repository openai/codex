use crate::ExecutorFileSystem;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;
use std::io;

const MAX_CONCURRENT_PROBES: usize = 8;
const MAX_CONCURRENT_FIND_UP_REQUESTS: usize = 8;
pub const MAX_FIND_UP_CANDIDATES: usize = 16;
pub const MAX_FIND_UP_CANDIDATE_BYTES: usize = 256;
pub const MAX_FIND_UP_TOTAL_CANDIDATE_BYTES: usize = 2 * 1024;
pub const MAX_FIND_UP_IGNORED_ERRORS: usize = 64;
pub const MAX_FIND_UP_IGNORED_ERROR_MESSAGE_BYTES: usize = 1024;
pub const MAX_FIND_UP_IGNORED_ERROR_DETAILS_BYTES: usize = 64 * 1024;
const FIND_UP_IGNORED_ERROR_ITEM_OVERHEAD_BYTES: usize = 64;

/// Controls how an upward marker search handles metadata errors other than `NotFound`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FindUpErrorPolicy {
    /// Return the first error in lexical search order.
    Propagate,
    /// Treat errors as missing markers and continue searching.
    Ignore,
}

/// Filesystem entry kind that qualifies as an upward-search match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FindUpMatchKind {
    Any,
    File,
    Directory,
}

impl FindUpMatchKind {
    fn matches(self, metadata: &crate::FileMetadata) -> bool {
        match self {
            Self::Any => true,
            Self::File => metadata.is_file,
            Self::Directory => metadata.is_directory,
        }
    }
}

/// Bounded options for an upward filesystem search.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpOptions {
    /// Relative paths tested in order beneath each ancestor.
    pub candidate_relative_paths: Vec<String>,
    pub match_kind: FindUpMatchKind,
    pub non_not_found_error_policy: FindUpErrorPolicy,
}

/// One independent upward filesystem search in a batch.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpRequest {
    pub start: PathUri,
    pub options: FindUpOptions,
}

/// First qualifying path found by an upward filesystem search.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpMatch {
    pub ancestor: PathUri,
    pub path: PathUri,
}

/// A recoverable metadata error encountered during an upward filesystem search.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpError {
    pub path: PathUri,
    pub message: String,
}

/// Result and generic work counters from an upward filesystem search.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpOutcome {
    pub matched: Option<FindUpMatch>,
    pub visited_ancestor_count: usize,
    pub metadata_probe_count: usize,
    /// Total number of ignored errors, including any omitted from `ignored_errors`.
    pub ignored_error_count: usize,
    /// Ordered prefix of ignored errors, bounded by count, message size, and aggregate bytes.
    #[serde(default)]
    pub ignored_errors: Vec<FindUpError>,
    /// Whether one or more ignored errors could not be returned within the bounds.
    #[serde(default)]
    pub ignored_errors_truncated: bool,
}

pub(super) async fn find_up_via_metadata(
    file_system: &(impl ExecutorFileSystem + ?Sized),
    start: &PathUri,
    options: &FindUpOptions,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<FindUpOutcome> {
    validate_find_up_options(start, options)?;

    let mut outcome = FindUpOutcome::default();
    let mut ignored_error_detail_bytes = 0usize;
    for ancestor in start.ancestors() {
        outcome.visited_ancestor_count += 1;
        for candidate in &options.candidate_relative_paths {
            let path = checked_candidate_path(&ancestor, candidate)?;
            outcome.metadata_probe_count += 1;
            match file_system.get_metadata(&path, sandbox).await {
                Ok(metadata) if options.match_kind.matches(&metadata) => {
                    outcome.matched = Some(FindUpMatch { ancestor, path });
                    return Ok(outcome);
                }
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => match options.non_not_found_error_policy {
                    FindUpErrorPolicy::Propagate => return Err(err),
                    FindUpErrorPolicy::Ignore => {
                        outcome.ignored_error_count += 1;
                        record_ignored_error(
                            &mut outcome,
                            &mut ignored_error_detail_bytes,
                            path,
                            &err,
                        );
                    }
                },
            }
        }
    }
    Ok(outcome)
}

fn record_ignored_error(
    outcome: &mut FindUpOutcome,
    detail_bytes: &mut usize,
    path: PathUri,
    err: &io::Error,
) {
    if outcome.ignored_errors_truncated {
        return;
    }
    let Some(message) = bounded_error_message(err) else {
        outcome.ignored_errors_truncated = true;
        return;
    };
    if outcome.ignored_errors.len() == MAX_FIND_UP_IGNORED_ERRORS {
        outcome.ignored_errors_truncated = true;
        return;
    }
    let Some(fixed_item_bytes) = message
        .len()
        .checked_add(FIND_UP_IGNORED_ERROR_ITEM_OVERHEAD_BYTES)
    else {
        outcome.ignored_errors_truncated = true;
        return;
    };
    let Some(remaining_path_bytes) = MAX_FIND_UP_IGNORED_ERROR_DETAILS_BYTES
        .checked_sub(*detail_bytes)
        .and_then(|remaining| remaining.checked_sub(fixed_item_bytes))
    else {
        outcome.ignored_errors_truncated = true;
        return;
    };
    let Some(path_bytes) = formatted_len_at_most(&path, remaining_path_bytes) else {
        outcome.ignored_errors_truncated = true;
        return;
    };
    *detail_bytes += fixed_item_bytes + path_bytes;
    outcome.ignored_errors.push(FindUpError { path, message });
}

fn formatted_len_at_most(value: &impl std::fmt::Display, max_bytes: usize) -> Option<usize> {
    struct BoundedLength {
        bytes: usize,
        max_bytes: usize,
        exceeded: bool,
    }

    impl std::fmt::Write for BoundedLength {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            let Some(bytes) = self.bytes.checked_add(value.len()) else {
                self.exceeded = true;
                return Ok(());
            };
            if bytes > self.max_bytes {
                self.exceeded = true;
            } else if !self.exceeded {
                self.bytes = bytes;
            }
            Ok(())
        }
    }

    let mut length = BoundedLength {
        bytes: 0,
        max_bytes,
        exceeded: false,
    };
    std::fmt::write(&mut length, format_args!("{value}")).ok()?;
    (!length.exceeded).then_some(length.bytes)
}

fn bounded_error_message(err: &io::Error) -> Option<String> {
    struct BoundedMessage {
        value: String,
        truncated: bool,
    }

    impl std::fmt::Write for BoundedMessage {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            if self.value.len().saturating_add(value.len())
                > MAX_FIND_UP_IGNORED_ERROR_MESSAGE_BYTES
            {
                self.truncated = true;
            } else if !self.truncated {
                self.value.push_str(value);
            }
            Ok(())
        }
    }

    let mut message = BoundedMessage {
        value: String::new(),
        truncated: false,
    };
    std::fmt::write(&mut message, format_args!("{err:#}")).ok()?;
    (!message.truncated).then_some(message.value)
}

pub(super) async fn find_up_batch_via_individual(
    file_system: &(impl ExecutorFileSystem + ?Sized),
    requests: &[FindUpRequest],
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<Vec<FileSystemResult<FindUpOutcome>>> {
    Ok(futures::stream::iter(requests.iter().cloned())
        .map(|request| async move {
            file_system
                .find_up(&request.start, &request.options, sandbox)
                .await
        })
        .buffered(MAX_CONCURRENT_FIND_UP_REQUESTS)
        .collect()
        .await)
}

fn validate_find_up_options(start: &PathUri, options: &FindUpOptions) -> FileSystemResult<()> {
    let candidate_count = options.candidate_relative_paths.len();
    if candidate_count == 0 || candidate_count > MAX_FIND_UP_CANDIDATES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "filesystem find-up candidate count must be between 1 and {MAX_FIND_UP_CANDIDATES}, got {candidate_count}"
            ),
        ));
    }

    let mut total_bytes = 0usize;
    for candidate in &options.candidate_relative_paths {
        let candidate_bytes = candidate.len();
        if candidate_bytes == 0 || candidate_bytes > MAX_FIND_UP_CANDIDATE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "filesystem find-up candidates must be between 1 and {MAX_FIND_UP_CANDIDATE_BYTES} bytes"
                ),
            ));
        }
        total_bytes = total_bytes.checked_add(candidate_bytes).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "filesystem find-up candidate size overflow",
            )
        })?;
        checked_candidate_path(start, candidate)?;
    }
    if total_bytes > MAX_FIND_UP_TOTAL_CANDIDATE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "filesystem find-up candidates must not exceed {MAX_FIND_UP_TOTAL_CANDIDATE_BYTES} bytes total"
            ),
        ));
    }
    Ok(())
}

fn checked_candidate_path(ancestor: &PathUri, candidate: &str) -> FileSystemResult<PathUri> {
    let path = ancestor
        .join(candidate)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    if path == *ancestor || !path.starts_with(ancestor) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem find-up candidate {candidate:?} escapes ancestor {ancestor}"),
        ));
    }
    Ok(path)
}

/// Finds the nearest ancestor containing one of the provided marker names.
///
/// Marker paths are probed in lexical order from `start` toward the filesystem root. A bounded
/// number of ordinary metadata calls are kept in flight so remote filesystems can pipeline them
/// without requiring a batch protocol operation.
pub async fn find_nearest_ancestor_with_markers(
    file_system: &dyn ExecutorFileSystem,
    start: &PathUri,
    markers: Vec<String>,
    error_policy: FindUpErrorPolicy,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<Option<PathUri>> {
    find_nearest_ancestor(
        file_system,
        start.clone(),
        markers,
        PathUri::parent,
        |ancestor, marker| {
            ancestor
                .join(marker)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
        },
        error_policy,
        sandbox,
    )
    .await
}

/// Finds the nearest native ancestor containing one of the provided marker names.
///
/// Ancestors and marker paths remain native until each complete probe is converted to a URI. This
/// preserves paths that require an opaque [`PathUri`] fallback.
pub async fn find_nearest_native_ancestor_with_markers(
    file_system: &dyn ExecutorFileSystem,
    start: &AbsolutePathBuf,
    markers: Vec<String>,
    error_policy: FindUpErrorPolicy,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<Option<AbsolutePathBuf>> {
    find_nearest_ancestor(
        file_system,
        start.clone(),
        markers,
        AbsolutePathBuf::parent,
        |ancestor, marker| Ok(PathUri::from_abs_path(&ancestor.join(marker))),
        error_policy,
        sandbox,
    )
    .await
}

async fn find_nearest_ancestor<P, Parent, MarkerPath>(
    file_system: &dyn ExecutorFileSystem,
    start: P,
    markers: Vec<String>,
    parent: Parent,
    mut marker_path: MarkerPath,
    error_policy: FindUpErrorPolicy,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<Option<P>>
where
    P: Clone + Send,
    Parent: FnMut(&P) -> Option<P> + Send,
    MarkerPath: FnMut(&P, &str) -> FileSystemResult<PathUri> + Send,
{
    let mut ancestors = std::iter::successors(Some(start), parent);
    let mut ancestor = ancestors.next();
    let mut marker_index = 0;
    let probes = std::iter::from_fn(move || {
        let current_ancestor = ancestor.clone()?;
        let marker = markers.get(marker_index)?;
        let marker_path = marker_path(&current_ancestor, marker);

        marker_index += 1;
        if marker_index == markers.len() {
            marker_index = 0;
            ancestor = ancestors.next();
        }

        Some((current_ancestor, marker_path))
    });
    let mut results = futures::stream::iter(probes)
        .map(|(ancestor, marker_path)| async move {
            let marker_path = marker_path?;
            match file_system.get_metadata(&marker_path, sandbox).await {
                Ok(_) => Ok(Some(ancestor)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => match error_policy {
                    FindUpErrorPolicy::Propagate => Err(err),
                    FindUpErrorPolicy::Ignore => Ok(None),
                },
            }
        })
        .buffered(MAX_CONCURRENT_PROBES);

    while let Some(result) = results.next().await {
        if let Some(ancestor) = result? {
            return Ok(Some(ancestor));
        }
    }
    Ok(None)
}

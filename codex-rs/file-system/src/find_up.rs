use crate::ExecutorFileSystem;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;
use std::io;

const MAX_CONCURRENT_PROBES: usize = 8;
pub const MAX_FIND_UP_CANDIDATES: usize = 16;
pub const MAX_FIND_UP_CANDIDATE_BYTES: usize = 256;
pub const MAX_FIND_UP_TOTAL_CANDIDATE_BYTES: usize = 2 * 1024;

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

/// First qualifying path found by an upward filesystem search.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpMatch {
    pub ancestor: PathUri,
    pub path: PathUri,
}

/// Result and generic work counters from an upward filesystem search.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindUpOutcome {
    pub matched: Option<FindUpMatch>,
    pub visited_ancestor_count: usize,
    pub metadata_probe_count: usize,
    pub ignored_error_count: usize,
}

pub(super) async fn find_up_via_metadata(
    file_system: &(impl ExecutorFileSystem + ?Sized),
    start: &PathUri,
    options: &FindUpOptions,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<FindUpOutcome> {
    validate_find_up_options(start, options)?;

    let mut outcome = FindUpOutcome::default();
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
                    FindUpErrorPolicy::Ignore => outcome.ignored_error_count += 1,
                },
            }
        }
    }
    Ok(outcome)
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

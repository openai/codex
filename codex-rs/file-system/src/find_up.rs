use crate::ExecutorFileSystem;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;
use std::io;

const MAX_CONCURRENT_PROBES: usize = 8;

/// Controls how an upward marker search handles metadata errors other than `NotFound`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindUpErrorPolicy {
    /// Return the first error in lexical search order.
    Propagate,
    /// Treat errors as missing markers and continue searching.
    Ignore,
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

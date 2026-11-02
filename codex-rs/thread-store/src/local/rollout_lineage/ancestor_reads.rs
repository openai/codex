//! Retries reads from the same leaf when an ancestor cannot supply its frozen history.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;

use super::LocalThreadStore;
use super::RolloutLineage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

#[derive(Clone, Debug, Default)]
pub(in crate::local) struct AncestorReads(Option<Arc<TrackedAncestors>>);

#[derive(Debug)]
struct TrackedAncestors {
    cutoffs: HashMap<PathBuf, (HistoryPosition, ThreadId)>,
    failed_path: OnceLock<PathBuf>,
}

pub(super) struct FailedAncestor {
    pub(super) path: PathBuf,
    pub(super) owner: ThreadId,
    pub(super) error: ThreadStoreError,
}

impl AncestorReads {
    pub(super) fn new(cutoffs: HashMap<PathBuf, (HistoryPosition, ThreadId)>) -> Self {
        Self((!cutoffs.is_empty()).then(|| {
            Arc::new(TrackedAncestors {
                cutoffs,
                failed_path: OnceLock::new(),
            })
        }))
    }

    pub(in crate::local) fn cutoff(&self, path: &Path) -> Option<HistoryPosition> {
        self.0.as_ref()?.cutoffs.get(path).map(|(end, _)| *end)
    }

    pub(in crate::local) fn tracks(&self, path: &Path) -> bool {
        self.cutoff(path).is_some()
    }

    pub(in crate::local) fn record_failure(&self, path: &Path) {
        if let Some(ancestors) = &self.0
            && ancestors.cutoffs.contains_key(path)
        {
            ancestors.failed_path.get_or_init(|| path.to_owned());
        }
    }

    /// Validate the inherited offset on the reader that will actually consume the file.
    pub(in crate::local) fn open<F: Fn() -> bool>(
        &self,
        path: &Path,
        is_cancelled: F,
    ) -> io::Result<impl Read + Seek> {
        let file = codex_rollout::open_rollout_seekable_reader(path, is_cancelled);
        let Some(end) = self.cutoff(path) else {
            return file;
        };
        file.and_then(|mut file| {
            if file.seek(SeekFrom::End(0))? < end.end_byte_offset {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "cutoff byte offset is past the source rollout",
                ));
            }
            file.rewind()?;
            Ok(file)
        })
        .map_err(|err| {
            self.record_failure(path);
            io::Error::new(err.kind(), format!("{}: {err}", path.display()))
        })
    }
}

impl RolloutLineage {
    /// A failed ancestor read can select another copy. An alternate may name a different
    /// parent, so resolve its lineage before retrying the read.
    pub(in crate::local) async fn read_with_fallback<T, F>(
        self,
        store: &LocalThreadStore,
        read: impl Fn(Self) -> F,
    ) -> ThreadStoreResult<T>
    where
        F: Future<Output = ThreadStoreResult<T>> + Send,
    {
        let requested_thread_id = self.requested_thread_id;
        let mode = self.mode;
        let leaf = self
            .segments
            .last()
            .map(|segment| segment.rollout_path.clone());
        let mut excluded = HashMap::<ThreadId, FailedAncestor>::new();
        let mut lineage = self;
        loop {
            let reads = lineage.reads.clone();
            let error = match read(lineage).await {
                Ok(value) => return Ok(value),
                Err(error) => error,
            };
            let Some((path, (end, owner))) = reads.0.as_ref().and_then(|ancestors| {
                ancestors
                    .cutoffs
                    .get_key_value(ancestors.failed_path.get()?)
            }) else {
                return Err(error);
            };
            if let Some(original) = excluded.remove(&end.thread_id) {
                return Err(original.error);
            }
            let path = tokio::fs::canonicalize(path)
                .await
                .unwrap_or_else(|_| path.clone());
            excluded.insert(
                end.thread_id,
                FailedAncestor {
                    path,
                    owner: *owner,
                    error: error.clone(),
                },
            );
            lineage = store
                .resolve_rollout_lineage_with_mode(
                    requested_thread_id,
                    mode,
                    leaf.clone(),
                    &excluded,
                )
                .await
                .map_err(|_| error)?;
        }
    }
}

use crate::Stream;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::{DirEntry, ReadDir};

/// A wrapper around [`tokio::fs::ReadDir`] that implements [`Stream`].
///
/// [`tokio::fs::ReadDir`]: struct@tokio::fs::ReadDir
/// [`Stream`]: trait@crate::Stream
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub struct ReadDirStream {
    inner: ReadDir,
}

impl ReadDirStream {
    /// Create a new `ReadDirStream`.
    pub fn new(read_dir: ReadDir) -> Self {
        Self { inner: read_dir }
    }

    /// Get back the inner `ReadDir`.
    pub fn into_inner(self) -> ReadDir {
        self.inner
    }
}

impl Stream for ReadDirStream {
    type Item = io::Result<DirEntry>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_entry(cx).map(Result::transpose)
    }
}

impl AsRef<ReadDir> for ReadDirStream {
    fn as_ref(&self) -> &ReadDir {
        &self.inner
    }
}

impl AsMut<ReadDir> for ReadDirStream {
    fn as_mut(&mut self) -> &mut ReadDir {
        &mut self.inner
    }
}

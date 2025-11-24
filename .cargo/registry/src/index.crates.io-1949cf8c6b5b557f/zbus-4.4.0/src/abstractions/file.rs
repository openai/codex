//! Runtime-agnostic File I/O abstractions.
//!
//! Proving only specific API that we need internally.

#[cfg(unix)]
use std::fs::Metadata;
use std::{
    io::Result,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;

#[cfg(not(feature = "tokio"))]
#[derive(Debug)]
pub struct FileLines(futures_util::io::Lines<futures_util::io::BufReader<async_fs::File>>);
#[cfg(feature = "tokio")]
#[derive(Debug)]
pub struct FileLines(tokio::io::Lines<tokio::io::BufReader<tokio::fs::File>>);

impl FileLines {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(not(feature = "tokio"))]
        {
            async_fs::File::open(path)
                .await
                .map(futures_util::io::BufReader::new)
                .map(futures_util::AsyncBufReadExt::lines)
                .map(Self)
        }

        #[cfg(feature = "tokio")]
        {
            tokio::fs::File::open(path)
                .await
                .map(tokio::io::BufReader::new)
                .map(tokio::io::AsyncBufReadExt::lines)
                .map(Self)
        }
    }
}

impl Stream for FileLines {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        #[cfg(not(feature = "tokio"))]
        {
            Stream::poll_next(Pin::new(&mut self.get_mut().0), cx)
        }

        #[cfg(feature = "tokio")]
        {
            let fut = self.get_mut().0.next_line();
            futures_util::pin_mut!(fut);
            std::future::Future::poll(Pin::new(&mut fut), cx).map(Result::transpose)
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

// Not unix-specific itself but only used on unix.
#[cfg(unix)]
pub async fn metadata(path: impl AsRef<Path>) -> Result<Metadata> {
    #[cfg(not(feature = "tokio"))]
    {
        async_fs::metadata(path).await
    }

    #[cfg(feature = "tokio")]
    {
        tokio::fs::metadata(path).await
    }
}

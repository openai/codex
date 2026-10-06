//! Runs a rollout scan on one blocking worker, with cancellation and one reader observation.
//! Consumers may stop early; canceled reads do not contribute an EOF, failure, or duration.

use std::io;
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;
use std::time::Instant;

use super::BlockingLineReader;
use super::ReadFailureSource;
use super::ReadMetrics;
use super::RolloutLineReaderInner;
use super::open_rollout_line_reader;

/// Opens a fresh reader and processes its lines on the shared blocking worker.
pub(crate) async fn read_rollout_lines<T, F>(path: &Path, read: F) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut dyn Iterator<Item = io::Result<String>>) -> io::Result<T> + Send + 'static,
{
    let reader = open_rollout_line_reader(path).await?;
    let lines: BlockingLineReader = match reader.inner {
        RolloutLineReaderInner::Plain(lines) => {
            // The fresh reader has no buffered data to preserve when converting the file.
            let file = lines.into_inner().into_inner().into_std().await;
            io::BufReader::new(Box::new(file) as Box<dyn io::Read + Send>).lines()
        }
        RolloutLineReaderInner::Blocking(Some(lines)) => lines,
        RolloutLineReaderInner::Blocking(None) => {
            return Err(io::Error::other("compressed rollout reader is busy"));
        }
    };
    scan_lines(lines, reader.metrics, read).await
}

/// Runs a consumer over measured lines, retaining metrics if the caller drops the future.
pub(super) async fn scan_lines<T, F>(
    lines: BlockingLineReader,
    metrics: ReadMetrics,
    scan: F,
) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut dyn Iterator<Item = io::Result<String>>) -> io::Result<T> + Send + 'static,
{
    // Include the initial queue wait, as the per-line reader does, but exclude consumer work.
    let queued_at = Instant::now();
    let metrics = Arc::new(Mutex::new(metrics));
    let worker_metrics = Arc::clone(&metrics);
    let (stop, keep_running) = tokio::sync::oneshot::channel::<()>();
    let span = tracing::Span::current();
    let task = tokio::task::spawn_blocking(move || {
        let queue_duration = queued_at.elapsed();
        span.in_scope(|| {
            let mut metrics = worker_metrics
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            scan(&mut MeasuredLines {
                lines,
                metrics: &mut metrics,
                stop,
                queue_duration,
            })
        })
    });
    let result = task.await.map_err(|err| {
        let err = io::Error::other(err);
        metrics
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failed("read", ReadFailureSource::TaskJoin, &err);
        err
    });
    drop(keep_running);
    result?
}

struct MeasuredLines<'a> {
    lines: BlockingLineReader,
    metrics: &'a mut ReadMetrics,
    stop: tokio::sync::oneshot::Sender<()>,
    queue_duration: Duration,
}

impl Iterator for MeasuredLines<'_> {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stop.is_closed() {
            return None;
        }
        let started_at = Instant::now();
        let line = self.lines.next();
        if self.stop.is_closed() {
            return None;
        }
        let metrics = &mut self.metrics;
        metrics.duration = metrics
            .duration
            .saturating_add(std::mem::take(&mut self.queue_duration))
            .saturating_add(started_at.elapsed());
        metrics.reached_eof = line.is_none();
        match &line {
            Some(Ok(_)) => metrics.read_any_line = true,
            Some(Err(err)) => metrics.failed("read", ReadFailureSource::Stream, err),
            None => {}
        }
        line
    }
}

#[cfg(test)]
#[path = "blocking_reader_tests.rs"]
mod tests;

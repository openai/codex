//! Exercises cancellation during reads and between records.

use std::io;
use std::io::BufRead;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::ReadMetrics;
use super::read_rollout_lines;
use super::scan_lines;

struct PausedRead {
    contents: io::Cursor<&'static [u8]>,
    started: Option<tokio::sync::oneshot::Sender<()>>,
    resume: std::sync::mpsc::Receiver<()>,
    finished: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Read for PausedRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(started) = self.started.take() {
            let _ = started.send(());
            self.resume.recv().map_err(io::Error::other)?;
        }
        self.contents.read(buffer)
    }
}

impl Drop for PausedRead {
    fn drop(&mut self) {
        if let Some(finished) = self.finished.take() {
            let _ = finished.send(());
        }
    }
}

#[tokio::test]
async fn cancellation_during_read_does_not_deliver_the_record() -> anyhow::Result<()> {
    let (started, waiting) = tokio::sync::oneshot::channel();
    let (resume, paused) = std::sync::mpsc::channel();
    let (finished, done) = tokio::sync::oneshot::channel();
    let reader = Box::new(PausedRead {
        contents: io::Cursor::new(b"first\n".as_slice()),
        started: Some(started),
        resume: paused,
        finished: Some(finished),
    }) as Box<dyn Read + Send>;
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let task = tokio::spawn(scan_lines(
        io::BufReader::new(reader).lines(),
        ReadMetrics::default(),
        move |lines| {
            for line in lines {
                line?;
                seen.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        },
    ));
    waiting.await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    resume.send(())?;
    done.await?;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn cancelling_full_scan_stops_before_the_next_record() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let path = home.path().join("rollout.jsonl");
    std::fs::write(&path, "first\nsecond\n")?;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (continue_tx, continue_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        read_rollout_lines(&path, move |lines| {
            assert_eq!(lines.next().transpose()?, Some("first".to_string()));
            let _ = started_tx.send(());
            continue_rx.recv().map_err(io::Error::other)?;
            let _ = finished_tx.send(lines.next().transpose());
            Ok(())
        })
        .await
    });
    started_rx.await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    continue_tx.send(())?;
    assert_eq!(finished_rx.await??, None);
    Ok(())
}

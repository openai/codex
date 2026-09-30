//! Exercises cancellation while the worker is blocked inside a read.

use std::io;
use std::io::BufRead;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;

use super::ReadMetrics;
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

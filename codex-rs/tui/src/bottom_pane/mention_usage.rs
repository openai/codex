//! Locally remembers a bounded window of confirmed `$` picker selections.
//!
//! The active composer updates its window immediately and carries it across compatible widget
//! replacements. Background workers serialize writes, and a bounded file-lock wait merges
//! selections made by separate TUI processes without indefinitely delaying shutdown.
//! Malformed history is treated as empty and replaced by the next persisted selection.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use codex_config::types::HistoryPersistence;
use codex_utils_path::write_atomically;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const HISTORY_LIMIT: usize = 50;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const FILE_NAME: &str = "mention_usage.json";
const LOCK_TIMEOUT: Duration = Duration::from_millis(100);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(10);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(PartialEq)]
struct Storage {
    path: PathBuf,
    persistence: HistoryPersistence,
}

#[derive(Default)]
pub(crate) struct MentionUsage {
    recent: VecDeque<String>,
    storage: Option<Storage>,
    writer: Option<mpsc::Sender<String>>,
    workers: Vec<JoinHandle<()>>,
}

impl MentionUsage {
    pub(super) fn load(codex_home: &Path, persistence: HistoryPersistence) -> Self {
        let path = codex_home.join(FILE_NAME);
        let recent = match persistence {
            HistoryPersistence::SaveAll => read_recent(&path).unwrap_or_else(|error| {
                tracing::debug!(%error, "could not load mention preferences");
                VecDeque::new()
            }),
            HistoryPersistence::None => VecDeque::new(),
        };
        Self {
            recent,
            storage: Some(Storage { path, persistence }),
            ..Self::default()
        }
    }

    pub(crate) fn inherit(&mut self, previous: &mut Self) {
        let mut retired = std::mem::take(previous);
        if self.storage == retired.storage {
            std::mem::swap(self, &mut retired);
        }
        // Close the retired queue, but keep its worker reachable for orderly shutdown.
        drop(retired.writer.take());
        self.workers.append(&mut retired.workers);
        self.workers.retain(|worker| !worker.is_finished());
    }

    pub(crate) async fn flush(&mut self) {
        drop(self.writer.take());
        let mut workers = std::mem::take(&mut self.workers);
        let drain = async {
            for worker in &mut workers {
                if let Err(error) = worker.await {
                    tracing::debug!(%error, "mention preference writer stopped");
                }
            }
        };
        if tokio::time::timeout(FLUSH_TIMEOUT, drain).await.is_err() {
            for worker in workers {
                worker.abort();
            }
            tracing::debug!("timed out saving mention preferences");
        }
    }

    pub(super) fn frequencies(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for identity in &self.recent {
            *counts.entry(identity.clone()).or_default() += 1;
        }
        counts
    }

    pub(super) fn record(&mut self, identity: &str) {
        append_recent(&mut self.recent, identity.to_owned());
        let Some(Storage {
            path: file_path,
            persistence: HistoryPersistence::SaveAll,
        }) = self.storage.as_ref()
        else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let writer = self.writer.get_or_insert_with(|| {
            let (sender, mut receiver) = mpsc::channel::<String>(HISTORY_LIMIT);
            let file_path = file_path.clone();
            self.workers.push(runtime.spawn(async move {
                while let Some(identity) = receiver.recv().await {
                    let file_path = file_path.clone();
                    match tokio::task::spawn_blocking(move || {
                        persist_selection(&file_path, identity)
                    })
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            tracing::debug!(%error, "could not save mention preferences");
                        }
                        Err(error) => {
                            tracing::debug!(%error, "mention preference writer stopped");
                        }
                    }
                }
            }));
            sender
        });
        if writer.try_send(identity.to_owned()).is_err() {
            tracing::debug!("could not queue mention preference update");
        }
    }
}

fn append_recent(recent: &mut VecDeque<String>, identity: String) {
    if recent.len() == HISTORY_LIMIT {
        recent.pop_front();
    }
    recent.push_back(identity);
}

fn read_recent(path: &Path) -> io::Result<VecDeque<String>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(VecDeque::new()),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() > MAX_FILE_BYTES {
        return Err(io::Error::other("mention preferences exceed size limit"));
    }
    let mut recent: VecDeque<String> = match serde_json::from_reader(file) {
        Ok(recent) => recent,
        Err(error) if error.is_io() => return Err(error.into()),
        Err(_) => VecDeque::new(),
    };
    recent.drain(..recent.len().saturating_sub(HISTORY_LIMIT));
    Ok(recent)
}

fn persist_selection(path: &Path, identity: String) -> io::Result<()> {
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path.with_extension("lock"))?;
    let deadline = Instant::now() + LOCK_TIMEOUT;
    loop {
        match lock_file.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(LOCK_RETRY_INTERVAL);
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut recent = read_recent(path)?;
    append_recent(&mut recent, identity);
    let contents = serde_json::to_string(&recent)?;
    if contents.len() as u64 > MAX_FILE_BYTES {
        return Err(io::Error::other("mention preferences exceed size limit"));
    }
    write_atomically(path, &contents)
}

#[cfg(test)]
#[path = "mention_usage_tests.rs"]
mod tests;

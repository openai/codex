//! Candidate discovery coverage shared by flat and date-partitioned rollout layouts.

use super::*;
use crate::list::ThreadListLayout;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;

#[tokio::test]
async fn dropping_scan_stops_directory_enumeration() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    for name in ["first", "second"] {
        File::create(root.path().join(name))?;
    }
    let path = root.path().to_path_buf();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let visited = Arc::new(AtomicUsize::new(0));
    let worker_visited = Arc::clone(&visited);
    let task = tokio::spawn(run_scan(move |scan| {
        let mut started_tx = Some(started_tx);
        let result = collect_files(
            &path,
            |name, _| {
                worker_visited.fetch_add(1, Ordering::SeqCst);
                if let Some(started_tx) = started_tx.take() {
                    let _ = started_tx.send(());
                    resume_rx.recv().expect("resume scan");
                }
                Some(name.to_string())
            },
            scan,
        );
        let _ = finished_tx.send(result.as_ref().err().map(io::Error::kind));
        result
    }));
    started_rx.await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    resume_tx.send(())?;
    assert_eq!(
        (finished_rx.await?, visited.load(Ordering::SeqCst)),
        (Some(io::ErrorKind::Interrupted), 1),
    );
    Ok(())
}

#[tokio::test]
async fn updated_candidates_preserve_layout_and_plain_sibling_preference() -> io::Result<()> {
    for layout in [ThreadListLayout::Flat, ThreadListLayout::NestedByDate] {
        let root = TempDir::new()?;
        let directory = match layout {
            ThreadListLayout::Flat => root.path().to_path_buf(),
            ThreadListLayout::NestedByDate => root.path().join("2026/09/26"),
        };
        std::fs::create_dir_all(&directory)?;
        let id = Uuid::from_u128(1);
        let path = directory.join(format!("rollout-2026-09-26T10-00-00-{id}.jsonl"));
        File::create(&path)?.set_modified(UNIX_EPOCH + Duration::from_secs(100))?;
        File::create(compression::compressed_rollout_path(&path))?
            .set_modified(UNIX_EPOCH + Duration::from_secs(200))?;
        File::create(directory.join("unrelated.jsonl"))?;
        std::fs::create_dir(directory.join("rollout-2026-09-26T10-00-01-invalid.jsonl"))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            &path,
            directory.join(format!(
                "rollout-2026-09-26T10-00-02-{}.jsonl",
                Uuid::from_u128(2)
            )),
        )?;

        let mut scanned = 0;
        let candidates = match layout {
            ThreadListLayout::Flat => {
                collect_flat_files_by_updated_at(root.path(), &mut scanned).await?
            }
            ThreadListLayout::NestedByDate => {
                collect_files_by_updated_at(root.path(), &mut scanned).await?
            }
        };
        assert_eq!(
            (
                scanned,
                candidates
                    .into_iter()
                    .map(|candidate| (candidate.id, candidate.path, candidate.updated_at))
                    .collect::<Vec<_>>()
            ),
            (
                1,
                vec![(
                    id,
                    path,
                    Some(OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(100))
                )]
            ),
        );
    }
    Ok(())
}

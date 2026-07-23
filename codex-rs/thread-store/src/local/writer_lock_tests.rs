use std::fs;
use std::sync::Arc;

use codex_protocol::ThreadId;
use tempfile::TempDir;

use super::COORDINATION_LOCK_FILE;
use super::WRITER_LOCK_DIR;
use super::WriterLockCoordinator;
use crate::ThreadStoreError;

#[test]
fn writer_locks_reject_competing_owners_and_release_their_files() {
    let home = TempDir::new().expect("temp dir");
    let primary = Arc::new(WriterLockCoordinator::new(home.path()));
    let secondary = Arc::new(WriterLockCoordinator::new(home.path()));
    let thread_id = ThreadId::default();
    let other_thread_id = ThreadId::default();

    let owner = primary.acquire(thread_id).expect("acquire writer lock");
    let lock_path = home
        .path()
        .join(WRITER_LOCK_DIR)
        .join(format!("{thread_id}.lock"));
    assert!(lock_path.exists());

    let err = match secondary.acquire(thread_id) {
        Ok(_) => panic!("competing owner should fail"),
        Err(err) => err,
    };
    assert!(matches!(err, ThreadStoreError::Conflict { .. }));
    let other_owner = secondary
        .acquire(other_thread_id)
        .expect("other thread should acquire its own lock");

    drop(owner);
    assert!(!lock_path.exists());
    let next_owner = secondary
        .acquire(thread_id)
        .expect("released thread should accept another owner");
    drop(next_owner);
    drop(other_owner);

    let entries = fs::read_dir(home.path().join(WRITER_LOCK_DIR))
        .expect("read lock directory")
        .map(|entry| entry.expect("lock directory entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![COORDINATION_LOCK_FILE]);
}

#[test]
fn first_acquisition_removes_stale_locks_without_removing_active_locks() {
    let home = TempDir::new().expect("temp dir");
    let primary = Arc::new(WriterLockCoordinator::new(home.path()));
    let active_thread_id = ThreadId::default();
    let active_owner = primary
        .acquire(active_thread_id)
        .expect("acquire active writer lock");

    let stale_thread_id = ThreadId::default();
    let stale_path = home
        .path()
        .join(WRITER_LOCK_DIR)
        .join(format!("{stale_thread_id}.lock"));
    fs::File::create(&stale_path).expect("create stale writer lock");

    let secondary = Arc::new(WriterLockCoordinator::new(home.path()));
    let secondary_owner = secondary
        .acquire(ThreadId::default())
        .expect("acquire writer lock after cleanup");

    assert!(!stale_path.exists());
    let err = match secondary.acquire(active_thread_id) {
        Ok(_) => panic!("active writer should survive cleanup"),
        Err(err) => err,
    };
    assert!(matches!(err, ThreadStoreError::Conflict { .. }));

    drop(secondary_owner);
    drop(active_owner);
}

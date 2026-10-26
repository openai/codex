//! Tests maintenance ownership across shared guards and duplicated file handles.

use std::io;
use std::sync::Arc;

use tempfile::TempDir;

use super::try_acquire_rollout_maintenance_lock;

#[test]
fn dropping_last_guard_releases_lock_with_duplicated_handle() -> io::Result<()> {
    let home = TempDir::new()?;
    let guard = Arc::new(
        try_acquire_rollout_maintenance_lock(home.path())?.expect("acquire maintenance lock"),
    );
    let retained_guard = Arc::clone(&guard);
    let duplicated_file = guard.file.try_clone()?;

    assert!(try_acquire_rollout_maintenance_lock(home.path())?.is_none());
    drop(guard);
    assert!(try_acquire_rollout_maintenance_lock(home.path())?.is_none());

    drop(retained_guard);
    let next_guard = try_acquire_rollout_maintenance_lock(home.path())?
        .expect("dropping the last guard releases the lock despite the duplicated handle");

    drop(duplicated_file);
    assert!(try_acquire_rollout_maintenance_lock(home.path())?.is_none());
    drop(next_guard);
    assert!(try_acquire_rollout_maintenance_lock(home.path())?.is_some());
    Ok(())
}

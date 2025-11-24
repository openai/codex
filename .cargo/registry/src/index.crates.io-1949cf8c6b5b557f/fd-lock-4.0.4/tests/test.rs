use fd_lock::RwLock;
use std::fs::File;
use std::io::ErrorKind;

use tempfile::tempdir;

#[test]
fn double_read_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lockfile");

    let l0 = RwLock::new(File::create(&path).unwrap());
    let l1 = RwLock::new(File::open(path).unwrap());

    let _g0 = l0.try_read().unwrap();
    let _g1 = l1.try_read().unwrap();
}

#[test]
fn double_write_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lockfile");

    let mut l0 = RwLock::new(File::create(&path).unwrap());
    let mut l1 = RwLock::new(File::open(path).unwrap());

    let g0 = l0.try_write().unwrap();

    let err = l1.try_write().unwrap_err();
    assert!(matches!(err.kind(), ErrorKind::WouldBlock));

    drop(g0);
}

#[test]
fn read_and_write_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lockfile");

    let l0 = RwLock::new(File::create(&path).unwrap());
    let mut l1 = RwLock::new(File::open(path).unwrap());

    let g0 = l0.try_read().unwrap();

    let err = l1.try_write().unwrap_err();
    assert!(matches!(err.kind(), ErrorKind::WouldBlock));

    drop(g0);
}

#[test]
fn write_and_read_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lockfile");

    let mut l0 = RwLock::new(File::create(&path).unwrap());
    let l1 = RwLock::new(File::open(path).unwrap());

    let g0 = l0.try_write().unwrap();

    let err = l1.try_read().unwrap_err();
    assert!(matches!(err.kind(), ErrorKind::WouldBlock));

    drop(g0);
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn try_lock_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lockfile");

        // On Windows, opening with an access_mode as 0 will prevent all locking operations from succeeding, simulating an I/O error.
        let mut l0 = RwLock::new(
            File::options()
                .create(true)
                .read(true)
                .write(true)
                .access_mode(0)
                .open(path)
                .unwrap(),
        );

        let err1 = l0.try_read().unwrap_err();
        assert!(matches!(err1.kind(), ErrorKind::PermissionDenied));

        let err2 = l0.try_write().unwrap_err();
        assert!(matches!(err2.kind(), ErrorKind::PermissionDenied));
    }
}

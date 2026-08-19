use super::verify_fd_mounts;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::os::fd::IntoRawFd;

/// A matching mount authenticates its original inode and closes the inherited descriptor.
#[test]
fn matching_mount_closes_inherited_descriptor() {
    let root = tempfile::tempdir().expect("temporary directory should be created");
    let descriptor = File::open(root.path())
        .expect("directory descriptor should open")
        .into_raw_fd();
    let marker = format!("{descriptor}:{}", root.path().display());

    verify_fd_mounts(&[marker]).expect("matching mount should verify");

    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

/// A swapped destination fails closed without leaking the original descriptor.
#[test]
fn mismatched_mount_closes_inherited_descriptor() {
    let source = tempfile::tempdir().expect("source directory should be created");
    let destination = tempfile::tempdir().expect("destination directory should be created");
    let descriptor = File::open(source.path())
        .expect("source directory descriptor should open")
        .into_raw_fd();
    let marker = format!("{descriptor}:{}", destination.path().display());

    let error = verify_fd_mounts(&[marker]).expect_err("different inodes must be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

/// A symlink to the original inode is not itself the authenticated mount.
#[test]
fn symlinked_mount_destination_closes_inherited_descriptor() {
    let root = tempfile::tempdir().expect("temporary directory should be created");
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    std::fs::create_dir(&source).expect("source directory should be created");
    std::os::unix::fs::symlink(&source, &destination)
        .expect("mount destination symlink should be created");
    let descriptor = File::open(&source)
        .expect("source directory descriptor should open")
        .into_raw_fd();
    let marker = format!("{descriptor}:{}", destination.display());

    let error = verify_fd_mounts(&[marker]).expect_err("symlinked mounts must be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

/// Malformed mount markers cannot claim standard streams or relative destinations.
#[test]
fn malformed_mount_markers_are_rejected() {
    for marker in [
        "missing-separator",
        "invalid:/tmp",
        "0:/tmp",
        "1:/tmp",
        "2:/tmp",
    ] {
        let error = verify_fd_mounts(&[marker.to_string()])
            .expect_err("malformed mount marker must be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    let root = tempfile::tempdir().expect("temporary directory should be created");
    let descriptor = File::open(root.path())
        .expect("directory descriptor should open")
        .into_raw_fd();
    let error = verify_fd_mounts(&[format!("{descriptor}:relative")])
        .expect_err("relative mount destinations must be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
}

/// A transferred descriptor can be consumed only once even if a marker is repeated.
#[test]
fn duplicate_mount_descriptors_are_rejected() {
    let root = tempfile::tempdir().expect("temporary directory should be created");
    let descriptor = File::open(root.path())
        .expect("directory descriptor should open")
        .into_raw_fd();
    let marker = format!("{descriptor}:{}", root.path().display());

    let error = verify_fd_mounts(&[marker.clone(), marker])
        .expect_err("a mount descriptor must not be consumed twice");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
}

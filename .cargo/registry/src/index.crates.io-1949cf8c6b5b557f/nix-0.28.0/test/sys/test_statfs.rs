use nix::sys::statfs::*;
use nix::sys::statvfs::*;
use std::fs::File;
use std::path::Path;

fn check_fstatfs(path: &str) {
    if !Path::new(path).exists() {
        return;
    }
    let vfs = statvfs(path.as_bytes()).unwrap();
    let file = File::open(path).unwrap();
    let fs = fstatfs(&file).unwrap();
    assert_fs_equals(fs, vfs);
}

fn check_statfs(path: &str) {
    if !Path::new(path).exists() {
        return;
    }
    let vfs = statvfs(path.as_bytes()).unwrap();
    let fs = statfs(path.as_bytes()).unwrap();
    assert_fs_equals(fs, vfs);
}

fn check_fstatfs_strict(path: &str) {
    if !Path::new(path).exists() {
        return;
    }
    let vfs = statvfs(path.as_bytes());
    let file = File::open(path).unwrap();
    let fs = fstatfs(&file);
    assert_fs_equals_strict(fs.unwrap(), vfs.unwrap())
}

fn check_statfs_strict(path: &str) {
    if !Path::new(path).exists() {
        return;
    }
    let vfs = statvfs(path.as_bytes());
    let fs = statfs(path.as_bytes());
    assert_fs_equals_strict(fs.unwrap(), vfs.unwrap())
}

// The cast is not unnecessary on all platforms.
#[allow(clippy::unnecessary_cast)]
fn assert_fs_equals(fs: Statfs, vfs: Statvfs) {
    assert_eq!(fs.files() as u64, vfs.files() as u64);
    assert_eq!(fs.blocks() as u64, vfs.blocks() as u64);
    assert_eq!(fs.block_size() as u64, vfs.fragment_size() as u64);
}

#[test]
fn statfs_call() {
    check_statfs("/tmp");
    check_statfs("/dev");
    check_statfs("/run");
    check_statfs("/");
}

#[test]
fn fstatfs_call() {
    check_fstatfs("/tmp");
    check_fstatfs("/dev");
    check_fstatfs("/run");
    check_fstatfs("/");
}

// This test is ignored because files_free/blocks_free can change after statvfs call and before
// statfs call.
#[test]
#[ignore]
fn statfs_call_strict() {
    check_statfs_strict("/tmp");
    check_statfs_strict("/dev");
    check_statfs_strict("/run");
    check_statfs_strict("/");
}

// This test is ignored because files_free/blocks_free can change after statvfs call and before
// fstatfs call.
#[test]
#[ignore]
fn fstatfs_call_strict() {
    check_fstatfs_strict("/tmp");
    check_fstatfs_strict("/dev");
    check_fstatfs_strict("/run");
    check_fstatfs_strict("/");
}

// The cast is not unnecessary on all platforms.
#[allow(clippy::unnecessary_cast)]
fn assert_fs_equals_strict(fs: Statfs, vfs: Statvfs) {
    assert_eq!(fs.files_free() as u64, vfs.files_free() as u64);
    assert_eq!(fs.blocks_free() as u64, vfs.blocks_free() as u64);
    assert_eq!(fs.blocks_available() as u64, vfs.blocks_available() as u64);
    assert_eq!(fs.files() as u64, vfs.files() as u64);
    assert_eq!(fs.blocks() as u64, vfs.blocks() as u64);
    assert_eq!(fs.block_size() as u64, vfs.fragment_size() as u64);
}

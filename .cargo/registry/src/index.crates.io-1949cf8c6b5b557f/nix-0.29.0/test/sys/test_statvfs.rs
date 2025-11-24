use nix::sys::statvfs::*;
use std::fs::File;

#[test]
fn statvfs_call() {
    statvfs(&b"/"[..]).unwrap();
}

#[test]
fn fstatvfs_call() {
    let root = File::open("/").unwrap();
    fstatvfs(&root).unwrap();
}

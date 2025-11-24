use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use libc::{EACCES, EROFS};

use nix::mount::{mount, umount, MsFlags};
use nix::sys::stat::{self, Mode};

use crate::*;

static SCRIPT_CONTENTS: &[u8] = b"#!/bin/sh
exit 23";

const EXPECTED_STATUS: i32 = 23;

const NONE: Option<&'static [u8]> = None;

#[test]
fn test_mount_tmpfs_without_flags_allows_rwx() {
    require_capability!(
        "test_mount_tmpfs_without_flags_allows_rwx",
        CAP_SYS_ADMIN
    );
    let tempdir = tempfile::tempdir().unwrap();

    mount(
        NONE,
        tempdir.path(),
        Some(b"tmpfs".as_ref()),
        MsFlags::empty(),
        NONE,
    )
    .unwrap_or_else(|e| panic!("mount failed: {e}"));

    let test_path = tempdir.path().join("test");

    // Verify write.
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode((Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO).bits())
        .open(&test_path)
        .and_then(|mut f| f.write(SCRIPT_CONTENTS))
        .unwrap_or_else(|e| panic!("write failed: {e}"));

    // Verify read.
    let mut buf = Vec::new();
    File::open(&test_path)
        .and_then(|mut f| f.read_to_end(&mut buf))
        .unwrap_or_else(|e| panic!("read failed: {e}"));
    assert_eq!(buf, SCRIPT_CONTENTS);

    // while forking and unmounting prevent other child processes
    let _m = FORK_MTX.lock();
    // Verify execute.
    assert_eq!(
        EXPECTED_STATUS,
        Command::new(&test_path)
            .status()
            .unwrap_or_else(|e| panic!("exec failed: {e}"))
            .code()
            .unwrap_or_else(|| panic!("child killed by signal"))
    );

    umount(tempdir.path()).unwrap_or_else(|e| panic!("umount failed: {e}"));
}

#[test]
fn test_mount_rdonly_disallows_write() {
    require_capability!("test_mount_rdonly_disallows_write", CAP_SYS_ADMIN);
    let tempdir = tempfile::tempdir().unwrap();

    mount(
        NONE,
        tempdir.path(),
        Some(b"tmpfs".as_ref()),
        MsFlags::MS_RDONLY,
        NONE,
    )
    .unwrap_or_else(|e| panic!("mount failed: {e}"));

    // EROFS: Read-only file system
    assert_eq!(
        EROFS,
        File::create(tempdir.path().join("test"))
            .unwrap_err()
            .raw_os_error()
            .unwrap()
    );

    umount(tempdir.path()).unwrap_or_else(|e| panic!("umount failed: {e}"));
}

#[test]
fn test_mount_noexec_disallows_exec() {
    require_capability!("test_mount_noexec_disallows_exec", CAP_SYS_ADMIN);
    let tempdir = tempfile::tempdir().unwrap();

    mount(
        NONE,
        tempdir.path(),
        Some(b"tmpfs".as_ref()),
        MsFlags::MS_NOEXEC,
        NONE,
    )
    .unwrap_or_else(|e| panic!("mount failed: {e}"));

    let test_path = tempdir.path().join("test");

    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode((Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO).bits())
        .open(&test_path)
        .and_then(|mut f| f.write(SCRIPT_CONTENTS))
        .unwrap_or_else(|e| panic!("write failed: {e}"));

    // Verify that we cannot execute despite a+x permissions being set.
    let mode = stat::Mode::from_bits_truncate(
        fs::metadata(&test_path)
            .map(|md| md.permissions().mode())
            .unwrap_or_else(|e| panic!("metadata failed: {e}")),
    );

    assert!(
        mode.contains(Mode::S_IXUSR | Mode::S_IXGRP | Mode::S_IXOTH),
        "{:?} did not have execute permissions",
        &test_path
    );

    // while forking and unmounting prevent other child processes
    let _m = FORK_MTX.lock();
    // EACCES: Permission denied
    assert_eq!(
        EACCES,
        Command::new(&test_path)
            .status()
            .unwrap_err()
            .raw_os_error()
            .unwrap()
    );

    umount(tempdir.path()).unwrap_or_else(|e| panic!("umount failed: {e}"));
}

#[test]
fn test_mount_bind() {
    require_capability!("test_mount_bind", CAP_SYS_ADMIN);
    let tempdir = tempfile::tempdir().unwrap();
    let file_name = "test";

    {
        let mount_point = tempfile::tempdir().unwrap();

        mount(
            Some(tempdir.path()),
            mount_point.path(),
            NONE,
            MsFlags::MS_BIND,
            NONE,
        )
        .unwrap_or_else(|e| panic!("mount failed: {e}"));

        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode((Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO).bits())
            .open(mount_point.path().join(file_name))
            .and_then(|mut f| f.write(SCRIPT_CONTENTS))
            .unwrap_or_else(|e| panic!("write failed: {e}"));

        // wait for child processes to prevent EBUSY
        let _m = FORK_MTX.lock();
        umount(mount_point.path())
            .unwrap_or_else(|e| panic!("umount failed: {e}"));
    }

    // Verify the file written in the mount shows up in source directory, even
    // after unmounting.

    let mut buf = Vec::new();
    File::open(tempdir.path().join(file_name))
        .and_then(|mut f| f.read_to_end(&mut buf))
        .unwrap_or_else(|e| panic!("read failed: {e}"));
    assert_eq!(buf, SCRIPT_CONTENTS);
}

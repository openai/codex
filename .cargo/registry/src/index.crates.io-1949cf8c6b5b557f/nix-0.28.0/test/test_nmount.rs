use crate::*;
use nix::{
    errno::Errno,
    mount::{unmount, MntFlags, Nmount},
};
use std::{ffi::CString, fs::File, path::Path};
use tempfile::tempdir;

#[test]
fn ok() {
    require_mount!("nullfs");

    let mountpoint = tempdir().unwrap();
    let target = tempdir().unwrap();
    let _sentry = File::create(target.path().join("sentry")).unwrap();

    let fstype = CString::new("fstype").unwrap();
    let nullfs = CString::new("nullfs").unwrap();
    Nmount::new()
        .str_opt(&fstype, &nullfs)
        .str_opt_owned("fspath", mountpoint.path().to_str().unwrap())
        .str_opt_owned("target", target.path().to_str().unwrap())
        .nmount(MntFlags::empty())
        .unwrap();

    // Now check that the sentry is visible through the mountpoint
    let exists = Path::exists(&mountpoint.path().join("sentry"));

    // Cleanup the mountpoint before asserting
    unmount(mountpoint.path(), MntFlags::empty()).unwrap();

    assert!(exists);
}

#[test]
fn bad_fstype() {
    let mountpoint = tempdir().unwrap();
    let target = tempdir().unwrap();
    let _sentry = File::create(target.path().join("sentry")).unwrap();

    let e = Nmount::new()
        .str_opt_owned("fspath", mountpoint.path().to_str().unwrap())
        .str_opt_owned("target", target.path().to_str().unwrap())
        .nmount(MntFlags::empty())
        .unwrap_err();

    assert_eq!(e.error(), Errno::EINVAL);
    assert_eq!(e.errmsg(), Some("Invalid fstype"));
}

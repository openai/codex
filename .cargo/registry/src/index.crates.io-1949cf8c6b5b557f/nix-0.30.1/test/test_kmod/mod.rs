use crate::*;
use std::fs::copy;
use std::path::PathBuf;
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn compile_kernel_module() -> (PathBuf, String, TempDir) {
    let _m = crate::FORK_MTX.lock();

    let tmp_dir =
        tempdir().expect("unable to create temporary build directory");

    copy(
        "test/test_kmod/hello_mod/hello.c",
        tmp_dir.path().join("hello.c"),
    )
    .expect("unable to copy hello.c to temporary build directory");
    copy(
        "test/test_kmod/hello_mod/Makefile",
        tmp_dir.path().join("Makefile"),
    )
    .expect("unable to copy Makefile to temporary build directory");

    let status = Command::new("make")
        .current_dir(tmp_dir.path())
        .status()
        .expect("failed to run make");

    assert!(status.success());

    // Return the relative path of the build kernel module
    (tmp_dir.path().join("hello.ko"), "hello".to_owned(), tmp_dir)
}

use nix::errno::Errno;
use nix::kmod::{delete_module, DeleteModuleFlags};
use nix::kmod::{finit_module, init_module, ModuleInitFlags};
use std::ffi::CString;
use std::fs::File;
use std::io::Read;

#[test]
fn test_finit_and_delete_module() {
    require_capability!("test_finit_and_delete_module", CAP_SYS_MODULE);
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let (kmod_path, kmod_name, _kmod_dir) = compile_kernel_module();

    let f = File::open(kmod_path).expect("unable to open kernel module");
    finit_module(&f, &CString::new("").unwrap(), ModuleInitFlags::empty())
        .expect("unable to load kernel module");

    delete_module(
        &CString::new(kmod_name).unwrap(),
        DeleteModuleFlags::empty(),
    )
    .expect("unable to unload kernel module");
}

#[test]
fn test_finit_and_delete_module_with_params() {
    require_capability!(
        "test_finit_and_delete_module_with_params",
        CAP_SYS_MODULE
    );
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let (kmod_path, kmod_name, _kmod_dir) = compile_kernel_module();

    let f = File::open(kmod_path).expect("unable to open kernel module");
    finit_module(
        &f,
        &CString::new("who=Rust number=2018").unwrap(),
        ModuleInitFlags::empty(),
    )
    .expect("unable to load kernel module");

    delete_module(
        &CString::new(kmod_name).unwrap(),
        DeleteModuleFlags::empty(),
    )
    .expect("unable to unload kernel module");
}

#[test]
fn test_init_and_delete_module() {
    require_capability!("test_init_and_delete_module", CAP_SYS_MODULE);
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let (kmod_path, kmod_name, _kmod_dir) = compile_kernel_module();

    let mut f = File::open(kmod_path).expect("unable to open kernel module");
    let mut contents: Vec<u8> = Vec::new();
    f.read_to_end(&mut contents)
        .expect("unable to read kernel module content to buffer");
    init_module(&contents, &CString::new("").unwrap())
        .expect("unable to load kernel module");

    delete_module(
        &CString::new(kmod_name).unwrap(),
        DeleteModuleFlags::empty(),
    )
    .expect("unable to unload kernel module");
}

#[test]
fn test_init_and_delete_module_with_params() {
    require_capability!(
        "test_init_and_delete_module_with_params",
        CAP_SYS_MODULE
    );
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let (kmod_path, kmod_name, _kmod_dir) = compile_kernel_module();

    let mut f = File::open(kmod_path).expect("unable to open kernel module");
    let mut contents: Vec<u8> = Vec::new();
    f.read_to_end(&mut contents)
        .expect("unable to read kernel module content to buffer");
    init_module(&contents, &CString::new("who=Nix number=2015").unwrap())
        .expect("unable to load kernel module");

    delete_module(
        &CString::new(kmod_name).unwrap(),
        DeleteModuleFlags::empty(),
    )
    .expect("unable to unload kernel module");
}

#[test]
fn test_finit_module_invalid() {
    require_capability!("test_finit_module_invalid", CAP_SYS_MODULE);
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let kmod_path = "/dev/zero";

    let f = File::open(kmod_path).expect("unable to open kernel module");
    let result =
        finit_module(&f, &CString::new("").unwrap(), ModuleInitFlags::empty());

    assert_eq!(result.unwrap_err(), Errno::EINVAL);
}

#[test]
fn test_finit_module_twice_and_delete_module() {
    require_capability!(
        "test_finit_module_twice_and_delete_module",
        CAP_SYS_MODULE
    );
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let (kmod_path, kmod_name, _kmod_dir) = compile_kernel_module();

    let f = File::open(kmod_path).expect("unable to open kernel module");
    finit_module(&f, &CString::new("").unwrap(), ModuleInitFlags::empty())
        .expect("unable to load kernel module");

    let result =
        finit_module(&f, &CString::new("").unwrap(), ModuleInitFlags::empty());

    assert_eq!(result.unwrap_err(), Errno::EEXIST);

    delete_module(
        &CString::new(kmod_name).unwrap(),
        DeleteModuleFlags::empty(),
    )
    .expect("unable to unload kernel module");
}

#[test]
fn test_delete_module_not_loaded() {
    require_capability!("test_delete_module_not_loaded", CAP_SYS_MODULE);
    let _m0 = crate::KMOD_MTX.lock();
    let _m1 = crate::CWD_LOCK.read();

    let result = delete_module(
        &CString::new("hello").unwrap(),
        DeleteModuleFlags::empty(),
    );

    assert_eq!(result.unwrap_err(), Errno::ENOENT);
}

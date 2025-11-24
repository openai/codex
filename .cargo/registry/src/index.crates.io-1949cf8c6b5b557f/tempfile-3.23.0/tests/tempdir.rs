// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(rust_2018_idioms)]

use std::fs;
use std::path::Path;

use tempfile::{Builder, TempDir};

/// For the wasi platforms, `std::env::temp_dir` will panic. For those targets, configure the /tmp
/// directory instead as the base directory for temp files.
fn configure_wasi_temp_dir() {
    if cfg!(target_os = "wasi") {
        let _ = tempfile::env::override_temp_dir(std::path::Path::new("/tmp"));
    }
}

#[test]
fn test_tempdir() {
    configure_wasi_temp_dir();

    let path = {
        let p = Builder::new().prefix("foobar").tempdir().unwrap();
        let p = p.path();
        assert!(p.to_str().unwrap().contains("foobar"));
        p.to_path_buf()
    };
    assert!(!path.exists());
}

#[test]
fn test_prefix() {
    configure_wasi_temp_dir();

    let tmpfile = TempDir::with_prefix("prefix").unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("prefix"));
}

#[test]
fn test_suffix() {
    configure_wasi_temp_dir();

    let tmpfile = TempDir::with_suffix("suffix").unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.ends_with("suffix"));
}

#[test]
fn test_customnamed() {
    configure_wasi_temp_dir();

    let tmpfile = Builder::new()
        .prefix("prefix")
        .suffix("suffix")
        .rand_bytes(12)
        .tempdir()
        .unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("prefix"));
    assert!(name.ends_with("suffix"));
    assert_eq!(name.len(), 24);
}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "thread::spawn is not supported")]
fn test_rm_tempdir_threading() {
    configure_wasi_temp_dir();

    use std::sync::mpsc::channel;
    use std::thread;

    let (tx, rx) = channel();
    let f = move || {
        let tmp = TempDir::new().unwrap();
        tx.send(tmp.path().to_path_buf()).unwrap();
        panic!("panic to unwind past `tmp`");
    };
    let _ = thread::spawn(f).join();
    let path = rx.recv().unwrap();
    assert!(!path.exists());

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();
    let f = move || {
        let _tmp = tmp;
        panic!("panic to unwind past `tmp`");
    };
    let _ = thread::spawn(f).join();
    assert!(!path.exists());

    let path;
    {
        let f = move || TempDir::new().unwrap();

        let tmp = thread::spawn(f).join().unwrap();
        path = tmp.path().to_path_buf();
        assert!(path.exists());
    }
    assert!(!path.exists());
}

#[test]
fn test_tempdir_keep() {
    configure_wasi_temp_dir();

    let path = {
        let tmp = TempDir::new().unwrap();
        tmp.keep()
    };
    assert!(path.exists());
    fs::remove_dir_all(&path).unwrap();
    assert!(!path.exists());
}

#[test]
fn test_tmpdir_close() {
    configure_wasi_temp_dir();

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();
    assert!(path.exists());
    tmp.close().unwrap();
    assert!(!path.exists());
}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "unwinding is not supported")]
fn dont_double_panic() {
    configure_wasi_temp_dir();

    use std::thread;
    let r: Result<(), _> = thread::spawn(move || {
        let tmpdir = TempDir::new().unwrap();
        // Remove the temporary directory so that TempDir sees
        // an error on drop
        fs::remove_dir(tmpdir.path()).unwrap();
        // Panic. If TempDir panics *again* due to the rmdir
        // error then the process will abort.
        panic!();
    })
    .join();
    assert!(r.is_err());
}

#[test]
fn pass_as_asref_path() {
    configure_wasi_temp_dir();

    let tempdir = TempDir::new().unwrap();
    takes_asref_path(&tempdir);

    fn takes_asref_path<T: AsRef<Path>>(path: T) {
        let path = path.as_ref();
        assert!(path.exists());
    }
}

#[test]
fn test_disable_cleanup() {
    configure_wasi_temp_dir();

    // Case 0: never mark as "disable cleanup"
    // Case 1: enable "disable cleanup" in the builder, don't touch it after.
    // Case 2: enable "disable cleanup" in the builder, turn it off after.
    // Case 3: don't enable disable cleanup in the builder, turn it on after.
    for case in 0..4 {
        let in_builder = case & 1 > 0;
        let toggle = case & 2 > 0;
        let mut tmpdir = Builder::new()
            .disable_cleanup(in_builder)
            .tempdir()
            .unwrap();
        if toggle {
            tmpdir.disable_cleanup(!in_builder);
        }

        let path = tmpdir.path().to_owned();
        drop(tmpdir);

        if in_builder ^ toggle {
            assert!(path.exists());
            fs::remove_dir(path).unwrap();
        } else {
            assert!(!path.exists(), "tempdir wasn't deleted");
        }
    }
}

#![cfg_attr(target_family = "wasm", allow(unused))]

use std::{
    collections::hash_map::RandomState,
    fs::{remove_file, File, OpenOptions},
    hash::{BuildHasher, Hasher},
    io, os,
    path::{Path, PathBuf},
};

#[cfg(not(any(unix, target_family = "wasm", windows)))]
compile_error!("Your system is not supported since cc cannot create named tempfile");

fn rand() -> u64 {
    RandomState::new().build_hasher().finish()
}

fn tmpname(suffix: &str) -> String {
    format!("{}{}", rand(), suffix)
}

fn create_named(path: &Path) -> io::Result<File> {
    let mut open_options = OpenOptions::new();

    open_options.read(true).write(true).create_new(true);

    #[cfg(all(unix, not(target_os = "wasi")))]
    <OpenOptions as os::unix::fs::OpenOptionsExt>::mode(&mut open_options, 0o600);

    #[cfg(windows)]
    <OpenOptions as os::windows::fs::OpenOptionsExt>::custom_flags(
        &mut open_options,
        crate::windows::windows_sys::FILE_ATTRIBUTE_TEMPORARY,
    );

    open_options.open(path)
}

pub(super) struct NamedTempfile {
    path: PathBuf,
    file: Option<File>,
}

impl NamedTempfile {
    pub(super) fn new(base: &Path, suffix: &str) -> io::Result<Self> {
        for _ in 0..10 {
            let path = base.join(tmpname(suffix));
            match create_named(&path) {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(file),
                        path,
                    })
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            };
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "too many temporary files exist in base `{}` with suffix `{}`",
                base.display(),
                suffix
            ),
        ))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn take_file(&mut self) -> Option<File> {
        self.file.take()
    }
}

impl Drop for NamedTempfile {
    fn drop(&mut self) {
        // On Windows you have to close all handle to it before
        // removing the file.
        self.file.take();
        let _ = remove_file(&self.path);
    }
}

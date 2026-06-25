// Portions of this file are adapted from Rust's standard library.
// Copyright The Rust Project Developers. Licensed under Apache-2.0 or MIT.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;

use winapi::um::fileapi::GetFileAttributesW;
use winapi::um::fileapi::GetFullPathNameW;
use winapi::um::fileapi::INVALID_FILE_ATTRIBUTES;
use winapi::um::sysinfoapi::GetSystemDirectoryW;
use winapi::um::sysinfoapi::GetWindowsDirectoryW;

pub(super) fn current_directory(cwd: &Path) -> io::Result<Vec<u16>> {
    if !fs::metadata(cwd)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("current directory `{}` is not a directory", cwd.display()),
        ));
    }

    let directory = nul_terminated(cwd.as_os_str())?;
    strip_safe_verbatim_prefix(directory)
}

pub(super) fn program_exists(path: &Path) -> bool {
    let Ok(path) = to_user_path(path) else {
        return false;
    };
    unsafe { GetFileAttributesW(path.as_ptr()) != INVALID_FILE_ATTRIBUTES }
}

pub(super) fn system_directory() -> io::Result<PathBuf> {
    system_path(GetSystemDirectoryW)
}

pub(super) fn to_user_path(path: &Path) -> io::Result<Vec<u16>> {
    from_wide_to_user_path(nul_terminated(path.as_os_str())?)
}

pub(super) fn windows_directory() -> io::Result<PathBuf> {
    system_path(GetWindowsDirectoryW)
}

fn system_path(function: unsafe extern "system" fn(*mut u16, u32) -> u32) -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 260];
    loop {
        let length = unsafe { function(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        buffer.resize(length.saturating_add(1), 0);
    }
}

fn from_wide_to_user_path(path: Vec<u16>) -> io::Result<Vec<u16>> {
    let path = if path.len() <= 260 {
        strip_safe_verbatim_prefix(path)?
    } else {
        path
    };
    get_long_path(path, /*prefer_verbatim*/ false)
}

fn strip_safe_verbatim_prefix(mut path: Vec<u16>) -> io::Result<Vec<u16>> {
    const SEP: u16 = b'\\' as u16;
    const QUERY: u16 = b'?' as u16;
    const COLON: u16 = b':' as u16;
    const U: u16 = b'U' as u16;
    const N: u16 = b'N' as u16;
    const C: u16 = b'C' as u16;
    match path.as_slice() {
        [SEP, SEP, QUERY, SEP, _, COLON, SEP, ..] => {
            let candidate = path[4..].to_vec();
            if full_path_matches(&candidate)? {
                return Ok(candidate);
            }
        }
        [SEP, SEP, QUERY, SEP, U, N, C, SEP, ..] => {
            path[6] = SEP;
            let candidate = path[6..].to_vec();
            if full_path_matches(&candidate)? {
                return Ok(candidate);
            }
            path[6] = C;
        }
        _ => {}
    }
    Ok(path)
}

fn full_path_matches(candidate: &[u16]) -> io::Result<bool> {
    let expected = candidate.strip_suffix(&[0]).unwrap_or(candidate);
    Ok(get_full_path_name(candidate)? == expected)
}

fn get_long_path(path: Vec<u16>, prefer_verbatim: bool) -> io::Result<Vec<u16>> {
    const SEP: u16 = b'\\' as u16;
    const QUERY: u16 = b'?' as u16;
    const DOT: u16 = b'.' as u16;
    const COLON: u16 = b':' as u16;
    const LEGACY_MAX_PATH: usize = 248;

    if path == [0]
        || path.starts_with(&[SEP, SEP, QUERY, SEP])
        || path.starts_with(&[SEP, QUERY, QUERY, SEP])
    {
        return Ok(path);
    }
    let is_separator = |unit| unit == SEP || unit == b'/' as u16;
    let is_drive_absolute =
        path.get(1) == Some(&COLON) && path.get(2).is_some_and(|unit| is_separator(*unit));
    let is_unc = path.first().is_some_and(|unit| is_separator(*unit))
        && path.get(1).is_some_and(|unit| is_separator(*unit));
    if path.len() < LEGACY_MAX_PATH && (is_drive_absolute || is_unc) {
        return Ok(path);
    }

    let absolute = get_full_path_name(&path)?;
    if !prefer_verbatim && absolute.len() + 1 < LEGACY_MAX_PATH {
        return Ok(absolute.into_iter().chain([0]).collect());
    }

    let mut verbatim = Vec::with_capacity(absolute.len().saturating_add(8));
    verbatim.extend([SEP, SEP, QUERY, SEP]);
    match absolute.as_slice() {
        [_, COLON, SEP, ..] => verbatim.extend_from_slice(&absolute),
        [SEP, SEP, DOT, SEP, rest @ ..] => verbatim.extend_from_slice(rest),
        [SEP, SEP, rest @ ..] => {
            verbatim.extend([b'U' as u16, b'N' as u16, b'C' as u16, SEP]);
            verbatim.extend_from_slice(rest);
        }
        _ => return Ok(absolute.into_iter().chain([0]).collect()),
    }
    verbatim.push(0);
    Ok(verbatim)
}

fn get_full_path_name(path: &[u16]) -> io::Result<Vec<u16>> {
    let mut buffer = vec![0u16; 260];
    loop {
        let length = unsafe {
            GetFullPathNameW(
                path.as_ptr(),
                buffer.len() as u32,
                buffer.as_mut_ptr(),
                ptr::null_mut(),
            )
        } as usize;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(buffer);
        }
        buffer.resize(length.saturating_add(1), 0);
    }
}

fn nul_terminated(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = value.encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "value contains a NUL character",
        ));
    }
    wide.push(0);
    Ok(wide)
}

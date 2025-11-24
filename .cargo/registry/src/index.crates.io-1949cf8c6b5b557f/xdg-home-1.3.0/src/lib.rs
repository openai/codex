#![doc = include_str!("../README.md")]
#![doc(test(attr(
    warn(unused),
    deny(warnings),
    // W/o this, we seem to get some bogus warning about `extern crate ..`.
    allow(unused_extern_crates),
)))]

use std::path::PathBuf;

/// Get the path of the current user's home directory.
///
/// See the library documentation for more information.
pub fn home_dir() -> Option<PathBuf> {
    match std::env::var("HOME") {
        Ok(home) => Some(home.into()),
        Err(_) => {
            #[cfg(unix)]
            {
                unix::home_dir()
            }

            #[cfg(windows)]
            {
                win32::home_dir()
            }
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::ffi::{CStr, OsStr};
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    pub(super) fn home_dir() -> Option<PathBuf> {
        let uid = unsafe { libc::geteuid() };

        // SAFETY: Not initalizing references here so it's safe.
        let mut passwd: libc::passwd = unsafe { std::mem::zeroed() };
        // This has to be enough for everyone.
        let mut passwd_buf = [0_u8; 1024];
        let mut result = std::ptr::null_mut();
        let ret = unsafe {
            libc::getpwuid_r(
                uid,
                &mut passwd,
                passwd_buf.as_mut_ptr() as *mut _,
                passwd_buf.len(),
                &mut result,
            )
        };
        if ret != 0 || result.is_null() || passwd.pw_dir.is_null() {
            return None;
        }

        // SAFETY: `getpwuid()->pw_dir` is a valid pointer to a c-string.
        let home_dir = unsafe { CStr::from_ptr(passwd.pw_dir) };

        Some(PathBuf::from(OsStr::from_bytes(home_dir.to_bytes())))
    }
}

#[cfg(windows)]
mod win32 {
    use std::{path::PathBuf, ptr::null_mut};

    use windows_sys::Win32::Foundation::S_OK;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::FOLDERID_Profile;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

    pub(super) fn home_dir() -> Option<PathBuf> {
        let rfid = FOLDERID_Profile;
        let mut psz_path = null_mut();
        let res = unsafe { SHGetKnownFolderPath(&rfid, 0, null_mut(), &mut psz_path as *mut _) };
        if res != S_OK {
            return None;
        }

        // Determine the length of the UTF-16 string.
        let mut len = 0;
        // SAFETY: `psz_path` guaranteed to be a valid pointer to a null-terminated UTF-16 string.
        while unsafe { *(psz_path as *const u16).offset(len) } != 0 {
            len += 1;
        }
        let slice = unsafe { std::slice::from_raw_parts(psz_path, len as usize) };
        let path = String::from_utf16(slice).ok()?;
        unsafe {
            CoTaskMemFree(psz_path as *mut _);
        }

        Some(PathBuf::from(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home() {
        let home = home_dir().unwrap();
        assert!(home.is_dir());

        if let Ok(env_home) = std::env::var("HOME") {
            // If `HOME` is set, `home_dir` took the value from it.
            let env_home = PathBuf::from(env_home);
            assert_eq!(home, env_home);

            // With `HOME` unset, `home_dir` should still return the same value.
            std::env::remove_var("HOME");
            assert_eq!(home_dir().unwrap(), env_home);
        }
    }
}

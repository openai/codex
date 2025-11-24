extern crate option_ext;

use std::ffi::OsString;
use std::path::PathBuf;

// we don't need to explicitly handle empty strings in the code above,
// because an empty string is not considered to be a absolute path here.
pub fn is_absolute_path(path: OsString) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Some(path)
    } else {
        None
    }
}

#[cfg(all(unix, not(target_os = "redox")))]
extern crate libc;

#[cfg(all(unix, not(target_os = "redox")))]
mod target_unix_not_redox {

use std::env;
use std::ffi::{CStr, OsString};
use std::mem;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

use super::libc;

// https://github.com/rust-lang/rust/blob/2682b88c526d493edeb2d3f2df358f44db69b73f/library/std/src/sys/unix/os.rs#L595
pub fn home_dir() -> Option<PathBuf> {
    return env::var_os("HOME")
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .or_else(|| unsafe { fallback() })
        .map(PathBuf::from);

    #[cfg(any(target_os = "android", target_os = "ios", target_os = "emscripten"))]
    unsafe fn fallback() -> Option<OsString> {
        None
    }
    #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "emscripten")))]
    unsafe fn fallback() -> Option<OsString> {
        let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            n if n < 0 => 512 as usize,
            n => n as usize,
        };
        let mut buf = Vec::with_capacity(amt);
        let mut passwd: libc::passwd = mem::zeroed();
        let mut result = ptr::null_mut();
        match libc::getpwuid_r(
            libc::getuid(),
            &mut passwd,
            buf.as_mut_ptr(),
            buf.capacity(),
            &mut result,
        ) {
            0 if !result.is_null() => {
                let ptr = passwd.pw_dir as *const _;
                let bytes = CStr::from_ptr(ptr).to_bytes();
                if bytes.is_empty() {
                    None
                } else {
                    Some(OsStringExt::from_vec(bytes.to_vec()))
                }
            }
            _ => None,
        }
    }
}

}

#[cfg(all(unix, not(target_os = "redox")))]
pub use self::target_unix_not_redox::home_dir;

#[cfg(target_os = "redox")]
extern crate redox_users;

#[cfg(target_os = "redox")]
mod target_redox {

use std::path::PathBuf;

use super::redox_users::{All, AllUsers, Config};

pub fn home_dir() -> Option<PathBuf> {
    let current_uid = redox_users::get_uid().ok()?;
    let users = AllUsers::basic(Config::default()).ok()?;
    let user = users.get_by_id(current_uid)?;

    Some(PathBuf::from(user.home.clone()))
}

}

#[cfg(target_os = "redox")]
pub use self::target_redox::home_dir;

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
mod xdg_user_dirs;

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
mod target_unix_not_mac {

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use super::{home_dir, is_absolute_path};
use super::xdg_user_dirs;

fn user_dir_file(home_dir: &Path) -> PathBuf {
    env::var_os("XDG_CONFIG_HOME").and_then(is_absolute_path).unwrap_or_else(|| home_dir.join(".config")).join("user-dirs.dirs")
}

// this could be optimized further to not create a map and instead retrieve the requested path only
pub fn user_dir(user_dir_name: &str) -> Option<PathBuf> {
    if let Some(home_dir) = home_dir() {
        xdg_user_dirs::single(&home_dir, &user_dir_file(&home_dir), user_dir_name).remove(user_dir_name)
    } else {
        None
    }
}

pub fn user_dirs(home_dir_path: &Path) -> HashMap<String, PathBuf> {
    xdg_user_dirs::all(home_dir_path, &user_dir_file(home_dir_path))
}

}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
pub use self::target_unix_not_mac::{user_dir, user_dirs};

#[cfg(target_os = "windows")]
extern crate windows_sys as windows;

#[cfg(target_os = "windows")]
mod target_windows {

use std::ffi::c_void;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::slice;

use super::windows::Win32::UI::Shell;

pub fn known_folder(folder_id: windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let mut path_ptr: windows::core::PWSTR = std::ptr::null_mut();
        let result = Shell::SHGetKnownFolderPath(
            &folder_id,
            0,
            std::ptr::null_mut(),
            &mut path_ptr
        );
        if result == 0 {
            let len = windows::Win32::Globalization::lstrlenW(path_ptr) as usize;
            let path = slice::from_raw_parts(path_ptr, len);
            let ostr: OsString = OsStringExt::from_wide(path);
            windows::Win32::System::Com::CoTaskMemFree(path_ptr as *const c_void);
            Some(PathBuf::from(ostr))
        } else {
            windows::Win32::System::Com::CoTaskMemFree(path_ptr as *const c_void);
            None
        }
    }
}

pub fn known_folder_profile() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Profile)
}

pub fn known_folder_roaming_app_data() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_RoamingAppData)
}

pub fn known_folder_local_app_data() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_LocalAppData)
}

pub fn known_folder_music() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Music)
}

pub fn known_folder_desktop() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Desktop)
}

pub fn known_folder_documents() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Documents)
}

pub fn known_folder_downloads() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Downloads)
}

pub fn known_folder_pictures() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Pictures)
}

pub fn known_folder_public() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Public)
}
pub fn known_folder_templates() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Templates)
}
pub fn known_folder_videos() -> Option<PathBuf> {
    known_folder(Shell::FOLDERID_Videos)
}

}

#[cfg(target_os = "windows")]
pub use self::target_windows::{
    known_folder, known_folder_profile, known_folder_roaming_app_data, known_folder_local_app_data,
    known_folder_music, known_folder_desktop, known_folder_documents, known_folder_downloads,
    known_folder_pictures, known_folder_public, known_folder_templates, known_folder_videos
};

//! Installed helper discovery for automatic work before workspace trust.
//!
//! Only OS and conventional package-manager locations are trusted, never ambient
//! PATH or the working directory. Nix system profiles must resolve into the
//! trusted store. Custom installations fall back to no helper.
//! This assumes the installed software directories themselves are trusted; it
//! does not defend against an attacker who can modify system installations.

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

/// Finds an installed helper without consulting PATH, PATHEXT, or the working directory.
pub fn system_executable(name: &str) -> Option<PathBuf> {
    executable_in_directories(name, &system_directories())
}

/// Child-process PATH for automatic helpers, not for user-requested tools.
pub fn system_path() -> std::io::Result<OsString> {
    let directories = system_directories();
    if directories.is_empty() {
        return Err(std::io::Error::other("no trusted executable directories"));
    }
    std::env::join_paths(directories).map_err(std::io::Error::other)
}

fn executable_in_directories(name: &str, directories: &[PathBuf]) -> Option<PathBuf> {
    // Do not allow callers to accidentally escape the installation directories.
    if Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
        return None;
    }
    let name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    directories.iter().find_map(|directory| {
        let executable = dunce::canonicalize(directory.join(&name)).ok()?;
        // A symlink into a workspace is not an installed executable. Package
        // manager links may target sibling libexec/Cellar directories.
        let installed = directories.iter().any(|root| executable.starts_with(root))
            || installation_roots()
                .iter()
                .any(|root| executable.starts_with(root));
        if !installed || !executable.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if executable.metadata().ok()?.permissions().mode() & 0o111 == 0 {
                return None;
            }
        }
        Some(executable)
    })
}

#[cfg(unix)]
fn installation_roots() -> Vec<PathBuf> {
    [
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
        "/usr/local",
        "/opt/homebrew",
        "/opt/local",
        "/Library/Developer/CommandLineTools",
        "/Applications/Xcode.app/Contents/Developer",
        "/nix/store",
        "/mnt/c/Windows/System32",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn system_directories() -> Vec<PathBuf> {
    #[cfg(unix)]
    let directories = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/local/bin",
        "/run/current-system/sw/bin",
        "/nix/var/nix/profiles/default/bin",
        "/Library/Developer/CommandLineTools/usr/bin",
        "/Applications/Xcode.app/Contents/Developer/usr/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ]
    .into_iter()
    .map(PathBuf::from);
    #[cfg(target_os = "linux")]
    let directories = directories.chain(
        crate::is_wsl()
            .then_some([
                PathBuf::from("/mnt/c/Windows/System32"),
                PathBuf::from("/mnt/c/Windows/System32/WindowsPowerShell/v1.0"),
            ])
            .into_iter()
            .flatten(),
    );
    #[cfg(windows)]
    let directories = installation_roots().into_iter().flat_map(|root| {
        vec![
            root.join("Git/cmd"),
            root.join("Git/mingw64/bin"),
            root.join("Git/usr/bin"),
            root,
        ]
    });
    let roots = installation_roots();
    directories
        .filter_map(|directory| dunce::canonicalize(directory).ok())
        .filter(|directory| roots.iter().any(|root| directory.starts_with(root)))
        .collect()
}

#[cfg(windows)]
fn installation_roots() -> Vec<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramFiles;
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramFilesX86;
    use windows_sys::Win32::UI::Shell::FOLDERID_System;
    use windows_sys::Win32::UI::Shell::KF_FLAG_DEFAULT_PATH;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

    [
        FOLDERID_ProgramFiles,
        FOLDERID_ProgramFilesX86,
        FOLDERID_System,
    ]
    .iter()
    .filter_map(|id| {
        let mut path = std::ptr::null_mut();
        // SAFETY: the API writes a CoTaskMem-allocated, NUL-terminated UTF-16 path.
        let result = unsafe {
            SHGetKnownFolderPath(
                id,
                KF_FLAG_DEFAULT_PATH as u32,
                /*htoken*/ 0,
                &mut path,
            )
        };
        if result != 0 || path.is_null() {
            return None;
        }
        // SAFETY: success guarantees the string; free it after copying.
        let directory = unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            let directory =
                PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(path, len)));
            CoTaskMemFree(path.cast());
            directory
        };
        dunce::canonicalize(directory).ok()
    })
    .collect()
}

#[cfg(test)]
#[path = "system_commands_tests.rs"]
mod tests;

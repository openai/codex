use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::errors::GitReadError;
use crate::repository_authority::RepositoryAuthority;
#[cfg(windows)]
use std::path::Component;
#[cfg(windows)]
use std::path::Prefix;

pub(crate) struct SelectedGitExecutable {
    pub(crate) executable: PathBuf,
    pub(crate) argv0: PathBuf,
    pub(crate) safe_path: OsString,
}

pub(crate) fn select_git_executable(
    authority: &RepositoryAuthority,
    search_path: &OsStr,
) -> Result<SelectedGitExecutable, GitReadError> {
    let mut safe_directories = Vec::new();
    let mut selected = None;
    for directory in std::env::split_paths(search_path) {
        if !directory.is_absolute() || search_directory_is_untrusted(&directory, authority) {
            continue;
        }
        let Ok(canonical_directory) = std::fs::canonicalize(&directory) else {
            continue;
        };
        if path_is_untrusted(&canonical_directory, authority)
            || !std::fs::metadata(&canonical_directory).is_ok_and(|metadata| metadata.is_dir())
        {
            continue;
        }
        push_unique_path(&mut safe_directories, canonical_directory);

        if selected.is_some() {
            continue;
        }
        let candidate = directory.join(git_executable_name());
        if path_is_untrusted(&candidate, authority) {
            continue;
        }
        let Ok(canonical_candidate) = std::fs::canonicalize(&candidate) else {
            continue;
        };
        if path_is_untrusted(&canonical_candidate, authority)
            || !is_native_executable_file(&canonical_candidate)
        {
            continue;
        }
        if let Some(parent) = canonical_candidate.parent() {
            push_unique_path(&mut safe_directories, parent.to_path_buf());
        }
        selected = Some((canonical_candidate, candidate));
    }
    let (executable, argv0) = selected.ok_or(GitReadError::NoTrustedGit)?;
    let safe_path =
        std::env::join_paths(safe_directories).map_err(|_| GitReadError::NoTrustedGit)?;
    Ok(SelectedGitExecutable {
        executable,
        argv0,
        safe_path,
    })
}

pub(crate) fn harden_git_launch_environment(
    command: &mut std::process::Command,
    safe_path: &OsStr,
) {
    let mut names = std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| startup_injection_variable(name))
        .collect::<Vec<_>>();
    names.extend(
        command
            .get_envs()
            .filter(|&(name, _)| startup_injection_variable(name))
            .map(|(name, _)| name.to_os_string()),
    );
    for name in names {
        command.env_remove(name);
    }
    command.env("PATH", safe_path);
    #[cfg(windows)]
    command.env("NoDefaultCurrentDirectoryInExePath", "1");
}

fn startup_injection_variable(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_uppercase();
    name.starts_with("DYLD_")
        || name.starts_with("LD_")
        || name == "LIBPATH"
        || name == "SHLIB_PATH"
        || name.starts_with("CORECLR_")
        || name.starts_with("COR_")
        || name.starts_with("DOTNET_")
        || name == "GCONV_PATH"
        || name == "NIX_LD"
        || name == "NIX_LD_LIBRARY_PATH"
}

pub(crate) fn path_is_untrusted(path: &Path, authority: &RepositoryAuthority) -> bool {
    authority.path_is_untrusted_for_executable(path)
}

pub(crate) fn search_directory_is_untrusted(
    directory: &Path,
    authority: &RepositoryAuthority,
) -> bool {
    #[cfg(windows)]
    if windows_path_requires_fail_closed(directory)
        || windows_path_has_untrusted_canonical_ancestor(directory, authority)
    {
        return true;
    }
    path_is_untrusted(directory, authority)
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(windows)]
pub(crate) fn windows_path_requires_fail_closed(path: &Path) -> bool {
    let mut components = path.components();
    let supported_namespace = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(_)
            | Prefix::VerbatimDisk(_)
            | Prefix::UNC(_, _)
            | Prefix::VerbatimUNC(_, _) => true,
            Prefix::DeviceNS(device) => windows_device_namespace_is_filesystem(device),
            Prefix::Verbatim(namespace) => namespace
                .to_str()
                .is_some_and(|namespace| namespace.eq_ignore_ascii_case("UNC")),
        },
        _ => false,
    };
    !supported_namespace || components.any(|component| matches!(component, Component::ParentDir))
}

#[cfg(windows)]
fn windows_device_namespace_is_filesystem(device: &OsStr) -> bool {
    let bytes = device.as_encoded_bytes();
    bytes.eq_ignore_ascii_case(b"UNC")
        || matches!(bytes, [drive, b':'] if drive.is_ascii_alphabetic())
}

#[cfg(windows)]
fn windows_path_has_untrusted_canonical_ancestor(
    path: &Path,
    authority: &RepositoryAuthority,
) -> bool {
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return true;
    };
    if path_is_untrusted(&canonical_path, authority) {
        return true;
    }
    for ancestor in path.ancestors().skip(1) {
        let canonical_ancestor = match std::fs::canonicalize(ancestor) {
            Ok(canonical_ancestor) => canonical_ancestor,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
                ) =>
            {
                continue;
            }
            Err(_) => return true,
        };
        if path_is_untrusted(&canonical_ancestor, authority) {
            return true;
        }
    }
    false
}

#[cfg(windows)]
pub(crate) fn git_executable_name() -> &'static str {
    "git.exe"
}

#[cfg(not(windows))]
pub(crate) fn git_executable_name() -> &'static str {
    "git"
}

#[cfg(target_os = "macos")]
pub(crate) fn is_native_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return false;
    }
    let Ok(bytes) = read_prefix(path, 4) else {
        return false;
    };
    matches!(
        bytes.as_slice(),
        [0xfe, 0xed, 0xfa, 0xce]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xce, 0xfa, 0xed, 0xfe]
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
            | [0xca, 0xfe, 0xba, 0xbf]
            | [0xbf, 0xba, 0xfe, 0xca]
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn is_native_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    metadata.is_file()
        && metadata.permissions().mode() & 0o111 != 0
        && read_prefix(path, 4).is_ok_and(|bytes| bytes == b"\x7fELF")
}

#[cfg(windows)]
pub(crate) fn is_native_executable_file(path: &Path) -> bool {
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;

    if !path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return false;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() < 68 {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut dos = [0_u8; 64];
    if file.read_exact(&mut dos).is_err() || &dos[..2] != b"MZ" {
        return false;
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().expect("PE offset bytes")) as u64;
    if offset > 1024 * 1024 || offset + 4 > metadata.len() {
        return false;
    }
    let mut signature = [0_u8; 4];
    file.seek(SeekFrom::Start(offset)).is_ok()
        && file.read_exact(&mut signature).is_ok()
        && signature == *b"PE\0\0"
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_native_executable_file(_path: &Path) -> bool {
    false
}

#[cfg(unix)]
fn read_prefix(path: &Path, length: usize) -> io::Result<Vec<u8>> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

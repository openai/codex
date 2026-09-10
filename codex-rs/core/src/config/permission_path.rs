//! Permission-profile path grammar. Path facts and resolution are owned by
//! the shared configuration context; symbolic paths preserve native spelling.

use std::io;

use codex_config::ConfigPathContext;
use codex_utils_absolute_path::normalize_windows_device_path;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

pub(super) fn contains_glob(path: &str, context: &ConfigPathContext) -> io::Result<bool> {
    Ok(contains_glob_chars_for_platform(
        path,
        context.convention() == PathConvention::Windows,
    ))
}

pub(super) fn absolute_path(path: &str, context: &ConfigPathContext) -> io::Result<PathUri> {
    let convention = context.convention();
    if convention.home_relative_suffix(path).is_none()
        && LegacyAppPathString::from_string(path)
            .to_path_uri(convention)
            .is_err()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("filesystem path `{path}` must be absolute, use `~/...`, or start with `:`"),
        ));
    }
    context
        .resolve_path(path)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
}

pub(super) fn relative_subpath(subpath: &str, context: &ConfigPathContext) -> io::Result<String> {
    let convention = context.convention();
    let mut components = convention.path_segments(subpath);
    let first = components.next();
    let has_drive_prefix = convention == PathConvention::Windows
        && subpath
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && subpath.as_bytes().get(1) == Some(&b':');
    if !subpath.is_empty()
        && !matches!(first, Some("" | "." | ".."))
        && !has_drive_prefix
        && !components.any(|component| component == "..")
    {
        // Preserve spelling after the same validation as native components:
        // interior `.` and repeated separators stay in symbolic rules.
        return Ok(subpath.to_string());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "filesystem subpath `{subpath}` must be a descendant path without `.` or `..` components"
        ),
    ))
}

pub(super) fn contains_glob_chars_for_platform(path: &str, is_windows: bool) -> bool {
    let normalized_windows_path = if is_windows {
        normalize_windows_device_path(path)
    } else {
        None
    };
    let path = normalized_windows_path.as_deref().unwrap_or(path);
    path.chars().any(|ch| matches!(ch, '*' | '?' | '[' | ']'))
}

#[cfg(test)]
#[path = "permission_path_tests.rs"]
mod tests;

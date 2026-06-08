use std::fmt;

use thiserror::Error;

/// A normalized absolute URI path in a configured environment.
///
/// The stored representation is host-independent and always uses `/`
/// separators:
///
/// - A POSIX path such as `/srv/app/main.rs` keeps that spelling.
/// - A Windows drive path such as `C:\Users\Alice` becomes
///   `/c:/Users/Alice`. Drive letters are lowercased.
/// - A Windows UNC path such as `\\server\share\src` becomes
///   `//server/share/src`. UNC, or Universal Naming Convention, paths address a
///   network share by server and share name rather than by drive letter.
///
/// Construction is lexical: repeated separators and `.` components are
/// removed, while `..` components are resolved without escaping the POSIX,
/// drive, or UNC root. Original separator spelling and drive-letter case are
/// therefore not preserved. Filesystem aliases, symlinks, case sensitivity,
/// reserved names, and Unicode normalization are intentionally not resolved.
///
/// [`Self::new`] accepts the canonical URI representation and recognizes
/// canonical drive and UNC roots. Use [`Self::from_native_path`] for a path
/// from the current host, or [`Self::posix`] and [`Self::windows`] when the
/// source host's path syntax is known explicitly. The explicit constructors
/// ensure a POSIX filename that resembles a drive or UNC path is not assigned
/// Windows root semantics. Converting back to native syntax requires an
/// explicit [`PathFlavor`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnvironmentPath(String);

impl EnvironmentPath {
    /// Creates a path from absolute canonical URI path syntax.
    ///
    /// This constructor recognizes `/c:/` drive roots and `//server/share` UNC
    /// roots regardless of the current host. Use [`Self::from_native_path`] for
    /// native input from the current host.
    pub fn new(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        let path = path.into();
        validate_environment_path(&path)?;
        let path = normalize_environment_path(&path, PathNormalization::InferWindowsRoots);
        Ok(Self(path))
    }

    /// Converts an absolute native path from the current host.
    pub fn from_native_path(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        if cfg!(windows) {
            Self::windows(path)
        } else {
            Self::posix(path)
        }
    }

    /// Converts an absolute native POSIX path.
    pub fn posix(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        let path = path.into();
        validate_posix_path(&path)?;
        Ok(Self(normalize_environment_path(
            &path,
            PathNormalization::Posix,
        )))
    }

    /// Converts an absolute native Windows drive or UNC path.
    pub fn windows(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        let path = path.into();
        validate_windows_path(&path)?;
        let path = path.replace('\\', "/");
        if path.starts_with("//") {
            return Ok(Self(normalize_environment_path(
                &path,
                PathNormalization::InferWindowsRoots,
            )));
        }

        Self::new(format!("/{path}"))
    }

    /// Returns the canonical URI path spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this canonical URI path to the requested native path syntax.
    ///
    /// Windows conversion only succeeds for a fully qualified drive or UNC
    /// path. This avoids returning root-relative paths whose meaning depends on
    /// the process's current drive.
    pub fn to_native_path(&self, flavor: PathFlavor) -> Result<String, EnvironmentPathError> {
        match flavor {
            PathFlavor::Posix => Ok(self.0.clone()),
            PathFlavor::Windows => {
                if has_unsupported_windows_namespace(self.as_str()) {
                    return Err(EnvironmentPathError::UnsupportedWindowsNamespace(
                        self.to_string(),
                    ));
                }
                let path = self.as_str().replace('\\', "/");
                if !(is_windows_drive_path(&path)
                    || path.starts_with("//") && is_valid_unc_path(&path))
                {
                    return Err(EnvironmentPathError::IncompatiblePathFlavor {
                        path: self.to_string(),
                        flavor,
                    });
                }
                let path = normalize_environment_path(&path, PathNormalization::InferWindowsRoots);
                let path = if is_windows_drive_path(&path) {
                    &path[1..]
                } else {
                    &path
                };
                Ok(path.replace('/', "\\"))
            }
        }
    }

    /// Returns the final path component, or `None` for a filesystem root.
    pub fn basename(&self) -> Option<&str> {
        let root_depth = environment_path_root_depth(self.as_str());
        let components = environment_path_components(self.as_str());
        if components.len() <= root_depth {
            return None;
        }
        components.last().copied()
    }

    /// Returns the normalized parent path, or `None` for a filesystem root.
    pub fn parent(&self) -> Option<Self> {
        let root_depth = environment_path_root_depth(self.as_str());
        let mut components = environment_path_components(self.as_str());
        if components.len() <= root_depth {
            return None;
        }
        components.pop();
        Some(Self(format_environment_path(
            self.as_str(),
            &components,
            PathNormalization::InferWindowsRoots,
        )))
    }

    /// Lexically joins a relative normalized URI path onto this path.
    pub fn join(&self, path: &str) -> Result<Self, EnvironmentPathError> {
        if path.starts_with('/') {
            return Err(EnvironmentPathError::JoinPathMustBeRelative(
                path.to_string(),
            ));
        }
        if path.contains('\0') {
            return Err(EnvironmentPathError::ContainsNull);
        }
        if path.is_empty() {
            return Ok(self.clone());
        }

        let joined = format!("{}/{}", self.as_str().trim_end_matches('/'), path);
        Self::new(joined)
    }
}

impl fmt::Display for EnvironmentPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for EnvironmentPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Validates a path that is already expressed in canonical URI syntax.
///
/// Canonical paths use POSIX separators, but a leading drive or UNC root is
/// interpreted using Windows root rules.
fn validate_environment_path(path: &str) -> Result<(), EnvironmentPathError> {
    validate_posix_path(path)?;
    if has_unsupported_windows_namespace(path) {
        return Err(EnvironmentPathError::UnsupportedWindowsNamespace(
            path.to_string(),
        ));
    }
    if path.starts_with("//") && !path.starts_with("///") && !is_valid_unc_path(path) {
        return Err(EnvironmentPathError::InvalidWindowsUncPath(
            path.to_string(),
        ));
    }
    Ok(())
}

/// Validates the invariants common to absolute POSIX and canonical URI paths.
fn validate_posix_path(path: &str) -> Result<(), EnvironmentPathError> {
    if path.is_empty() {
        return Err(EnvironmentPathError::Empty);
    }
    if path.contains('\0') {
        return Err(EnvironmentPathError::ContainsNull);
    }
    if !path.starts_with('/') {
        return Err(EnvironmentPathError::NotAbsolute(path.to_string()));
    }
    Ok(())
}

/// Validates an absolute native Windows drive or UNC path.
///
/// Root-relative paths such as `\src` and drive-relative paths such as
/// `C:src` are rejected because their meaning depends on process state.
fn validate_windows_path(path: &str) -> Result<(), EnvironmentPathError> {
    if path.is_empty() {
        return Err(EnvironmentPathError::Empty);
    }
    if path.contains('\0') {
        return Err(EnvironmentPathError::ContainsNull);
    }
    if has_unsupported_windows_namespace(path) {
        return Err(EnvironmentPathError::UnsupportedWindowsNamespace(
            path.to_string(),
        ));
    }
    if path.starts_with(r"\\") || path.starts_with("//") {
        let normalized = path.replace('\\', "/");
        if is_valid_unc_path(&normalized) {
            return Ok(());
        }
        return Err(EnvironmentPathError::InvalidWindowsUncPath(
            path.to_string(),
        ));
    }
    if matches!(
        path.as_bytes(),
        [drive, b':', separator, ..]
            if drive.is_ascii_alphabetic() && matches!(separator, b'/' | b'\\')
    ) {
        return Ok(());
    }
    Err(EnvironmentPathError::NotAbsolute(path.to_string()))
}

/// Returns whether a normalized UNC path contains both its server and share.
///
/// Callers only pass strings beginning with `//`. The server and share cannot
/// be dot segments because those would make root depth ambiguous during lexical
/// normalization.
fn is_valid_unc_path(path: &str) -> bool {
    let mut components = path[2..]
        .split('/')
        .filter(|component| !component.is_empty());
    components
        .next()
        .is_some_and(|component| !matches!(component, "." | ".."))
        && components
            .next()
            .is_some_and(|component| !matches!(component, "." | ".."))
}

/// Detects Windows device and verbatim namespaces that URI paths cannot model.
fn has_unsupported_windows_namespace(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path.starts_with("//?/") || path.starts_with("//./")
}

/// Controls whether a canonical path may assign Windows root semantics.
#[derive(Clone, Copy)]
enum PathNormalization {
    /// Treat every component as POSIX filename text.
    Posix,
    /// Recognize canonical drive and UNC roots.
    InferWindowsRoots,
}

/// Lexically normalizes separators and dot segments without filesystem access.
fn normalize_environment_path(path: &str, normalization: PathNormalization) -> String {
    let mut path = path.to_string();
    // Drive letters are case-insensitive in native Windows paths. Lowercasing
    // gives canonical URI identity without folding the rest of the path.
    if matches!(normalization, PathNormalization::InferWindowsRoots)
        && matches!(
            path.as_bytes(),
            [b'/', drive, b':'] | [b'/', drive, b':', b'/', ..]
                if drive.is_ascii_alphabetic()
        )
    {
        path[1..2].make_ascii_lowercase();
    }

    // Root components are protected from `..`: none for POSIX, the drive for a
    // drive path, and the server plus share for UNC.
    let root_depth = match normalization {
        PathNormalization::Posix => 0,
        PathNormalization::InferWindowsRoots => environment_path_root_depth(&path),
    };
    let mut components = Vec::new();
    for component in environment_path_components(&path) {
        match component {
            "" | "." => {}
            ".." if components.len() > root_depth => {
                components.pop();
            }
            ".." => {}
            component => components.push(component),
        }
    }
    format_environment_path(&path, &components, normalization)
}

/// Returns the number of components that form an inferred Windows root.
fn environment_path_root_depth(path: &str) -> usize {
    if path.starts_with("//") && !path.starts_with("///") {
        2
    } else if is_windows_drive_path(path) {
        1
    } else {
        0
    }
}

/// Recognizes canonical `/c:/...` Windows drive paths.
fn is_windows_drive_path(path: &str) -> bool {
    matches!(
        path.as_bytes(),
        [b'/', drive, b':', b'/', ..] if drive.is_ascii_alphabetic()
    )
}

/// Splits a canonical path into non-empty components after its leading slash.
fn environment_path_components(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|component| !component.is_empty())
        .collect()
}

/// Reassembles normalized components while preserving the inferred root form.
fn format_environment_path(
    original: &str,
    components: &[&str],
    normalization: PathNormalization,
) -> String {
    if original.starts_with("//") && !original.starts_with("///") {
        return format!("//{}", components.join("/"));
    }

    let path = format!("/{}", components.join("/"));
    if matches!(normalization, PathNormalization::InferWindowsRoots)
        && (original.ends_with('/') || is_windows_drive_path(original))
        && components.len() == 1
        && components[0].len() == 2
        && components[0].ends_with(':')
        && components[0].as_bytes()[0].is_ascii_alphabetic()
    {
        format!("{path}/")
    } else {
        path
    }
}

/// Native path syntax used by an execution environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PathFlavor {
    Posix,
    Windows,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EnvironmentPathError {
    #[error("environment path cannot be empty")]
    Empty,
    #[error("environment path `{0}` must be absolute")]
    NotAbsolute(String),
    #[error("environment path cannot contain a null character")]
    ContainsNull,
    #[error("path `{0}` uses an unsupported Windows device or verbatim namespace")]
    UnsupportedWindowsNamespace(String),
    #[error("Windows UNC path `{0}` must contain a server and share")]
    InvalidWindowsUncPath(String),
    #[error("path `{0}` must be relative when joining an environment path")]
    JoinPathMustBeRelative(String),
    #[error("path `{path}` cannot be represented using {flavor:?} path syntax")]
    IncompatiblePathFlavor { path: String, flavor: PathFlavor },
}

#[cfg(test)]
#[path = "environment_path_tests.rs"]
mod tests;

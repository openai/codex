use std::fmt;

use thiserror::Error;

/// Native path syntax used by an execution environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PathFlavor {
    Posix,
    Windows,
}

/// A normalized absolute URI path in a configured environment.
///
/// The stored representation always uses `/` separators. Use [`Self::posix`]
/// or [`Self::windows`] when converting a native path into this representation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnvironmentPath(String);

impl EnvironmentPath {
    /// Creates a path that is already in normalized URI path syntax.
    pub fn new(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        let path = path.into();
        validate_environment_path(&path)?;
        Ok(Self(normalize_environment_path(&path)))
    }

    pub fn posix(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        Self::new(path)
    }

    pub fn windows(path: impl Into<String>) -> Result<Self, EnvironmentPathError> {
        let path = path.into();
        validate_windows_path(&path)?;
        let path = path.replace('\\', "/");
        if path.starts_with("//") {
            return Ok(Self(normalize_environment_path(&path)));
        }

        Self::new(format!("/{path}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this canonical URI path to the requested native path syntax.
    pub fn to_native_path(&self, flavor: PathFlavor) -> String {
        match flavor {
            PathFlavor::Posix => self.0.clone(),
            PathFlavor::Windows => {
                let path = if is_windows_drive_path(self.as_str()) {
                    &self.as_str()[1..]
                } else {
                    self.as_str()
                };
                path.replace('/', "\\")
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
        Some(Self(format_environment_path(self.as_str(), &components)))
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

impl EnvironmentPath {
    pub(crate) fn from_normalized(path: String) -> Self {
        Self(path)
    }
}

fn validate_environment_path(path: &str) -> Result<(), EnvironmentPathError> {
    if path.is_empty() {
        return Err(EnvironmentPathError::Empty);
    }
    if path.contains('\0') {
        return Err(EnvironmentPathError::ContainsNull);
    }
    if !path.starts_with('/') {
        return Err(EnvironmentPathError::NotAbsolute(path.to_string()));
    }
    if path.starts_with("//") && !path.starts_with("///") && !is_valid_unc_path(path) {
        return Err(EnvironmentPathError::InvalidWindowsUncPath(
            path.to_string(),
        ));
    }
    Ok(())
}

fn validate_windows_path(path: &str) -> Result<(), EnvironmentPathError> {
    if path.is_empty() {
        return Err(EnvironmentPathError::Empty);
    }
    if path.contains('\0') {
        return Err(EnvironmentPathError::ContainsNull);
    }
    if path.starts_with(r"\\?\") || path.starts_with(r"\\.\") {
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

fn is_valid_unc_path(path: &str) -> bool {
    let mut components = path[2..]
        .split('/')
        .filter(|component| !component.is_empty());
    components.next().is_some() && components.next().is_some()
}

fn normalize_environment_path(path: &str) -> String {
    let mut path = path.to_string();
    if matches!(
        path.as_bytes(),
        [b'/', drive, b':', b'/', ..] if drive.is_ascii_alphabetic()
    ) {
        path[1..2].make_ascii_lowercase();
    }

    let root_depth = environment_path_root_depth(&path);
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
    format_environment_path(&path, &components)
}

fn environment_path_root_depth(path: &str) -> usize {
    if path.starts_with("//") && !path.starts_with("///") {
        2
    } else if is_windows_drive_path(path) {
        1
    } else {
        0
    }
}

fn is_windows_drive_path(path: &str) -> bool {
    matches!(
        path.as_bytes(),
        [b'/', drive, b':', b'/', ..] if drive.is_ascii_alphabetic()
    )
}

fn environment_path_components(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|component| !component.is_empty())
        .collect()
}

fn format_environment_path(original: &str, components: &[&str]) -> String {
    if original.starts_with("//") && !original.starts_with("///") {
        return format!("//{}", components.join("/"));
    }

    let path = format!("/{}", components.join("/"));
    if components.len() == 1
        && components[0].len() == 2
        && components[0].ends_with(':')
        && components[0].as_bytes()[0].is_ascii_alphabetic()
    {
        format!("{path}/")
    } else {
        path
    }
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
}

#[cfg(test)]
#[path = "environment_path_tests.rs"]
mod tests;

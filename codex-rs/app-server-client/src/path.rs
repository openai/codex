//! App-server-host path strings.
//!
//! Remote app-server fs APIs deserialize and resolve paths on the server host,
//! so callers must not parse remote paths using their local platform rules.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppServerPath(String);

impl AppServerPath {
    pub fn from_app_server(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn from_absolute_str(raw: &str) -> Option<Self> {
        is_absolute_app_server_path(raw).then(|| Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn components(&self) -> Vec<&str> {
        self.0
            .split(['/', '\\'])
            .filter(|part| !part.is_empty())
            .collect()
    }

    pub fn join(&self, segment: impl AsRef<str>) -> Self {
        let separator = if is_windows_absolute_path(&self.0) {
            '\\'
        } else {
            '/'
        };
        let mut raw = self.0.trim_end_matches(['/', '\\']).to_string();
        if !raw.ends_with(separator) {
            raw.push(separator);
        }
        raw.push_str(segment.as_ref());
        Self(raw)
    }
}

impl fmt::Display for AppServerPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn is_absolute_app_server_path(path: &str) -> bool {
    path.starts_with('/') || is_windows_absolute_path(path)
}

fn is_windows_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/'))
        || path.starts_with("\\\\")
        || path.starts_with("//")
}

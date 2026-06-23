use crate::PathConvention;
use crate::PathUri;
use crate::is_windows_separator_byte;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use std::path::Path;
use thiserror::Error;
use ts_rs::TS;

/// A UTF-8 path for preserving raw path compatibility at the app-server API
/// boundary while Codex migrates to [`PathUri`].
///
/// Supports storing arbitrary strings read from the API and converting to and
/// from [`PathUri`] using an explicitly selected native path convention.
///
/// When converting from [`PathUri`], "native" refers to the supplied
/// [`PathConvention`], which may be foreign to the operating system running
/// this process. The inner string is private so path-producing code must use a
/// path conversion method instead of bypassing the intended conversion
/// boundary. Non-UTF-8 paths are converted to UTF-8 lossily because this API
/// value is serialized as a JSON string.
///
/// Deserialization accepts any UTF-8 string without interpreting or validating
/// it. That unrestricted construction path is intentionally available only to
/// serde: Codex-internal code cannot construct this type directly from a raw
/// `String` and is instead encouraged to convert through [`PathUri`] or
/// [`AbsolutePathBuf`]. Relative path text remains valid until an operation
/// such as [`Self::to_path_uri`] requires an absolute path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, TS)]
#[serde(transparent)]
#[ts(type = "string")]
pub struct LegacyAppPathString(String);

impl LegacyAppPathString {
    /// Preserves path text without interpreting it using the current host.
    pub fn from_path(path: &Path) -> Self {
        Self(path.to_string_lossy().into_owned())
    }

    /// Renders an absolute path using the current host's path convention.
    pub fn from_abs_path(path: &AbsolutePathBuf) -> Self {
        Self::from_path(path.as_path())
    }

    /// Renders a path URI using the requested native path convention.
    ///
    /// Rendering fails when the URI shape does not match the convention, such
    /// as a POSIX path rendered as Windows or a UNC path rendered as POSIX. It
    /// also fails when an opaque fallback does not encode an absolute path for
    /// the convention. Non-UTF-8 segments are rendered lossily, and encoded
    /// separators are emitted as native path text.
    pub fn from_path_uri(
        path: &PathUri,
        convention: PathConvention,
    ) -> Result<Self, LegacyAppPathStringError> {
        if let Some(path_bytes) = path.opaque_fallback_bytes() {
            return render_opaque_fallback(path, &path_bytes, convention).map(Self);
        }
        match convention {
            PathConvention::Posix => render_posix_path(path),
            PathConvention::Windows => render_windows_path(path),
        }
        .map(Self)
    }

    /// Parses this API string as an absolute path using the requested native
    /// path convention and returns its canonical path URI.
    pub fn to_path_uri(
        &self,
        convention: PathConvention,
    ) -> Result<PathUri, LegacyAppPathStringError> {
        PathUri::from_absolute_native_path(&self.0, convention).ok_or_else(|| {
            LegacyAppPathStringError::InvalidNativePath {
                path: self.0.clone(),
                convention: Some(convention),
            }
        })
    }

    /// Parses this API string as an absolute path using the convention inferred from its spelling.
    pub fn to_inferred_path_uri(&self) -> Option<PathUri> {
        PathUri::try_from(self.clone()).ok()
    }

    /// Parses this API string as a host-native absolute path.
    pub fn to_inferred_abs_path(&self) -> Option<AbsolutePathBuf> {
        AbsolutePathBuf::try_from(self.clone()).ok()
    }

    /// Infers the path convention of an absolute API path from its spelling.
    ///
    /// Relative paths and ambiguous spellings return `None`. In particular,
    /// slash-prefixed paths are treated as POSIX even when they could also be
    /// interpreted as slash-delimited Windows UNC paths.
    pub fn infer_absolute_path_convention(&self) -> Option<PathConvention> {
        let bytes = self.0.as_bytes();
        let has_windows_drive_root = matches!(
            bytes,
            [drive, b':', separator, ..]
                if drive.is_ascii_alphabetic() && is_windows_separator_byte(*separator)
        );
        if has_windows_drive_root || self.0.starts_with(r"\\") {
            Some(PathConvention::Windows)
        } else if self.0.starts_with('/') {
            Some(PathConvention::Posix)
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<AbsolutePathBuf> for LegacyAppPathString {
    fn from(path: AbsolutePathBuf) -> Self {
        Self::from_abs_path(&path)
    }
}

impl From<PathUri> for LegacyAppPathString {
    fn from(path: PathUri) -> Self {
        Self(path.inferred_native_path_string())
    }
}

impl TryFrom<LegacyAppPathString> for PathUri {
    type Error = LegacyAppPathStringError;

    fn try_from(path: LegacyAppPathString) -> Result<Self, Self::Error> {
        let Some(convention) = path.infer_absolute_path_convention() else {
            return Err(LegacyAppPathStringError::InvalidNativePath {
                path: path.0,
                convention: None,
            });
        };
        PathUri::from_absolute_native_path(path.as_str(), convention).ok_or({
            LegacyAppPathStringError::InvalidNativePath {
                path: path.0,
                convention: Some(convention),
            }
        })
    }
}

impl TryFrom<LegacyAppPathString> for AbsolutePathBuf {
    type Error = LegacyAppPathStringError;

    fn try_from(path: LegacyAppPathString) -> Result<Self, Self::Error> {
        AbsolutePathBuf::from_absolute_path_checked(path.as_str()).map_err(|_| {
            LegacyAppPathStringError::InvalidNativePath {
                path: path.0,
                convention: None,
            }
        })
    }
}

fn render_opaque_fallback(
    path: &PathUri,
    path_bytes: &[u8],
    convention: PathConvention,
) -> Result<String, LegacyAppPathStringError> {
    let rendered = match convention {
        PathConvention::Posix if path_bytes.starts_with(b"/") => {
            Some(String::from_utf8_lossy(path_bytes).into_owned())
        }
        PathConvention::Windows => render_windows_opaque_fallback(path_bytes),
        PathConvention::Posix => None,
    };
    rendered.ok_or_else(|| LegacyAppPathStringError::OpaqueFallback {
        path: path.to_string(),
    })
}

fn render_windows_opaque_fallback(path_bytes: &[u8]) -> Option<String> {
    if !path_bytes.len().is_multiple_of(2) {
        return None;
    }
    let path_wide = path_bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();

    // Windows absolute paths either have a rooted drive prefix (`C:\\`) or a
    // rooted namespace/UNC prefix (`\\server`, `\\.\\`, or `\\?\\`).
    let has_drive_root = matches!(
        path_wide.as_slice(),
        [drive, colon, separator, ..]
            if ((u16::from(b'A')..=u16::from(b'Z')).contains(drive)
                || (u16::from(b'a')..=u16::from(b'z')).contains(drive))
                && *colon == u16::from(b':')
                && is_windows_separator(*separator)
    );
    let has_namespace_or_unc_root = matches!(
        path_wide.as_slice(),
        [first, second, ..]
            if is_windows_separator(*first) && is_windows_separator(*second)
    );
    (has_drive_root || has_namespace_or_unc_root).then(|| String::from_utf16_lossy(&path_wide))
}

fn is_windows_separator(character: u16) -> bool {
    character == u16::from(b'\\') || character == u16::from(b'/')
}

impl fmt::Display for LegacyAppPathString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for LegacyAppPathString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl JsonSchema for LegacyAppPathString {
    fn schema_name() -> String {
        "LegacyAppPathString".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(generator)
    }
}

fn render_posix_path(path: &PathUri) -> Result<String, LegacyAppPathStringError> {
    let url = path.to_url();
    // POSIX file paths do not have a UNC authority, so `file://server/share`
    // cannot be represented as `/share` without losing the server identity.
    if url.host_str().is_some() {
        return Err(incompatible_convention(path, PathConvention::Posix));
    }

    // URI segments are already separated with `/` on every host. Decode each
    // one independently so `file:///a%20dir/file` becomes `/a dir/file`.
    let mut rendered = String::new();
    for segment in path_segments(&url) {
        rendered.push('/');
        rendered.push_str(&decode_native_segment(segment));
    }
    Ok(rendered)
}

fn render_windows_path(path: &PathUri) -> Result<String, LegacyAppPathStringError> {
    let url = path.to_url();
    let mut segments = path_segments(&url);
    let mut rendered = String::new();
    if let Some(host) = url.host_str() {
        // A URI authority selects the UNC form: `file://server/share/file`
        // becomes `\\server\share\file`. The first segment is the share name,
        // which must be present.
        let Some(share) = segments.next() else {
            return Err(incompatible_convention(path, PathConvention::Windows));
        };
        let share = decode_native_segment(share);
        if share.is_empty() {
            return Err(incompatible_convention(path, PathConvention::Windows));
        }
        rendered.push_str(r"\\");
        rendered.push_str(host);
        rendered.push('\\');
        rendered.push_str(&share);
    } else {
        // Without an authority, Windows requires a drive root. For example,
        // `file:///C:/src/main.rs` begins with the `C:` URI segment and renders
        // as `C:\src\main.rs`; a POSIX URI such as `file:///usr/bin` is rejected.
        let Some(drive) = segments.next() else {
            return Err(incompatible_convention(path, PathConvention::Windows));
        };
        let drive = decode_native_segment(drive);
        let bytes = drive.as_bytes();
        if bytes.len() != 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
            return Err(incompatible_convention(path, PathConvention::Windows));
        }
        rendered.push_str(&drive);
    }

    for segment in segments {
        // URL path separators become Windows separators after each component
        // has been decoded.
        let segment = decode_native_segment(segment);
        rendered.push('\\');
        rendered.push_str(&segment);
    }
    // `file:///C:` and `file:///C:/` both identify the drive root, never the
    // drive-relative path `C:`.
    if rendered.len() == 2 && rendered.as_bytes()[1] == b':' {
        rendered.push('\\');
    }
    Ok(rendered)
}

fn path_segments(url: &url::Url) -> std::str::Split<'_, char> {
    url.path_segments()
        .unwrap_or_else(|| unreachable!("validated file URLs have path segments"))
}

fn decode_native_segment(segment: &str) -> String {
    // Decode exactly once. Thus `%20` becomes a space and `%252F` becomes the
    // literal text `%2F`, rather than being decoded a second time into `/`.
    let bytes = urlencoding::decode_binary(segment.as_bytes());
    String::from_utf8_lossy(&bytes).into_owned()
}

fn incompatible_convention(path: &PathUri, convention: PathConvention) -> LegacyAppPathStringError {
    LegacyAppPathStringError::IncompatibleConvention {
        path: path.to_string(),
        convention,
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LegacyAppPathStringError {
    #[error("opaque fallback path URI `{path}` cannot be recovered as a native path")]
    OpaqueFallback { path: String },
    #[error("path URI `{path}` cannot be rendered using {convention} path syntax")]
    IncompatibleConvention {
        path: String,
        convention: PathConvention,
    },
    #[error(
        "path `{path}` is not absolute{convention}",
        convention = .convention.map(|convention| format!(" using {convention} path syntax")).unwrap_or_default()
    )]
    InvalidNativePath {
        path: String,
        convention: Option<PathConvention>,
    },
}

#[cfg(test)]
#[path = "api_path_string_tests.rs"]
mod tests;

use crate::PathUri;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use thiserror::Error;
use ts_rs::TS;

/// Path syntax used to render a [`PathUri`] as an operating-system path.
///
/// This describes path grammar rather than a specific operating system because
/// Linux and macOS share the POSIX representation relevant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum PathConvention {
    Posix,
    Windows,
}

impl PathConvention {
    /// Returns the path convention used by the current process.
    #[cfg(windows)]
    pub const fn native() -> Self {
        Self::Windows
    }

    /// Returns the path convention used by the current process.
    #[cfg(unix)]
    pub const fn native() -> Self {
        Self::Posix
    }
}

impl fmt::Display for PathConvention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Posix => f.write_str("POSIX"),
            Self::Windows => f.write_str("Windows"),
        }
    }
}

/// A UTF-8 path rendered using an explicitly selected native path convention.
///
/// "Native" refers to the supplied [`PathConvention`], which may be foreign to
/// the operating system running this process. The inner string is private so
/// path-producing code must render through [`Self::from_path_uri`] rather than
/// accidentally applying the current host's path rules. Opaque fallback paths
/// are decoded using the supplied convention and converted to UTF-8 lossily at
/// this API boundary because the value is serialized as a JSON string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, TS)]
#[ts(type = "string")]
pub struct ApiPathString(String);

impl ApiPathString {
    /// Wraps an API path string without interpreting it using the current
    /// host's path rules.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Converts an absolute path to a [`PathUri`] before rendering it using the
    /// requested native path convention.
    pub fn from_abs_path(
        path: &AbsolutePathBuf,
        convention: PathConvention,
    ) -> Result<Self, ApiPathStringError> {
        Self::from_path_uri(&PathUri::from_abs_path(path), convention)
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
    ) -> Result<Self, ApiPathStringError> {
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
    pub fn to_path_uri(&self, convention: PathConvention) -> Result<PathUri, ApiPathStringError> {
        let path = match convention {
            PathConvention::Posix => parse_posix_path(&self.0),
            PathConvention::Windows => parse_windows_path(&self.0),
        };
        path.ok_or_else(|| ApiPathStringError::InvalidNativePath {
            path: self.0.clone(),
            convention,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

fn parse_posix_path(path: &str) -> Option<PathUri> {
    let path = path.strip_prefix('/')?;
    if path.contains('\0') {
        return Some(PathUri::from_opaque_path_bytes(
            format!("/{path}").as_bytes(),
        ));
    }
    path_uri_from_segments(/*host*/ None, path.split('/'))
}

fn parse_windows_path(path: &str) -> Option<PathUri> {
    let bytes = path.as_bytes();
    let uses_namespace = matches!(
        bytes,
        [first, second, namespace @ (b'.' | b'?'), separator, ..]
            if is_windows_separator_byte(*first)
                && is_windows_separator_byte(*second)
                && is_windows_separator_byte(*separator)
                && matches!(*namespace, b'.' | b'?')
    );
    if uses_namespace || path.contains('\0') {
        return Some(windows_opaque_path_uri(path));
    }

    if matches!(
        bytes,
        [drive, b':', separator, ..]
            if drive.is_ascii_alphabetic() && is_windows_separator_byte(*separator)
    ) {
        return path_uri_from_segments(
            /*host*/ None,
            std::iter::once(&path[..2]).chain(path[3..].split(is_windows_separator_char)),
        );
    }

    if matches!(bytes, [first, second, ..]
        if is_windows_separator_byte(*first) && is_windows_separator_byte(*second))
    {
        let mut components = path[2..].split(is_windows_separator_char);
        let host = components.next().filter(|host| !host.is_empty())?;
        let share = components.next().filter(|share| !share.is_empty())?;
        return path_uri_from_segments(Some(host), std::iter::once(share).chain(components))
            .or_else(|| Some(windows_opaque_path_uri(path)));
    }

    None
}

pub(crate) fn resolve_native_path(
    base: &PathUri,
    path: &str,
    convention: PathConvention,
) -> Result<PathUri, ApiPathStringError> {
    if path.is_empty() {
        return Ok(base.clone());
    }

    match convention {
        PathConvention::Posix => {
            if path.starts_with('/') {
                return parse_resolved_posix_path(path);
            }
            reject_opaque_base(base)?;
            let base = render_posix_path(base)?;
            parse_resolved_posix_path(&format!("{}/{path}", base.trim_end_matches('/')))
        }
        PathConvention::Windows => resolve_windows_path(base, path),
    }
}

fn resolve_windows_path(base: &PathUri, path: &str) -> Result<PathUri, ApiPathStringError> {
    if is_absolute_windows_path(path) {
        return parse_resolved_windows_path(path);
    }

    reject_opaque_base(base)?;
    let base = render_windows_path(base)?;
    let path = if path.starts_with(['\\', '/']) {
        let root_end = windows_root_end(&base);
        format!(
            "{}\\{}",
            &base[..root_end],
            path.trim_start_matches(['\\', '/'])
        )
    } else {
        format!("{}\\{path}", base.trim_end_matches(['\\', '/']))
    };
    parse_resolved_windows_path(&path)
}

fn parse_resolved_posix_path(path: &str) -> Result<PathUri, ApiPathStringError> {
    let Some(path) = path.strip_prefix('/') else {
        return Err(invalid_native_path(path, PathConvention::Posix));
    };
    if path.contains('\0') {
        return Err(invalid_native_path(path, PathConvention::Posix));
    }
    build_resolved_path_uri(
        /*host*/ None,
        path.split('/'),
        /*protected_segments*/ 0,
        path,
        PathConvention::Posix,
    )
}

fn parse_resolved_windows_path(path: &str) -> Result<PathUri, ApiPathStringError> {
    if path.contains('\0') {
        return Err(invalid_native_path(path, PathConvention::Windows));
    }

    if let Some(rest) = path.strip_prefix(r"\\").or_else(|| path.strip_prefix("//")) {
        let mut segments = rest.split(is_windows_separator_char);
        let Some(host) = segments.next().filter(|host| !host.is_empty()) else {
            return Err(invalid_native_path(path, PathConvention::Windows));
        };
        let Some(share) = segments.next().filter(|share| !share.is_empty()) else {
            return Err(invalid_native_path(path, PathConvention::Windows));
        };
        return build_resolved_path_uri(
            Some(host),
            std::iter::once(share).chain(segments),
            /*protected_segments*/ 1,
            path,
            PathConvention::Windows,
        );
    }

    let bytes = path.as_bytes();
    if !matches!(
        bytes,
        [drive, b':', separator, ..]
            if drive.is_ascii_alphabetic() && is_windows_separator_byte(*separator)
    ) {
        return Err(invalid_native_path(path, PathConvention::Windows));
    }
    build_resolved_path_uri(
        /*host*/ None,
        std::iter::once(&path[..2]).chain(path[3..].split(is_windows_separator_char)),
        /*protected_segments*/ 1,
        path,
        PathConvention::Windows,
    )
}

fn build_resolved_path_uri<'a>(
    host: Option<&str>,
    segments: impl IntoIterator<Item = &'a str>,
    protected_segments: usize,
    native_path: &str,
    convention: PathConvention,
) -> Result<PathUri, ApiPathStringError> {
    let segments = segments.into_iter().collect::<Vec<_>>();
    let preserve_trailing_separator = segments
        .last()
        .is_some_and(|segment| matches!(*segment, "" | "." | ".."));
    let mut normalized_segments = Vec::with_capacity(segments.len());
    for segment in segments {
        match segment {
            "" | "." => {}
            ".." => {
                if normalized_segments.len() > protected_segments {
                    normalized_segments.pop();
                }
            }
            _ => {
                let is_drive = convention == PathConvention::Windows
                    && host.is_none()
                    && normalized_segments.is_empty()
                    && is_windows_drive(segment);
                if convention == PathConvention::Windows
                    && !is_drive
                    && !is_valid_windows_component(segment)
                {
                    return Err(invalid_native_path(native_path, convention));
                }
                normalized_segments.push(segment);
            }
        }
    }
    if normalized_segments.len() < protected_segments {
        return Err(invalid_native_path(native_path, convention));
    }
    if preserve_trailing_separator {
        normalized_segments.push("");
    }

    path_uri_from_segments(host, normalized_segments.into_iter())
        .ok_or_else(|| invalid_native_path(native_path, convention))
}

fn is_absolute_windows_path(path: &str) -> bool {
    path.strip_prefix(r"\\")
        .or_else(|| path.strip_prefix("//"))
        .is_some()
        || matches!(
            path.as_bytes(),
            [drive, b':', separator, ..]
                if drive.is_ascii_alphabetic() && is_windows_separator_byte(*separator)
        )
}

fn windows_root_end(path: &str) -> usize {
    if path.starts_with(r"\\") {
        path.match_indices('\\')
            .nth(3)
            .map_or(path.len(), |(index, _)| index)
    } else {
        2
    }
}

fn is_windows_drive(component: &str) -> bool {
    let bytes = component.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_valid_windows_component(component: &str) -> bool {
    !component
        .chars()
        .any(|character| character <= '\u{1f}' || r#"<>:"/\|?*"#.contains(character))
        && !component.ends_with([' ', '.'])
        && !is_reserved_windows_component(component)
}

fn is_reserved_windows_component(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    let stem = stem.to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

fn reject_opaque_base(path: &PathUri) -> Result<(), ApiPathStringError> {
    if path.opaque_fallback_bytes().is_some() {
        return Err(ApiPathStringError::OpaqueFallback {
            path: path.to_string(),
        });
    }
    Ok(())
}

fn invalid_native_path(path: &str, convention: PathConvention) -> ApiPathStringError {
    ApiPathStringError::InvalidNativePath {
        path: path.to_string(),
        convention,
    }
}

fn path_uri_from_segments<'a>(
    host: Option<&str>,
    segments: impl Iterator<Item = &'a str>,
) -> Option<PathUri> {
    let mut url = url::Url::parse("file:///").ok()?;
    if let Some(host) = host {
        url.set_host(Some(host)).ok()?;
    }
    {
        let mut url_segments = url.path_segments_mut().ok()?;
        url_segments.clear();
        for segment in segments {
            url_segments.push(segment);
        }
    }
    PathUri::try_from(url).ok()
}

fn windows_opaque_path_uri(path: &str) -> PathUri {
    let path_bytes = path
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    PathUri::from_opaque_path_bytes(&path_bytes)
}

fn is_windows_separator_char(character: char) -> bool {
    matches!(character, '\\' | '/')
}

fn is_windows_separator_byte(character: u8) -> bool {
    matches!(character, b'\\' | b'/')
}

fn render_opaque_fallback(
    path: &PathUri,
    path_bytes: &[u8],
    convention: PathConvention,
) -> Result<String, ApiPathStringError> {
    let rendered = match convention {
        PathConvention::Posix if path_bytes.starts_with(b"/") => {
            Some(String::from_utf8_lossy(path_bytes).into_owned())
        }
        PathConvention::Windows => render_windows_opaque_fallback(path_bytes),
        PathConvention::Posix => None,
    };
    rendered.ok_or_else(|| ApiPathStringError::OpaqueFallback {
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

impl fmt::Display for ApiPathString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ApiPathString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl JsonSchema for ApiPathString {
    fn schema_name() -> String {
        "ApiPathString".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(generator)
    }
}

fn render_posix_path(path: &PathUri) -> Result<String, ApiPathStringError> {
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

fn render_windows_path(path: &PathUri) -> Result<String, ApiPathStringError> {
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

fn incompatible_convention(path: &PathUri, convention: PathConvention) -> ApiPathStringError {
    ApiPathStringError::IncompatibleConvention {
        path: path.to_string(),
        convention,
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApiPathStringError {
    #[error("opaque fallback path URI `{path}` cannot be recovered as a native path")]
    OpaqueFallback { path: String },
    #[error("path URI `{path}` cannot be rendered using {convention} path syntax")]
    IncompatibleConvention {
        path: String,
        convention: PathConvention,
    },
    #[error("path `{path}` is not absolute using {convention} path syntax")]
    InvalidNativePath {
        path: String,
        convention: PathConvention,
    },
}

#[cfg(test)]
#[path = "api_path_string_tests.rs"]
mod tests;

//! Typed, immutable `file:` URIs with cross-platform path inspection.
//!
//! See [`PathUri`] for scheme, normalization, and serialization behavior.

use base64::Engine;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use ts_rs::TS;
use url::Url;

mod api_path_string;

pub use api_path_string::LegacyAppPathString;
pub use api_path_string::LegacyAppPathStringError;

pub const FILE_SCHEME: &str = "file";
const BAD_PATH_URI_PREFIX: &str = "file:///%00/bad/path/";

/// An immutable, cross-platform representation of a `file:` URI.
///
/// Only the `file:` scheme is currently accepted. Construction validates the
/// URL, and the URI cannot be mutated after construction. [`Self::basename`],
/// [`Self::parent`], and [`Self::join`] operate on URI path segments without
/// interpreting them using the operating system running Codex. Fallback URIs
/// created by [`Self::from_abs_path`] are opaque to these lexical operations.
///
/// `file:` paths retain their URI spelling so they can be parsed independently
/// of the current host. A local POSIX `file:` URI can also retain
/// percent-encoded non-UTF-8 bytes for lossless native round trips.
///
/// Like [VS Code resources], path operations use `/` URI separators on every
/// host. Lexical path operations preserve a URL authority without interpreting
/// Windows drive or UNC roots from path text. Native path normalization,
/// filesystem aliases, symlinks, case sensitivity, and Unicode normalization
/// are not resolved.
///
/// Serde represents a `PathUri` as its canonical URI string. Deserialization
/// accepts only valid `file:` URI strings. These strings round-trip through
/// their canonical URL form, including encoded non-UTF-8 path bytes.
///
/// [VS Code resources]: https://github.com/microsoft/vscode/blob/main/src/vs/base/common/resources.ts
#[derive(Clone, Debug, PartialEq, Eq, Hash, TS)]
#[ts(type = "string")]
pub struct PathUri(Url);

impl PathUri {
    /// Parses and validates a `file:` URI.
    pub fn parse(uri: &str) -> Result<Self, PathUriParseError> {
        Url::parse(uri)?.try_into()
    }

    /// Converts an absolute path on the current host to a `file:` URI.
    ///
    /// Paths without a valid URI representation are replaced by
    /// `file:///%00/bad/path/<base64>`, where `<base64>` is the URL-safe, unpadded
    /// encoding of the original path (Unix bytes or Windows UTF-16LE). This
    /// includes paths containing nulls and, on Windows, unsupported prefix
    /// kinds such as device and generic verbatim namespaces, non-Unicode path
    /// or UNC components, UNC server names that are not valid URL hosts, and
    /// `\\localhost` UNC paths whose server component must remain explicit.
    /// The encoded null reserves a URI namespace that cannot collide with a
    /// real path on Unix or Windows.
    pub fn from_abs_path(path: &AbsolutePathBuf) -> Self {
        #[cfg(windows)]
        let preserve_localhost_unc = matches!(
            path.as_path().components().next(),
            Some(std::path::Component::Prefix(prefix))
                if matches!(
                    prefix.kind(),
                    std::path::Prefix::UNC(server, _)
                        if server.to_string_lossy().eq_ignore_ascii_case("localhost")
                )
        );
        #[cfg(not(windows))]
        let preserve_localhost_unc = false;

        if !preserve_localhost_unc
            && let Ok(url) = Url::from_file_path(path.as_path())
            && let Ok(uri) = Self::try_from(url)
        {
            return uri;
        }

        #[cfg(unix)]
        let path_bytes = {
            use std::os::unix::ffi::OsStrExt;
            path.as_path().as_os_str().as_bytes().to_vec()
        };
        #[cfg(windows)]
        let path_bytes = {
            use std::os::windows::ffi::OsStrExt;
            path.as_path()
                .as_os_str()
                .encode_wide()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        };
        Self::from_opaque_path_bytes(&path_bytes)
    }

    /// Parses an absolute native path using the specified path convention.
    pub(crate) fn from_absolute_native_path(
        path: &str,
        convention: PathConvention,
    ) -> Option<Self> {
        match convention {
            PathConvention::Posix => parse_posix_path(path),
            PathConvention::Windows => parse_windows_path(path),
        }
    }

    fn from_opaque_path_bytes(path_bytes: &[u8]) -> Self {
        let encoded_path = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(path_bytes);
        let Ok(uri) = Self::parse(&format!("{BAD_PATH_URI_PREFIX}{encoded_path}")) else {
            unreachable!("URL-safe base64 always produces a valid fallback path URI");
        };
        uri
    }

    /// Converts a path on the current host to a `file:` URI.
    ///
    /// Relative paths are reported as invalid input. Absolute paths without a
    /// valid URI representation use the fallback documented on
    /// [`Self::from_abs_path`].
    pub fn from_host_native_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = AbsolutePathBuf::from_absolute_path_checked(path)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        Ok(Self::from_abs_path(&path))
    }

    /// Returns the percent-encoded URI path.
    ///
    /// The URL authority is not included. For example,
    /// `file://server/share/file.rs` has the path `/share/file.rs`.
    pub fn encoded_path(&self) -> &str {
        self.0.path()
    }

    fn opaque_fallback_bytes(&self) -> Option<Vec<u8>> {
        decode_bad_path_uri(&self.0)
    }

    /// Infers the native path convention represented by this URI.
    ///
    /// A URI authority is treated as a Windows UNC host, and a leading
    /// drive-letter segment such as `C:` is treated as a Windows drive. All
    /// other ordinary file URIs are treated as POSIX paths. This deliberately
    /// classifies `file:///C:/src` as Windows even though `/C:/src` is also a
    /// valid POSIX path. In practice, POSIX paths with a drive-shaped first
    /// component are rare enough that recognizing foreign Windows paths is the
    /// more useful default.
    ///
    /// Opaque fallback URIs are inspected for an absolute POSIX byte prefix or
    /// an absolute Windows UTF-16LE prefix. `None` is returned when their
    /// payload does not identify either convention.
    ///
    /// TODO(anp): Once `PathUri` carries an environment identifier, prefer the
    /// environment's declared convention over this spelling-based heuristic.
    pub fn infer_path_convention(&self) -> Option<PathConvention> {
        if let Some(path_bytes) = self.opaque_fallback_bytes() {
            return infer_opaque_path_convention(&path_bytes);
        }
        if self.0.host_str().is_some() {
            return Some(PathConvention::Windows);
        }

        let has_windows_drive = self
            .0
            .path_segments()
            .and_then(|mut segments| segments.find(|segment| !segment.is_empty()))
            .is_some_and(is_windows_drive_uri_segment);
        if has_windows_drive {
            Some(PathConvention::Windows)
        } else {
            Some(PathConvention::Posix)
        }
    }

    /// Renders this URI using the native path syntax inferred from its shape.
    ///
    /// This is independent of the current host: a Windows URI renders with
    /// Windows separators on every host. If the convention cannot be inferred
    /// or the URI cannot be represented using that convention, the canonical
    /// URI string is returned instead.
    pub fn inferred_native_path_string(&self) -> String {
        self.infer_path_convention()
            .and_then(|convention| LegacyAppPathString::from_path_uri(self, convention).ok())
            .map(LegacyAppPathString::into_string)
            .unwrap_or_else(|| self.to_string())
    }

    /// Returns the decoded final URI path segment, or `None` for the URI root
    /// or an opaque fallback URI created by [`Self::from_abs_path`].
    ///
    /// If the segment contains non-UTF-8 encoded bytes, its percent-encoded
    /// spelling is returned instead.
    pub fn basename(&self) -> Option<String> {
        if decode_bad_path_uri(&self.0).is_some() {
            return None;
        }

        self.0
            .path_segments()?
            .rfind(|segment| !segment.is_empty())
            .map(decode_uri_path)
    }

    /// Renders this URI as a path-flavored string using its inferred convention.
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.inferred_native_path_string())
    }

    /// Returns the lexical parent without crossing the inferred native path root.
    ///
    /// POSIX `/`, Windows drive roots, Windows UNC share roots, and opaque fallback
    /// URIs created by [`Self::from_abs_path`] have no parent.
    pub fn parent(&self) -> Option<Self> {
        if decode_bad_path_uri(&self.0).is_some() {
            return None;
        }

        let convention = self.infer_path_convention()?;
        // In URI form, both a Windows drive root (`file:///C:`) and a UNC share root
        // (`file://server/share`) retain one non-empty path segment. Keep that segment as the
        // anchor so parent traversal cannot produce a URI that is not an absolute Windows path.
        let anchor_depth = usize::from(convention == PathConvention::Windows);
        let depth = self
            .0
            .path_segments()?
            .filter(|segment| !segment.is_empty())
            .count();
        if depth <= anchor_depth {
            return None;
        }
        let mut url = self.0.clone();
        {
            let mut segments = match url.path_segments_mut() {
                Ok(segments) => segments,
                Err(()) => unreachable!("validated file URLs support hierarchical path segments"),
            };
            segments.pop_if_empty().pop();
        }
        Some(Self(url))
    }

    /// Returns this URI and each lexical parent up to its inferred native path root.
    pub fn ancestors(&self) -> impl Iterator<Item = Self> {
        std::iter::successors(Some(self.clone()), Self::parent)
    }

    /// Returns true when this URI is lexically equal to or below `base`.
    ///
    /// Containment is computed using URI authority and path-segment boundaries,
    /// without consulting the host filesystem. Percent-encoded native path
    /// separators fail closed because native path conversion may interpret them
    /// as segment boundaries. Opaque fallback URIs created by
    /// [`Self::from_abs_path`] only contain themselves.
    pub fn starts_with(&self, base: &Self) -> bool {
        if self == base {
            return true;
        }
        if decode_bad_path_uri(&self.0).is_some() || decode_bad_path_uri(&base.0).is_some() {
            return false;
        }
        if self.0.host_str() != base.0.host_str() {
            return false;
        }

        let Some(path_segments) = containment_path_segments(
            &self.0,
            self.infer_path_convention()
                .unwrap_or(PathConvention::Posix),
        ) else {
            return false;
        };
        let Some(base_segments) = containment_path_segments(
            &base.0,
            base.infer_path_convention()
                .unwrap_or(PathConvention::Posix),
        ) else {
            return false;
        };
        path_segments.starts_with(&base_segments)
    }

    /// Lexically resolves native absolute or relative path text against this URI.
    ///
    /// Path text is interpreted using the POSIX or Windows convention inferred
    /// from the base URI. An absolute path replaces the base URI's path, while a
    /// relative path is appended lexically. Windows root-relative paths retain
    /// the base drive or UNC share, while drive-relative paths are rejected.
    /// Empty and `.` segments are ignored, while `..` removes one segment
    /// without escaping the POSIX root, Windows drive, or UNC share. Literal
    /// `%`, `?`, and `#` characters are percent-encoded as filename text. Paths
    /// containing a null character are rejected because they cannot be safely
    /// converted to native paths.
    /// Opaque fallback URIs created by [`Self::from_abs_path`] reject non-empty
    /// joins.
    pub fn join(&self, path: &str) -> Result<Self, PathUriParseError> {
        if path.contains('\0') {
            return Err(PathUriParseError::InvalidFileUriPath {
                path: path.to_string(),
            });
        }
        if path.is_empty() {
            return Ok(self.clone());
        }
        let convention =
            self.infer_path_convention()
                .ok_or_else(|| PathUriParseError::InvalidFileUriPath {
                    path: self.to_string(),
                })?;
        // An absolute native path is already fully resolved, so replace the base URI's main path
        // instead of appending it.
        if let Some(absolute) = Self::from_absolute_native_path(path, convention) {
            return Ok(absolute);
        }
        let path_bytes = path.as_bytes();
        if convention == PathConvention::Windows
            && matches!(path_bytes, [drive, b':', ..] if drive.is_ascii_alphabetic())
        {
            return Err(PathUriParseError::InvalidFileUriPath {
                path: path.to_string(),
            });
        }
        if decode_bad_path_uri(&self.0).is_some() {
            return Err(PathUriParseError::InvalidFileUriPath {
                path: self.to_string(),
            });
        }

        let mut url = self.0.clone();
        let anchor_depth = usize::from(convention == PathConvention::Windows);
        let mut depth = url
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).count())
            .unwrap_or_default();
        let windows_root_relative = convention == PathConvention::Windows
            && matches!(path_bytes, [b'\\' | b'/', rest @ ..] if !matches!(rest, [b'\\' | b'/', ..]));
        {
            let Ok(mut segments) = url.path_segments_mut() else {
                unreachable!("validated file URLs support hierarchical path segments");
            };
            segments.pop_if_empty();
            if windows_root_relative {
                while depth > anchor_depth {
                    segments.pop();
                    depth -= 1;
                }
            }
            let path = match convention {
                PathConvention::Posix => path.to_string(),
                PathConvention::Windows => path.replace('\\', "/"),
            };
            for component in path.split('/') {
                match component {
                    "" | "." => {}
                    ".." => {
                        if depth > anchor_depth {
                            segments.pop();
                            depth -= 1;
                        }
                    }
                    component => {
                        segments.push(component);
                        depth += 1;
                    }
                }
            }
        }
        Self::try_from(url)
    }

    /// Converts this file URI to a path using the current host's path rules.
    ///
    /// The URI's inferred path convention must match the current host. Conversion should succeed
    /// when the URI was created from an [`AbsolutePathBuf`] on the current host, including fallback
    /// URIs created by [`Self::from_abs_path`]. Foreign conventions are rejected rather than being
    /// projected onto a syntactically valid but unrelated host path.
    pub fn to_abs_path(&self) -> io::Result<AbsolutePathBuf> {
        if self.infer_path_convention() != Some(PathConvention::native()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                PathUriParseError::InvalidFileUriPath {
                    path: self.to_string(),
                },
            ));
        }
        if let Some(path_bytes) = decode_bad_path_uri(&self.0) {
            #[cfg(unix)]
            let decoded_path = {
                use std::os::unix::ffi::OsStringExt;
                Some(std::path::PathBuf::from(std::ffi::OsString::from_vec(
                    path_bytes,
                )))
            };
            #[cfg(windows)]
            let decoded_path = {
                use std::os::windows::ffi::OsStringExt;
                path_bytes.len().is_multiple_of(2).then(|| {
                    let path_wide = path_bytes
                        .chunks_exact(2)
                        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                        .collect::<Vec<_>>();
                    std::path::PathBuf::from(std::ffi::OsString::from_wide(&path_wide))
                })
            };
            if let Some(decoded_path) = decoded_path
                && let Ok(path) = AbsolutePathBuf::from_absolute_path_checked(decoded_path)
                && Self::from_abs_path(&path).eq(self)
            {
                return Ok(path);
            }

            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                PathUriParseError::InvalidFileUriPath {
                    path: self.to_string(),
                },
            ));
        }

        let path = self.0.to_file_path().map_err(|()| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                PathUriParseError::InvalidFileUriPath {
                    path: self.to_string(),
                },
            )
        })?;
        AbsolutePathBuf::from_absolute_path_checked(path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                PathUriParseError::InvalidFileUriPath {
                    path: self.to_string(),
                },
            )
        })
    }

    /// Returns a clone of the canonical URL.
    pub fn to_url(&self) -> Url {
        self.0.clone()
    }
}

impl TryFrom<Url> for PathUri {
    type Error = PathUriParseError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        if url.scheme() != FILE_SCHEME {
            return Err(PathUriParseError::UnsupportedScheme(
                url.scheme().to_string(),
            ));
        }
        validate_file_url(&url)?;
        let url = without_localhost_authority(url);
        Ok(Self(url))
    }
}

impl TryFrom<String> for PathUri {
    type Error = PathUriParseError;

    fn try_from(uri: String) -> Result<Self, Self::Error> {
        Self::parse(&uri)
    }
}

impl From<AbsolutePathBuf> for PathUri {
    fn from(p: AbsolutePathBuf) -> Self {
        Self::from_abs_path(&p)
    }
}

impl<'de> Deserialize<'de> for PathUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl FromStr for PathUri {
    type Err = PathUriParseError;

    fn from_str(uri: &str) -> Result<Self, Self::Err> {
        Self::parse(uri)
    }
}

impl fmt::Display for PathUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for PathUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl JsonSchema for PathUri {
    fn schema_name() -> String {
        "PathUri".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(generator)
    }
}

/// Removes the local `localhost` alias while retaining non-local UNC authority.
fn without_localhost_authority(mut url: Url) -> Url {
    if url.host_str() == Some("localhost") {
        let Ok(()) = url.set_host(None) else {
            unreachable!("validated file URLs can remove a localhost authority");
        };
    }
    url
}

/// Percent-decodes a URI path when it is valid UTF-8.
///
/// `file:` URLs may contain encoded non-UTF-8 bytes. In that case the encoded
/// spelling remains available for lexical inspection while the original `Url`
/// is retained for lossless native conversion.
fn decode_uri_path(path: &str) -> String {
    urlencoding::decode(path)
        .map(std::borrow::Cow::into_owned)
        .unwrap_or_else(|_| path.to_string())
}

/// Returns the original platform path bytes from a canonical bad-path URI.
fn decode_bad_path_uri(url: &Url) -> Option<Vec<u8>> {
    let encoded_path = url.as_str().strip_prefix(BAD_PATH_URI_PREFIX)?;
    if encoded_path.is_empty() || encoded_path.contains('/') {
        return None;
    }

    let path_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_path)
        .ok()?;
    (base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&path_bytes) == encoded_path)
        .then_some(path_bytes)
}

fn is_windows_drive_uri_segment(segment: &str) -> bool {
    matches!(segment.as_bytes(), [drive, b':'] if drive.is_ascii_alphabetic())
}

fn containment_path_segments(url: &Url, convention: PathConvention) -> Option<Vec<&str>> {
    let segments = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    (!segments.iter().any(|segment| {
        urlencoding::decode_binary(segment.as_bytes())
            .iter()
            .any(|byte| *byte == b'/' || (convention == PathConvention::Windows && *byte == b'\\'))
    }))
    .then_some(segments)
}

fn infer_opaque_path_convention(path_bytes: &[u8]) -> Option<PathConvention> {
    if path_bytes.starts_with(b"/") {
        return Some(PathConvention::Posix);
    }
    if !path_bytes.len().is_multiple_of(2) {
        return None;
    }

    let mut path_wide = path_bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
    let first = path_wide.next()?;
    let second = path_wide.next()?;
    let has_drive = u8::try_from(first).is_ok_and(|drive| drive.is_ascii_alphabetic())
        && second == u16::from(b':');
    let has_unc_prefix = first == u16::from(b'\\') && second == u16::from(b'\\');
    (has_drive || has_unc_prefix).then_some(PathConvention::Windows)
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

fn path_uri_from_segments<'a>(
    host: Option<&str>,
    segments: impl Iterator<Item = &'a str>,
) -> Option<PathUri> {
    let mut url = Url::parse("file:///").ok()?;
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

pub(crate) fn is_windows_separator_byte(character: u8) -> bool {
    matches!(character, b'\\' | b'/')
}

/// Rejects URI metadata that has no defined meaning for `file:` URIs.
fn validate_common_known_uri(url: &Url) -> Result<(), PathUriParseError> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(PathUriParseError::CredentialsNotAllowed);
    }
    if url.port().is_some() {
        return Err(PathUriParseError::PortNotAllowed);
    }
    if url.query().is_some() {
        return Err(PathUriParseError::QueryNotAllowed);
    }
    if url.fragment().is_some() {
        return Err(PathUriParseError::FragmentNotAllowed);
    }
    Ok(())
}

/// Applies the common URI checks plus `file:` path-byte restrictions.
fn validate_file_url(url: &Url) -> Result<(), PathUriParseError> {
    validate_common_known_uri(url)?;
    // `Url` accepts `%00`, but native path APIs use null as a terminator and
    // `Url::to_file_path` cannot represent a decoded null byte.
    if urlencoding::decode_binary(url.path().as_bytes()).contains(&0)
        && decode_bad_path_uri(url).is_none()
    {
        return Err(PathUriParseError::InvalidFileUriPath {
            path: url.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathUriParseError {
    #[error("invalid URI: {0}")]
    InvalidUri(#[from] url::ParseError),
    #[error("unsupported path URI scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("'{path}' is invalid on '{os}'", os = std::env::consts::OS)]
    InvalidFileUriPath { path: String },
    #[error("credentials are not allowed in path URIs")]
    CredentialsNotAllowed,
    #[error("ports are not allowed in path URIs")]
    PortNotAllowed,
    #[error("query parameters are not allowed in path URIs")]
    QueryNotAllowed,
    #[error("fragments are not allowed in path URIs")]
    FragmentNotAllowed,
    #[error("path `{0}` must be relative when joining a path URI")]
    JoinPathMustBeRelative(String),
}

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

    /// Splits absolute or relative native path text into lexical segments.
    ///
    /// This does not validate the path or require it to be absolute. POSIX paths split on `/`,
    /// while Windows paths split on both `\\` and `/`. Empty segments are retained.
    pub fn path_segments(self, path: &str) -> impl DoubleEndedIterator<Item = &str> {
        path.split(move |character| match self {
            Self::Posix => character == '/',
            Self::Windows => matches!(character, '/' | '\\'),
        })
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

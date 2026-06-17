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
use std::str::FromStr;
use thiserror::Error;
use ts_rs::TS;
use url::Url;

mod api_path_string;

pub use api_path_string::LegacyAppPathString;
pub use api_path_string::LegacyAppPathStringError;
pub use api_path_string::PathConvention;

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
/// also accepts an absolute native path for compatibility with fields that
/// previously used [`AbsolutePathBuf`]; relative paths are rejected. Valid
/// `file:` strings round-trip through their canonical URL form, including
/// encoded non-UTF-8 path bytes, but conversion to a native path remains
/// host-dependent as described by [RFC 8089].
///
/// [RFC 8089]: https://www.rfc-editor.org/rfc/rfc8089.html
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
    /// or UNC components, and UNC server names that are not valid URL hosts.
    /// The encoded null reserves a URI namespace that cannot collide with a
    /// real path on Unix or Windows.
    pub fn from_abs_path(path: &AbsolutePathBuf) -> Self {
        if let Ok(url) = Url::from_file_path(path.as_path())
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
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
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

    /// Returns the parent URI, or `None` for the URI root or an opaque fallback
    /// URI created by [`Self::from_abs_path`].
    pub fn parent(&self) -> Option<Self> {
        if self.encoded_path() == "/" || decode_bad_path_uri(&self.0).is_some() {
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

    /// Lexically joins a relative URI path onto this URI.
    ///
    /// Empty and `.` segments are ignored, while `..` removes one segment
    /// without escaping the URI root. Literal `%`, `?`, and `#` characters are
    /// percent-encoded as filename text. Paths containing a null character are
    /// rejected because they cannot be safely converted to native paths.
    /// Opaque fallback URIs created by [`Self::from_abs_path`] reject non-empty
    /// joins.
    pub fn join(&self, path: &str) -> Result<Self, PathUriParseError> {
        if path.starts_with('/') {
            return Err(PathUriParseError::JoinPathMustBeRelative(path.to_string()));
        }
        if path.contains('\0') {
            return Err(PathUriParseError::InvalidFileUriPath {
                path: path.to_string(),
            });
        }
        if path.is_empty() {
            return Ok(self.clone());
        }
        if decode_bad_path_uri(&self.0).is_some() {
            return Err(PathUriParseError::InvalidFileUriPath {
                path: self.to_string(),
            });
        }

        let mut url = self.0.clone();
        {
            let Ok(mut segments) = url.path_segments_mut() else {
                unreachable!("validated file URLs support hierarchical path segments");
            };
            segments.pop_if_empty();
            for component in path.split('/') {
                match component {
                    "" | "." => {}
                    ".." => {
                        segments.pop();
                    }
                    component => {
                        segments.push(component);
                    }
                }
            }
        }
        Self::try_from(url)
    }

    /// Converts this file URI to a path using the current host's path rules.
    ///
    /// Conversion should succeed when the URI was created from an
    /// [`AbsolutePathBuf`] on the current host, including fallback URIs created
    /// by [`Self::from_abs_path`]. It may fail when the URI came from a different
    /// operating system and its `file:` URI form cannot be represented using
    /// the current host's path rules, such as a UNC authority on POSIX or a
    /// POSIX root on Windows. Because a `file:` URI does not record its source
    /// operating system, callers should only use this method when the URI is
    /// known to identify a path on the current host.
    pub fn to_abs_path(&self) -> io::Result<AbsolutePathBuf> {
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

impl<'de> Deserialize<'de> for PathUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let unsupported_scheme = match Url::parse(&value) {
            Ok(url) => match Self::try_from(url) {
                Ok(uri) => return Ok(uri),
                // `Url` parses a Windows drive prefix such as `C:\` as the
                // scheme `c`. Give any unsupported URI one chance to satisfy
                // the native absolute-path invariant before reporting it.
                Err(error @ PathUriParseError::UnsupportedScheme(_)) => Some(error),
                Err(error) => return Err(serde::de::Error::custom(error)),
            },
            Err(url::ParseError::RelativeUrlWithoutBase) => None,
            Err(error) => {
                return Err(serde::de::Error::custom(PathUriParseError::InvalidUri(
                    error,
                )));
            }
        };

        let path = AbsolutePathBuf::from_absolute_path_checked(value).map_err(|path_error| {
            serde::de::Error::custom(
                unsupported_scheme
                    .map_or_else(|| path_error.to_string(), |error| error.to_string()),
            )
        })?;
        Ok(Self::from_abs_path(&path))
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

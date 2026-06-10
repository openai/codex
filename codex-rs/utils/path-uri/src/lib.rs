//! Typed, immutable `file:` URIs with cross-platform path inspection.
//!
//! See [`PathUri`] for scheme, normalization, and serialization behavior.

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

pub const FILE_SCHEME: &str = "file";

/// An immutable, cross-platform representation of a `file:` URI.
///
/// Only the `file:` scheme is currently accepted. Construction validates the
/// URL, and the URI cannot be mutated after construction. [`Self::basename`],
/// [`Self::parent`], and [`Self::join`] operate on URI path segments without
/// interpreting them using the operating system running Codex.
///
/// `file:` paths retain their URI spelling so they can be parsed independently
/// of the current host. In particular, `/C:/src` remains ambiguous between a
/// Windows drive path and a valid POSIX path until [`Self::to_abs_path`]
/// applies the current host's rules. A local POSIX `file:` URI can also retain
/// percent-encoded non-UTF-8 bytes for lossless native round trips.
///
/// Like [VS Code resources], path operations use `/` URI separators on every
/// host. They preserve a URL authority but do not infer Windows drive or UNC
/// roots from path text. Native path normalization, filesystem aliases,
/// symlinks, case sensitivity, and Unicode normalization are not resolved.
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
    /// On Windows, paths without a URI representation, including `\\.\` device
    /// paths and generic `\\?\` verbatim namespaces, are reported as invalid
    /// input.
    pub fn from_abs_path(path: &AbsolutePathBuf) -> io::Result<Self> {
        let url = Url::from_file_path(path.as_path()).map_err(|()| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                PathUriParseError::InvalidFileUriPath,
            )
        })?;
        Self::try_from(url).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
    }

    /// Converts a path on the current host to a `file:` URI.
    ///
    /// Relative paths and paths without a URI representation are reported as
    /// invalid input.
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = AbsolutePathBuf::from_absolute_path_checked(path)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        Self::from_abs_path(&path)
    }

    /// Returns the percent-encoded URI path.
    ///
    /// The URL authority is not included. For example,
    /// `file://server/share/file.rs` has the path `/share/file.rs`.
    pub fn encoded_path(&self) -> &str {
        self.0.path()
    }

    /// Returns the decoded final URI path segment, or `None` for the URI root.
    ///
    /// If the segment contains non-UTF-8 encoded bytes, its percent-encoded
    /// spelling is returned instead.
    pub fn basename(&self) -> Option<String> {
        self.0
            .path_segments()?
            .rfind(|segment| !segment.is_empty())
            .map(decode_uri_path)
    }

    /// Returns the parent URI, or `None` for the URI root.
    pub fn parent(&self) -> Option<Self> {
        if self.encoded_path() == "/" {
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
    pub fn join(&self, path: &str) -> Result<Self, PathUriParseError> {
        if path.starts_with('/') {
            return Err(PathUriParseError::JoinPathMustBeRelative(path.to_string()));
        }
        if path.contains('\0') {
            return Err(PathUriParseError::InvalidFileUriPath);
        }
        if path.is_empty() {
            return Ok(self.clone());
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
    /// [`AbsolutePathBuf`] on the current host. It may fail when the URI came
    /// from a different operating system and its `file:` URI form cannot be
    /// represented using the current host's path rules, such as a UNC authority
    /// on POSIX or a POSIX root on Windows. Because a `file:` URI does not record
    /// its source operating system, callers should only use this method when the
    /// URI is known to identify a path on the current host.
    pub fn to_abs_path(&self) -> io::Result<AbsolutePathBuf> {
        let path = self.0.to_file_path().map_err(|()| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                PathUriParseError::InvalidFileUriPath,
            )
        })?;
        AbsolutePathBuf::from_absolute_path_checked(path)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
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
        Self::from_abs_path(&path).map_err(serde::de::Error::custom)
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
    if urlencoding::decode_binary(url.path().as_bytes()).contains(&0) {
        return Err(PathUriParseError::InvalidFileUriPath);
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathUriParseError {
    #[error("invalid URI: {0}")]
    InvalidUri(#[from] url::ParseError),
    #[error("unsupported path URI scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("file URI contains an invalid absolute path")]
    InvalidFileUriPath,
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

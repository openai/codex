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
/// Windows drive path and a valid POSIX path until [`Self::to_native_path`]
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
    pub fn from_file_path(path: &AbsolutePathBuf) -> Result<Self, PathUriParseError> {
        let url = Url::from_file_path(path.as_path())
            .map_err(|()| PathUriParseError::PathCannotBeRepresentedAsFileUri)?;
        Self::try_from(url)
    }

    /// Returns `file`.
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    /// Returns the percent-encoded URI path.
    ///
    /// The URL authority is not included. For example,
    /// `file://server/share/file.rs` has the path `/share/file.rs`.
    pub fn path(&self) -> &str {
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
        if self.path() == "/" {
            return None;
        }

        let mut url = self.0.clone();
        let Ok(mut segments) = url.path_segments_mut() else {
            unreachable!("validated file URLs support hierarchical path segments");
        };
        segments.pop_if_empty().pop();
        drop(segments);
        Some(Self(url))
    }

    /// Lexically joins a relative URI path onto this URI.
    ///
    /// Empty and `.` segments are ignored, while `..` removes one segment
    /// without escaping the URI root. Literal `%`, `?`, and `#` characters are
    /// percent-encoded as filename text.
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
        drop(segments);
        Self::try_from(url)
    }

    /// Converts this file URI to a path using the current host's path rules.
    ///
    /// This fails when the URI cannot be represented as an absolute native
    /// path on the current host. Callers should only use this method when the
    /// URI is known to identify a path on that host.
    pub fn to_native_path(&self) -> Result<AbsolutePathBuf, PathUriParseError> {
        let path = self
            .0
            .to_file_path()
            .map_err(|()| PathUriParseError::InvalidFileUriPath)?;
        AbsolutePathBuf::from_absolute_path_checked(path)
            .map_err(|_| PathUriParseError::InvalidFileUriPath)
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
        let url = canonical_file_url(url)?;
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
        if looks_like_uri(&value) {
            return Self::parse(&value).map_err(serde::de::Error::custom);
        }

        let path =
            AbsolutePathBuf::from_absolute_path_checked(value).map_err(serde::de::Error::custom)?;
        Self::from_file_path(&path).map_err(serde::de::Error::custom)
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

/// Serde adapter for fields that still use the legacy native file-path wire format.
///
/// Deserialization accepts either an absolute legacy native path or a [`PathUri`].
/// Serialization emits the current host's native path spelling. New URI-native
/// fields should use [`PathUri`]'s own serde implementation instead.
pub mod legacy_file_path_serde {
    use super::*;

    pub fn serialize<S>(uri: &PathUri, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        uri.to_native_path()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PathUri, D::Error>
    where
        D: Deserializer<'de>,
    {
        PathUri::deserialize(deserializer)
    }
}

/// Removes the local `localhost` alias while retaining non-local UNC authority.
fn canonical_file_url(mut url: Url) -> Result<Url, PathUriParseError> {
    if url.host_str() == Some("localhost") {
        url.set_host(None)
            .map_err(|_| PathUriParseError::InvalidFileUriPath)?;
    }
    Ok(url)
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

/// Detects encoded `/` bytes that would conceal a path-segment boundary.
fn contains_percent_encoded_slash(path: &str) -> bool {
    path.as_bytes()
        .windows(3)
        .any(|bytes| bytes[0] == b'%' && bytes[1] == b'2' && matches!(bytes[2], b'f' | b'F'))
}

/// Detects a percent sign that is not followed by two hexadecimal digits.
fn has_invalid_percent_encoding(path: &str) -> bool {
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        if bytes
            .get(index + 1..index + 3)
            .is_none_or(|digits| !digits.iter().all(u8::is_ascii_hexdigit))
        {
            return true;
        }
        index += 3;
    }
    false
}

/// Rejects URI metadata that has no defined meaning for `file:` URIs.
fn validate_common_known_uri(url: &Url) -> Result<(), PathUriParseError> {
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err(PathUriParseError::CredentialsOrPortNotAllowed);
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
    if has_invalid_percent_encoding(url.path()) || contains_percent_encoded_slash(url.path()) {
        return Err(PathUriParseError::InvalidFileUriPath);
    }
    if urlencoding::decode_binary(url.path().as_bytes()).contains(&0) {
        return Err(PathUriParseError::InvalidFileUriPath);
    }
    Ok(())
}

/// Returns a syntactically valid URI scheme prefix without parsing the URI.
fn uri_scheme(uri: &str) -> Option<&str> {
    let (scheme, _) = uri.split_once(':')?;
    (!scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic()
                || (index > 0 && (byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')))
        }))
    .then_some(scheme)
}

/// Distinguishes URI strings from legacy native paths at the serde boundary.
///
/// A Windows drive prefix resembles a one-letter URI scheme, so an immediately
/// following slash or backslash keeps it in the native-path branch.
fn looks_like_uri(value: &str) -> bool {
    let Some(scheme) = uri_scheme(value) else {
        return false;
    };
    !(scheme.len() == 1
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && value
            .as_bytes()
            .get(2)
            .is_some_and(|separator| matches!(separator, b'/' | b'\\')))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathUriParseError {
    #[error("invalid URI: {0}")]
    InvalidUri(#[from] url::ParseError),
    #[error("unsupported path URI scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("path cannot be represented as a file URI")]
    PathCannotBeRepresentedAsFileUri,
    #[error("file URI contains an invalid absolute path")]
    InvalidFileUriPath,
    #[error("credentials and ports are not allowed in path URIs")]
    CredentialsOrPortNotAllowed,
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

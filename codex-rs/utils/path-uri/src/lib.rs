//! Typed, immutable URIs for local and configured-environment paths.
//!
//! See [`PathUri`] for scheme, normalization, and serialization behavior.

mod environment_path;

use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_environment_id::EnvironmentId;
use codex_utils_environment_id::EnvironmentIdError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use ts_rs::TS;
use url::PathSegmentsMut;
use url::Url;

pub use environment_path::EnvironmentPath;
pub use environment_path::EnvironmentPathError;
pub use environment_path::PathFlavor;

pub const FILE_SCHEME: &str = "file";
pub const CODEX_ENVIRONMENT_SCHEME: &str = "codex-env";

/// An immutable URI that identifies a path on the current host or in a
/// configured Codex environment.
///
/// Only `file:` and `codex-env:` schemes are accepted. Construction validates
/// and caches the scheme-specific components, which keeps [`Self::view`]
/// infallible and exhaustive. The URI cannot be mutated after construction. To
/// perform path operations, use the [`EnvironmentPath`] from [`Self::view`] and
/// construct a new `PathUri` from the resulting path.
///
/// `file:` paths retain URI path spelling so they can be parsed independently
/// of the current host. In particular, `/C:/src` remains ambiguous between a
/// Windows drive path and a valid POSIX path until
/// [`FileUriView::to_native_path`] applies the current host's rules. A local
/// POSIX `file:` URI can also retain percent-encoded non-UTF-8 bytes for
/// lossless native round trips.
///
/// `codex-env:` URIs contain an [`EnvironmentId`] and a normalized
/// [`EnvironmentPath`]. Like [VS Code resources], environment paths use `/`
/// separators on every host, so basename, parent, join, and comparison are
/// host-independent. Windows drive letters are lowercased, UNC paths retain a
/// leading `//`, and repeated separators and dot segments are normalized.
/// Original separator spelling, drive-letter case, filesystem aliases,
/// symlinks, case sensitivity, and Unicode normalization are not preserved or
/// resolved.
///
/// Serde represents a `PathUri` as its canonical URI string. Deserialization
/// also accepts an absolute native path for compatibility with fields that
/// previously used [`AbsolutePathBuf`]; relative paths are rejected. Valid
/// `codex-env:` strings round-trip through their normalized UTF-8 form. Valid
/// `file:` strings round-trip through their canonical URL form, including
/// encoded non-UTF-8 path bytes, but conversion to a native path remains
/// host-dependent as described by [RFC 8089].
///
/// [RFC 8089]: https://www.rfc-editor.org/rfc/rfc8089.html
/// [VS Code resources]: https://github.com/microsoft/vscode/blob/main/src/vs/base/common/resources.ts
#[derive(Clone, Debug, PartialEq, Eq, Hash, TS)]
#[ts(type = "string")]
pub struct PathUri {
    url: Url,
    parsed: ParsedPathUri,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ParsedPathUri {
    File {
        path: EnvironmentPath,
    },
    Environment {
        environment_id: EnvironmentId,
        path: EnvironmentPath,
    },
}

impl PathUri {
    /// Parses and validates a `file:` or `codex-env:` URI.
    pub fn parse(uri: &str) -> Result<Self, PathUriParseError> {
        if uri_scheme(uri)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case(CODEX_ENVIRONMENT_SCHEME))
        {
            return parse_environment_uri(uri);
        }
        Url::parse(uri)?.try_into()
    }

    /// Converts an absolute path on the current host to a `file:` URI.
    pub fn from_file_path(path: &AbsolutePathBuf) -> Result<Self, PathUriParseError> {
        let url = Url::from_file_path(path.as_path())
            .map_err(|()| PathUriParseError::PathCannotBeRepresentedAsFileUri)?;
        // `url` preserves the spelling of a Windows drive path. Rebuild local
        // drive URLs through `EnvironmentPath` so drive case and separators
        // match the cross-platform canonical form. UNC paths already use the
        // URL authority for their server name and must retain that structure.
        if cfg!(windows)
            && url.host().is_none()
            && let Some(path) = path.as_path().to_str()
        {
            return Self::try_from(file_url(&EnvironmentPath::windows(path)?)?);
        }
        Self::try_from(url)
    }

    /// Constructs a `codex-env:` URI from validated environment components.
    pub fn from_environment_path(
        environment_id: &EnvironmentId,
        path: &EnvironmentPath,
    ) -> Result<Self, PathUriParseError> {
        let url = environment_url(environment_id, path)?;
        Ok(Self {
            url,
            parsed: ParsedPathUri::Environment {
                environment_id: environment_id.clone(),
                path: path.clone(),
            },
        })
    }

    /// Returns `file` or `codex-env`.
    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    /// Returns the cached scheme-specific components.
    pub fn view(&self) -> PathUriView<'_> {
        match &self.parsed {
            ParsedPathUri::File { path } => PathUriView::File(FileUriView {
                path,
                url: &self.url,
            }),
            ParsedPathUri::Environment {
                environment_id,
                path,
            } => PathUriView::Environment(EnvironmentUriView {
                environment_id,
                path,
            }),
        }
    }

    /// Returns a clone of the canonical URL for either supported scheme.
    pub fn to_url(&self) -> Result<Url, PathUriParseError> {
        Ok(self.url.clone())
    }
}

/// A closed view over the path URI schemes understood by this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathUriView<'a> {
    File(FileUriView<'a>),
    Environment(EnvironmentUriView<'a>),
}

/// Borrowed components of a validated `file:` URI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileUriView<'a> {
    path: &'a EnvironmentPath,
    url: &'a Url,
}

impl<'a> FileUriView<'a> {
    pub fn path(self) -> &'a EnvironmentPath {
        self.path
    }

    /// Converts this file URI to a path using the current host's path rules.
    ///
    /// This fails when the URI describes a path form that the current host
    /// cannot represent, such as a Windows UNC authority on POSIX, or when the
    /// converted path is not absolute under the current host's rules. The URI
    /// and [`Self::path`] remain usable for lexical operations in those cases.
    ///
    /// This method only exists on the `File` variant. For a `codex-env:` URI,
    /// obtain [`EnvironmentUriView::path`] and call
    /// [`EnvironmentPath::to_native_path`] with the environment's path flavor.
    pub fn to_native_path(self) -> Result<AbsolutePathBuf, PathUriParseError> {
        let path = self
            .url
            .to_file_path()
            .map_err(|()| PathUriParseError::InvalidFileUriPath)?;
        AbsolutePathBuf::from_absolute_path_checked(path)
            .map_err(|_| PathUriParseError::InvalidFileUriPath)
    }
}

/// Borrowed components of a validated `codex-env:` URI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironmentUriView<'a> {
    environment_id: &'a EnvironmentId,
    path: &'a EnvironmentPath,
}

impl<'a> EnvironmentUriView<'a> {
    pub fn environment_id(self) -> &'a EnvironmentId {
        self.environment_id
    }

    pub fn path(self) -> &'a EnvironmentPath {
        self.path
    }
}

impl TryFrom<Url> for PathUri {
    type Error = PathUriParseError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        let (url, parsed) = match url.scheme() {
            FILE_SCHEME => (
                canonical_file_url(url.clone())?,
                ParsedPathUri::File {
                    path: parse_file_path(&url)?,
                },
            ),
            CODEX_ENVIRONMENT_SCHEME => {
                let (environment_id, path) = parse_environment_path(&url)?;
                (
                    environment_url(&environment_id, &path)?,
                    ParsedPathUri::Environment {
                        environment_id,
                        path,
                    },
                )
            }
            scheme => return Err(PathUriParseError::UnsupportedScheme(scheme.to_string())),
        };
        Ok(Self { url, parsed })
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
        self.url.fmt(f)
    }
}

impl Serialize for PathUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.url.as_str())
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
/// Serialization only accepts `file:` URIs and emits the current host's native
/// path spelling. New URI-native fields should use [`PathUri`]'s own serde
/// implementation instead.
pub mod legacy_file_path_serde {
    use super::*;

    pub fn serialize<S>(uri: &PathUri, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let PathUriView::File(view) = uri.view() else {
            return Err(serde::ser::Error::custom(
                "codex-env URI cannot use legacy file-path serialization",
            ));
        };
        view.to_native_path()
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

/// Validates a `file:` URL and extracts its host-independent URI path.
///
/// A non-local authority is represented as a canonical UNC path. Local paths
/// retain their URI spelling because interpreting `/C:/...` as Windows or POSIX
/// is deferred until native conversion.
fn parse_file_path(url: &Url) -> Result<EnvironmentPath, PathUriParseError> {
    validate_file_url(url)?;
    if url.host_str().is_some_and(|host| host != "localhost") {
        return EnvironmentPath::new(decode_file_uri_path(url))
            .map_err(|_| PathUriParseError::InvalidFileUriPath);
    }
    EnvironmentPath::posix(decode_uri_path(url.path()))
        .map_err(|_| PathUriParseError::InvalidFileUriPath)
}

/// Extracts and validates the environment id and canonical path from a parsed
/// `codex-env:` URL.
fn parse_environment_path(
    url: &Url,
) -> Result<(EnvironmentId, EnvironmentPath), PathUriParseError> {
    validate_common_known_uri(url)?;
    if url.host().is_some() {
        return Err(PathUriParseError::EnvironmentUriMustNotHaveAuthority);
    }
    if has_invalid_percent_encoding(url.path()) {
        return Err(PathUriParseError::InvalidEnvironmentUriPath);
    }

    let path = url
        .path()
        .strip_prefix('/')
        .ok_or(PathUriParseError::InvalidEnvironmentUriPath)?;
    let (environment_id, path) = path
        .split_once('/')
        .ok_or(PathUriParseError::InvalidEnvironmentUriPath)?;
    if contains_percent_encoded_slash(path) {
        return Err(PathUriParseError::InvalidEnvironmentUriPath);
    }
    let environment_id = urlencoding::decode(environment_id)
        .map_err(|_| PathUriParseError::InvalidEnvironmentUriPath)?;
    let environment_id = EnvironmentId::new(environment_id.into_owned())?;
    let path = format!("/{path}");
    let path =
        urlencoding::decode(&path).map_err(|_| PathUriParseError::InvalidEnvironmentUriPath)?;
    let path = EnvironmentPath::new(path.into_owned())?;
    Ok((environment_id, path))
}

/// Rebuilds a local Windows drive path as a canonical `file:` URL.
///
/// `Url::from_file_path` preserves drive-letter case. This helper is called for
/// local Windows drive paths so their URL spelling matches `EnvironmentPath`.
fn file_url(path: &EnvironmentPath) -> Result<Url, PathUriParseError> {
    let mut url = Url::parse("file:///")?;
    url.set_path(&path.as_str().replace('%', "%25"));
    Ok(url)
}

/// Removes the local `localhost` alias while retaining non-local UNC authority.
fn canonical_file_url(mut url: Url) -> Result<Url, PathUriParseError> {
    if url.host_str() == Some("localhost") {
        url.set_host(None)
            .map_err(|_| PathUriParseError::InvalidFileUriPath)?;
    }
    Ok(url)
}

/// Encodes validated environment components into a canonical `codex-env:` URL.
fn environment_url(
    environment_id: &EnvironmentId,
    path: &EnvironmentPath,
) -> Result<Url, PathUriParseError> {
    let mut url = Url::parse(&format!("{CODEX_ENVIRONMENT_SCHEME}:///"))?;
    let Ok(mut segments) = url.path_segments_mut() else {
        unreachable!("codex-env URLs support hierarchical path segments");
    };
    segments.clear();
    segments.push(environment_id.as_str());
    append_path_segments(&mut segments, path);
    drop(segments);
    Ok(url)
}

/// Appends each canonical path component with URL segment encoding.
fn append_path_segments(segments: &mut PathSegmentsMut<'_>, path: &EnvironmentPath) {
    let path = path.as_str().strip_prefix('/').unwrap_or_default();
    for component in path.split('/') {
        segments.push(component);
    }
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

/// Rejects URI metadata that has no defined meaning for either supported
/// scheme.
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

/// Converts a `file:` URL path and optional authority into canonical path text.
fn decode_file_uri_path(url: &Url) -> String {
    let path = decode_uri_path(url.path());
    if let Some(host) = url.host_str().filter(|host| *host != "localhost") {
        format!("//{host}{path}")
    } else {
        path
    }
}

/// Parses `codex-env:` before handing the value to `url::Url`.
///
/// General URL parsers normalize dot segments. Parsing the environment id and
/// path first prevents `..` in the environment path from being able to replace
/// the environment-id segment.
fn parse_environment_uri(uri: &str) -> Result<PathUri, PathUriParseError> {
    let (_, remainder) = uri
        .split_once(':')
        .ok_or(PathUriParseError::InvalidEnvironmentUriPath)?;
    if remainder.contains('?') {
        return Err(PathUriParseError::QueryNotAllowed);
    }
    if remainder.contains('#') {
        return Err(PathUriParseError::FragmentNotAllowed);
    }
    let Some(path) = remainder.strip_prefix("///") else {
        if remainder.starts_with("//") {
            return Err(PathUriParseError::EnvironmentUriMustNotHaveAuthority);
        }
        return Err(PathUriParseError::InvalidEnvironmentUriPath);
    };
    let (environment_id, path) = path
        .split_once('/')
        .ok_or(PathUriParseError::InvalidEnvironmentUriPath)?;
    if has_invalid_percent_encoding(environment_id) || has_invalid_percent_encoding(path) {
        return Err(PathUriParseError::InvalidEnvironmentUriPath);
    }
    if contains_percent_encoded_slash(path) {
        return Err(PathUriParseError::InvalidEnvironmentUriPath);
    }
    let environment_id = urlencoding::decode(environment_id)
        .map_err(|_| PathUriParseError::InvalidEnvironmentUriPath)?;
    let environment_id = EnvironmentId::new(environment_id.into_owned())?;
    let path = format!("/{path}");
    let path =
        urlencoding::decode(&path).map_err(|_| PathUriParseError::InvalidEnvironmentUriPath)?;
    let path = EnvironmentPath::new(path.into_owned())?;
    Ok(PathUri {
        url: environment_url(&environment_id, &path)?,
        parsed: ParsedPathUri::Environment {
            environment_id,
            path,
        },
    })
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

#[derive(Debug, Error)]
pub enum PathUriParseError {
    #[error("invalid URI: {0}")]
    InvalidUri(#[from] url::ParseError),
    #[error("unsupported path URI scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("path cannot be represented as a file URI")]
    PathCannotBeRepresentedAsFileUri,
    #[error("file URI contains an invalid absolute path")]
    InvalidFileUriPath,
    #[error("environment URI must not have an authority")]
    EnvironmentUriMustNotHaveAuthority,
    #[error("environment URI must contain an environment id and absolute path")]
    InvalidEnvironmentUriPath,
    #[error("credentials and ports are not allowed in path URIs")]
    CredentialsOrPortNotAllowed,
    #[error("query parameters are not allowed in path URIs")]
    QueryNotAllowed,
    #[error("fragments are not allowed in path URIs")]
    FragmentNotAllowed,
    #[error(transparent)]
    InvalidEnvironmentId(#[from] EnvironmentIdError),
    #[error(transparent)]
    InvalidEnvironmentPath(#[from] EnvironmentPathError),
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

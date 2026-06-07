//! Typed URIs for paths on the current host and in configured Codex environments.
//!
//! `file:` URI paths are stored in URI syntax, so they can be parsed
//! independently of the current host. Their path spelling stays lexical because
//! `/C:/src` can mean either a Windows drive path or a valid POSIX path; native
//! conversion is the point where host path rules are applied.
//! Remote paths use `codex-env:` URIs containing an environment identifier and
//! a normalized hierarchical path. Like [VS Code resources][vscode-resources],
//! environment paths always use `/` separators, so basename, parent, join, and
//! comparison operations do not depend on the operating system running Codex.
//!
//! Native Windows paths are normalized at construction: backslashes become
//! slashes, drive letters become lowercase, and `C:\src` becomes `/c:/src`.
//! UNC paths retain a leading `//`. POSIX paths already use the canonical form.
//!
//! This follows VS Code's split between URI-level path operations and
//! environment-aware native conversion. The tradeoff is that normalization
//! intentionally loses original separator spelling, repeated separators, dot
//! segments, and Windows drive-letter casing. Converting a canonical path back
//! to native syntax, validating platform-specific names, and deciding path case
//! sensitivity require metadata from the environment. Filesystem-dependent
//! operations such as canonicalizing symlinks still require the environment.
//! Environment URI identity is lexical and UTF-8-only: case, Unicode
//! normalization, hard links, and filesystem aliases are not folded together.
//! `file:` URIs may additionally retain percent-encoded non-UTF-8 bytes so
//! local POSIX paths can round-trip without loss.
//!
//! [`file:` URIs][rfc-8089] remain host-specific because they can only be
//! dependably converted to paths when local to the interpreting host.
//!
//! [rfc-8089]: https://www.rfc-editor.org/rfc/rfc8089.html
//! [vscode-resources]: https://github.com/microsoft/vscode/blob/main/src/vs/base/common/resources.ts

mod environment_path;

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
use url::PathSegmentsMut;
use url::Url;

pub use environment_path::EnvironmentPath;
pub use environment_path::EnvironmentPathError;
pub use environment_path::PathFlavor;

pub const FILE_SCHEME: &str = "file";
pub const CODEX_ENVIRONMENT_SCHEME: &str = "codex-env";

/// Maximum encoded environment identifier length accepted at Codex boundaries.
pub const MAX_ENVIRONMENT_ID_LEN: usize = 64;

/// A URI that can identify a path in the current host or another environment.
///
/// Construction is fallible and only succeeds for schemes understood by this
/// version of the crate. This keeps [`PathUri::view`] infallible and exhaustive.
#[derive(Clone, Debug, PartialEq, Eq, Hash, TS)]
#[ts(type = "string")]
pub struct PathUri(Url);

impl PathUri {
    pub fn parse(uri: &str) -> Result<Self, PathUriParseError> {
        if uri_scheme(uri)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case(CODEX_ENVIRONMENT_SCHEME))
        {
            return parse_environment_uri(uri);
        }
        Url::parse(uri)?.try_into()
    }

    pub fn from_file_path(path: &AbsolutePathBuf) -> Result<Self, PathUriParseError> {
        let url = Url::from_file_path(path.as_path())
            .map_err(|()| PathUriParseError::PathCannotBeRepresentedAsFileUri)?;
        #[cfg(windows)]
        if url.host().is_none()
            && let Some(path) = path.as_path().to_str()
        {
            return Ok(Self(file_url(&EnvironmentPath::windows(path)?)?));
        }
        Self::try_from(url)
    }

    pub fn from_environment_path(
        environment_id: &EnvironmentId,
        path: &EnvironmentPath,
    ) -> Result<Self, PathUriParseError> {
        let url = environment_url(environment_id, path)?;
        Self::try_from(url)
    }

    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    pub fn view(&self) -> PathUriView {
        if self.scheme() == FILE_SCHEME {
            return PathUriView::File(FileUriView {
                path: EnvironmentPath::from_normalized(decode_file_uri_path(&self.0)),
                url: self.0.clone(),
            });
        }

        let path = self.0.path().strip_prefix('/').unwrap_or_default();
        let (environment_id, path) = path.split_once('/').unwrap_or_default();
        PathUriView::Environment(EnvironmentUriView {
            environment_id: EnvironmentId(decode_uri_path(environment_id)),
            path: EnvironmentPath::from_normalized(decode_uri_path(&format!("/{path}"))),
        })
    }

    pub fn to_url(&self) -> Result<Url, PathUriParseError> {
        Ok(self.0.clone())
    }
}

/// A closed view over the path URI schemes understood by this crate.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathUriView {
    File(FileUriView),
    Environment(EnvironmentUriView),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileUriView {
    path: EnvironmentPath,
    url: Url,
}

impl FileUriView {
    pub fn path(&self) -> &EnvironmentPath {
        &self.path
    }

    /// Converts this file URI to a path using the current host's path rules.
    pub fn to_native_path(&self) -> Result<AbsolutePathBuf, PathUriParseError> {
        let path = self
            .url
            .to_file_path()
            .map_err(|()| PathUriParseError::InvalidFileUriPath)?;
        AbsolutePathBuf::from_absolute_path_checked(path)
            .map_err(|_| PathUriParseError::InvalidFileUriPath)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentUriView {
    environment_id: EnvironmentId,
    path: EnvironmentPath,
}

impl EnvironmentUriView {
    pub fn environment_id(&self) -> &EnvironmentId {
        &self.environment_id
    }

    pub fn path(&self) -> &EnvironmentPath {
        &self.path
    }
}

/// An opaque identifier for a configured remote environment.
///
/// The URI path dot segments `.` and `..` are excluded because URL parsers
/// normalize them before this crate can recover the identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema, TS)]
#[serde(transparent)]
#[schemars(with = "String")]
#[ts(type = "string")]
pub struct EnvironmentId(String);

impl EnvironmentId {
    pub fn new(id: impl Into<String>) -> Result<Self, EnvironmentIdError> {
        let id = id.into();
        validate_environment_id(&id)?;
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EnvironmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for EnvironmentId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for EnvironmentId {
    type Err = EnvironmentIdError;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::new(id)
    }
}

impl<'de> Deserialize<'de> for EnvironmentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl TryFrom<Url> for PathUri {
    type Error = PathUriParseError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        let url = match url.scheme() {
            FILE_SCHEME => {
                parse_file_path(&url)?;
                canonical_file_url(url)?
            }
            CODEX_ENVIRONMENT_SCHEME => {
                let (environment_id, path) = parse_environment_path(&url)?;
                environment_url(&environment_id, &path)?
            }
            scheme => return Err(PathUriParseError::UnsupportedScheme(scheme.to_string())),
        };
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

fn parse_file_path(url: &Url) -> Result<EnvironmentPath, PathUriParseError> {
    validate_file_url(url)?;
    if url.host_str().is_some_and(|host| host != "localhost") {
        return EnvironmentPath::new(decode_file_uri_path(url))
            .map_err(|_| PathUriParseError::InvalidFileUriPath);
    }
    EnvironmentPath::posix(decode_uri_path(url.path()))
        .map_err(|_| PathUriParseError::InvalidFileUriPath)
}

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

#[cfg(windows)]
fn file_url(path: &EnvironmentPath) -> Result<Url, PathUriParseError> {
    let mut url = Url::parse("file:///")?;
    url.set_path(&path.as_str().replace('%', "%25"));
    Ok(url)
}

fn canonical_file_url(mut url: Url) -> Result<Url, PathUriParseError> {
    if url.host_str() == Some("localhost") {
        url.set_host(None)
            .map_err(|_| PathUriParseError::InvalidFileUriPath)?;
    }
    Ok(url)
}

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

fn append_path_segments(segments: &mut PathSegmentsMut<'_>, path: &EnvironmentPath) {
    let path = path.as_str().strip_prefix('/').unwrap_or_default();
    for component in path.split('/') {
        segments.push(component);
    }
}

fn decode_uri_path(path: &str) -> String {
    urlencoding::decode(path)
        .map(std::borrow::Cow::into_owned)
        .unwrap_or_else(|_| path.to_string())
}

fn contains_percent_encoded_slash(path: &str) -> bool {
    path.as_bytes()
        .windows(3)
        .any(|bytes| bytes[0] == b'%' && bytes[1] == b'2' && matches!(bytes[2], b'f' | b'F'))
}

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

fn decode_file_uri_path(url: &Url) -> String {
    let path = decode_uri_path(url.path());
    if let Some(host) = url.host_str().filter(|host| *host != "localhost") {
        format!("//{host}{path}")
    } else {
        path
    }
}

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
    Ok(PathUri(environment_url(&environment_id, &path)?))
}

fn uri_scheme(uri: &str) -> Option<&str> {
    let (scheme, _) = uri.split_once(':')?;
    (!scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic()
                || (index > 0 && (byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')))
        }))
    .then_some(scheme)
}

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

fn validate_environment_id(id: &str) -> Result<(), EnvironmentIdError> {
    if id.is_empty() {
        return Err(EnvironmentIdError::Empty);
    }
    if matches!(id, "." | "..") {
        return Err(EnvironmentIdError::DotSegment(id.to_string()));
    }
    if id.len() > MAX_ENVIRONMENT_ID_LEN {
        return Err(EnvironmentIdError::TooLong {
            length: id.len(),
            max_length: MAX_ENVIRONMENT_ID_LEN,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EnvironmentIdError {
    #[error("environment id cannot be empty")]
    Empty,
    #[error("environment id `{0}` cannot be a URI path dot segment")]
    DotSegment(String),
    #[error("environment id is {length} bytes; maximum length is {max_length}")]
    TooLong { length: usize, max_length: usize },
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

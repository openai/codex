use crate::PathUri;
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

#[cfg(not(any(windows, unix)))]
compile_error!("PathConvention::native() requires a Windows or Unix target");

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
/// accidentally applying the current host's path rules.
#[derive(Clone, Debug, PartialEq, Eq, Hash, TS)]
#[ts(type = "string")]
pub struct NativePathString(String);

impl NativePathString {
    /// Renders a path URI using the requested native path convention.
    ///
    /// TODO(anp): Once `PathUri` carries an environment identifier, resolve the path
    /// convention from that identifier instead of requiring it explicitly.
    pub fn from_path_uri(
        path: &PathUri,
        convention: PathConvention,
    ) -> Result<Self, NativePathStringError> {
        if path.is_opaque_fallback() {
            return Err(NativePathStringError::OpaqueFallback {
                path: path.to_string(),
            });
        }
        let value = match convention {
            PathConvention::Posix => render_posix_path(path)?,
            PathConvention::Windows => render_windows_path(path)?,
        };
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NativePathString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for NativePathString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl JsonSchema for NativePathString {
    fn schema_name() -> String {
        "NativePathString".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(generator)
    }
}

fn render_posix_path(path: &PathUri) -> Result<String, NativePathStringError> {
    let url = path.to_url();
    if url.host_str().is_some() {
        return Err(incompatible_convention(path, PathConvention::Posix));
    }

    let mut rendered = String::new();
    for segment in path_segments(&url) {
        rendered.push('/');
        rendered.push_str(&decode_native_segment(
            path,
            segment,
            PathConvention::Posix,
        )?);
    }
    Ok(rendered)
}

fn render_windows_path(path: &PathUri) -> Result<String, NativePathStringError> {
    let url = path.to_url();
    let mut segments = path_segments(&url);
    let mut rendered = String::new();
    if let Some(host) = url.host_str() {
        let Some(share) = segments.next() else {
            return Err(incompatible_convention(path, PathConvention::Windows));
        };
        let share = decode_native_segment(path, share, PathConvention::Windows)?;
        if share.is_empty() {
            return Err(incompatible_convention(path, PathConvention::Windows));
        }
        validate_windows_component(path, &share)?;
        rendered.push_str(r"\\");
        rendered.push_str(host);
        rendered.push('\\');
        rendered.push_str(&share);
    } else {
        let Some(drive) = segments.next() else {
            return Err(incompatible_convention(path, PathConvention::Windows));
        };
        let drive = decode_native_segment(path, drive, PathConvention::Windows)?;
        let bytes = drive.as_bytes();
        if bytes.len() != 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
            return Err(incompatible_convention(path, PathConvention::Windows));
        }
        rendered.push_str(&drive);
    }

    for segment in segments {
        let segment = decode_native_segment(path, segment, PathConvention::Windows)?;
        if !segment.is_empty() {
            validate_windows_component(path, &segment)?;
        }
        rendered.push('\\');
        rendered.push_str(&segment);
    }
    if rendered.len() == 2 && rendered.as_bytes()[1] == b':' {
        rendered.push('\\');
    }
    Ok(rendered)
}

fn path_segments(url: &url::Url) -> std::str::Split<'_, char> {
    url.path_segments()
        .unwrap_or_else(|| unreachable!("validated file URLs have path segments"))
}

fn decode_native_segment(
    path: &PathUri,
    segment: &str,
    convention: PathConvention,
) -> Result<String, NativePathStringError> {
    let bytes = urlencoding::decode_binary(segment.as_bytes());
    let contains_separator =
        bytes.contains(&b'/') || (convention == PathConvention::Windows && bytes.contains(&b'\\'));
    if contains_separator {
        return Err(NativePathStringError::EncodedSeparator {
            path: path.to_string(),
            convention,
        });
    }
    std::str::from_utf8(&bytes)
        .map(str::to_string)
        .map_err(|_| NativePathStringError::NonUtf8 {
            path: path.to_string(),
        })
}

fn validate_windows_component(
    path: &PathUri,
    component: &str,
) -> Result<(), NativePathStringError> {
    let contains_invalid_character = component
        .chars()
        .any(|character| character <= '\u{1f}' || r#"<>:"/\|?*"#.contains(character));
    if contains_invalid_character || component.ends_with([' ', '.']) {
        return Err(NativePathStringError::InvalidWindowsComponent {
            path: path.to_string(),
            component: component.to_string(),
        });
    }
    Ok(())
}

fn incompatible_convention(path: &PathUri, convention: PathConvention) -> NativePathStringError {
    NativePathStringError::IncompatibleConvention {
        path: path.to_string(),
        convention,
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativePathStringError {
    #[error("opaque fallback path URI `{path}` cannot be rendered as a UTF-8 native path")]
    OpaqueFallback { path: String },
    #[error("path URI `{path}` cannot be rendered using {convention} path syntax")]
    IncompatibleConvention {
        path: String,
        convention: PathConvention,
    },
    #[error("path URI `{path}` contains path bytes that are not valid UTF-8")]
    NonUtf8 { path: String },
    #[error("path URI `{path}` contains a percent-encoded separator for {convention} path syntax")]
    EncodedSeparator {
        path: String,
        convention: PathConvention,
    },
    #[error("path URI `{path}` contains invalid Windows path component `{component}`")]
    InvalidWindowsComponent { path: String, component: String },
}

#[cfg(test)]
#[path = "native_path_string_tests.rs"]
mod tests;

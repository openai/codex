use super::PathConvention;
use super::PathUri;
use super::PathUriParseError;
use super::decode_bad_path_uri;
use super::decode_uri_path;
use super::is_windows_drive_uri_segment;
use super::is_windows_separator_byte;

impl PathUri {
    /// Parses absolute native path text without consulting the current host.
    ///
    /// POSIX paths, rooted Windows drive paths, and UNC paths are recognized
    /// from their spelling. Relative paths, drive-relative paths, Windows
    /// root-relative paths, and device namespace paths are rejected. Call
    /// [`Self::parse`] instead when the input is already a `file:` URI.
    pub fn from_inferred_absolute_path(path: &str) -> Result<Self, PathUriParseError> {
        let convention = inferred_absolute_path_convention(path)
            .ok_or_else(|| PathUriParseError::ExpectedAbsoluteNativePath(path.to_string()))?;
        Self::from_absolute_native_path(path, convention)
            .ok_or_else(|| PathUriParseError::ExpectedAbsoluteNativePath(path.to_string()))
    }

    /// Lexically resolves target-relative path text against this URI.
    ///
    /// Unlike [`Self::join`], this rejects absolute, root-relative, UNC, and
    /// drive-relative input under either POSIX or Windows path grammar. Parent
    /// segments may normalize components within the relative input, but may
    /// not traverse above the receiver URI.
    pub fn join_relative(&self, path: &str) -> Result<Self, PathUriParseError> {
        if has_rooted_or_drive_prefix(path) {
            return Err(PathUriParseError::JoinPathMustBeRelative(path.to_string()));
        }
        if path.is_empty() {
            return Ok(self.clone());
        }
        if decode_bad_path_uri(&self.0).is_some() {
            return self.join(path);
        }

        let convention =
            self.infer_path_convention()
                .ok_or_else(|| PathUriParseError::InvalidFileUriPath {
                    path: self.to_string(),
                })?;
        let mut depth = 0usize;
        for component in convention.path_segments(path) {
            match component {
                "" | "." => {}
                ".." if depth == 0 => {
                    return Err(PathUriParseError::JoinPathEscapesBase(path.to_string()));
                }
                ".." => depth -= 1,
                _ => depth += 1,
            }
        }

        self.join(path)
    }

    /// Returns this URI's normalized lexical path relative to `root`.
    ///
    /// The result is available only when this URI is equal to or a descendant
    /// of `root` under the same inferred path convention and URI authority.
    /// The returned text uses `/` separators on every platform. Opaque fallback
    /// URIs do not support lexical containment.
    pub fn relative_path_from(&self, root: &Self) -> Option<String> {
        if decode_bad_path_uri(&self.0).is_some() || decode_bad_path_uri(&root.0).is_some() {
            return None;
        }
        let convention = self.infer_path_convention()?;
        if root.infer_path_convention() != Some(convention)
            || self.0.host_str() != root.0.host_str()
        {
            return None;
        }

        let candidate = lexical_segments(self);
        let root = lexical_segments(root);
        if convention == PathConvention::Windows
            && (candidate.first().is_none_or(|segment| {
                self.0.host_str().is_none() && !is_windows_drive_uri_segment(segment)
            }) || root.first().is_none_or(|segment| {
                self.0.host_str().is_none() && !is_windows_drive_uri_segment(segment)
            }))
        {
            return None;
        }
        let relative = candidate.strip_prefix(root.as_slice())?;
        Some(
            relative
                .iter()
                .map(|segment| decode_uri_path(segment))
                .collect::<Vec<_>>()
                .join("/"),
        )
    }
}

fn inferred_absolute_path_convention(path: &str) -> Option<PathConvention> {
    let bytes = path.as_bytes();
    let rooted_drive = matches!(
        bytes,
        [drive, b':', separator, ..]
            if drive.is_ascii_alphabetic() && is_windows_separator_byte(*separator)
    );
    let unc = matches!(bytes, [b'\\', b'\\', third, ..] if !matches!(*third, b'.' | b'?'));
    if path.contains('\0') {
        None
    } else if rooted_drive || unc {
        Some(PathConvention::Windows)
    } else if path.starts_with('/') {
        Some(PathConvention::Posix)
    } else {
        None
    }
}

fn has_rooted_or_drive_prefix(path: &str) -> bool {
    matches!(path.as_bytes(), [b'/' | b'\\', ..])
        || matches!(path.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic())
}

fn lexical_segments(path: &PathUri) -> Vec<&str> {
    path.0
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|segment| !segment.is_empty())
        .collect()
}

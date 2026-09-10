//! Resolve filesystem denial paths with the existing URI parser and lexical join.
//! Reject ambiguous or lossy representations before rendering security constraints.

use crate::LegacyAppPathString;
use crate::LegacyAppPathStringError;
use crate::PathConvention;
use crate::PathUri;
use crate::PathUriParseError;

impl PathUri {
    /// Resolves configuration text using only the supplied convention, base, and home.
    /// Home-relative inputs require a home; other relative inputs require a base.
    /// Used facts and results must have an unambiguous, lossless native spelling.
    pub fn resolve_config_path(
        input: &str,
        convention: PathConvention,
        base: Option<&Self>,
        user_home_dir: Option<&Self>,
    ) -> Result<String, LegacyAppPathStringError> {
        Self::validate_config_path_text(input, convention)?;
        let path = LegacyAppPathString::from_string(input);
        let resolved = if convention.home_relative_suffix(input).is_some() {
            let home =
                user_home_dir.ok_or_else(|| LegacyAppPathStringError::MissingHomeDirectory {
                    path: input.to_string(),
                })?;
            home.validate_config_path(convention)?;
            path.resolve_against(home, Some(home))?
        } else {
            match path.to_path_uri(convention) {
                Ok(absolute) => absolute,
                Err(error) => {
                    let base = base.ok_or(error)?;
                    base.validate_config_path(convention)?;
                    path.resolve_against(base, /*user_home_dir*/ None)?
                }
            }
        };
        resolved.validate_config_path(convention)?;
        // Config paths historically omit trailing separators except at a root.
        let resolved = resolved.join(".")?;
        let mut rendered = LegacyAppPathString::from_path_uri(&resolved, convention)?.into_string();
        if resolved.0.host_str().is_some() && resolved.parent().is_none() {
            rendered.push('\\');
        }
        Ok(rendered)
    }

    /// Checks that a resolved configuration URI has a lossless native spelling
    /// in the owning executor's convention. Opaque and ambiguous paths fail.
    pub fn validate_config_path(
        &self,
        convention: PathConvention,
    ) -> Result<(), LegacyAppPathStringError> {
        if self.infer_path_convention() != Some(convention) {
            return Err(LegacyAppPathStringError::IncompatibleConvention {
                path: self.to_string(),
                convention,
            });
        }
        let bytes = self.decoded_path_bytes();
        if self.lexical_depth().is_none()
            || bytes.contains(&0)
            || std::str::from_utf8(&bytes).is_err()
        {
            return Err(PathUriParseError::InvalidFileUriPath {
                path: self.to_string(),
            }
            .into());
        }
        Self::validate_config_path_text(
            LegacyAppPathString::from_path_uri(self, convention)?.as_str(),
            convention,
        )
    }

    /// Validates native configuration text without resolving paths or glob syntax.
    /// Rejects spellings that could change targets during later native conversion.
    pub fn validate_config_path_text(
        input: &str,
        convention: PathConvention,
    ) -> Result<(), LegacyAppPathStringError> {
        let namespace_alias = codex_utils_absolute_path::normalize_windows_device_path(input);
        let native_input = namespace_alias.as_deref().unwrap_or(input);
        let has_windows_component_colon = convention == PathConvention::Windows
            && convention
                .path_segments(native_input)
                .enumerate()
                .any(|(index, segment)| {
                    let segment = if index == 0
                        && segment
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphabetic)
                        && segment.as_bytes().get(/*index*/ 1) == Some(&b':')
                    {
                        &segment[2..]
                    } else {
                        segment
                    };
                    segment.contains(':')
                });
        let mixed_home_separators = convention == PathConvention::Windows
            && convention
                .home_relative_suffix(input)
                .is_some_and(|suffix| {
                    suffix.starts_with('/') && suffix.trim_start_matches('/').starts_with('\\')
                        || suffix.starts_with('\\')
                            && suffix.trim_start_matches('\\').starts_with('/')
                });
        let bare_windows_drive = convention == PathConvention::Windows
            && matches!(native_input.as_bytes(), [drive, b':'] if drive.is_ascii_alphabetic());
        if input.contains('\0')
            || has_windows_component_colon
            || mixed_home_separators
            || bare_windows_drive
        {
            return Err(LegacyAppPathStringError::UnsupportedConfigPath {
                path: input.to_string(),
                convention,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "config_path_tests.rs"]
mod tests;

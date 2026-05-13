use std::fmt;

/// Fully qualified reference to a file-like asset known to Code Mode.
///
/// The scheme tells the file broker which provider owns the asset. The broker
/// should keep provider-specific credentials and bytes out of model-visible
/// arguments while still letting Code Mode pass stable refs between tools.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileRef {
    raw: String,
    scheme: FileScheme,
    body: String,
}

impl FileRef {
    pub fn parse(raw: impl Into<String>) -> Result<Self, FileRefParseError> {
        let raw = raw.into();
        let Some((scheme, body)) = raw.split_once("://") else {
            return Err(FileRefParseError::MissingScheme);
        };
        if body.is_empty() {
            return Err(FileRefParseError::MissingBody);
        }
        let scheme = FileScheme::parse(scheme)?;
        let body = body.to_string();
        Ok(Self { raw, scheme, body })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn scheme(&self) -> FileScheme {
        self.scheme
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

/// Provider family that owns a file ref.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileScheme {
    Env,
    Library,
    Connector,
    Other,
}

impl FileScheme {
    fn parse(scheme: &str) -> Result<Self, FileRefParseError> {
        if scheme.is_empty() {
            return Err(FileRefParseError::MissingScheme);
        }
        if !scheme
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
        {
            return Err(FileRefParseError::InvalidScheme(scheme.to_string()));
        }
        Ok(match scheme {
            "env" => Self::Env,
            "oai_library" => Self::Library,
            "connector" => Self::Connector,
            _ => Self::Other,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileRefParseError {
    MissingScheme,
    InvalidScheme(String),
    MissingBody,
}

impl fmt::Display for FileRefParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScheme => write!(f, "file ref must start with a provider scheme"),
            Self::InvalidScheme(scheme) => write!(f, "invalid file ref scheme `{scheme}`"),
            Self::MissingBody => write!(f, "file ref must include a provider-owned path or id"),
        }
    }
}

impl std::error::Error for FileRefParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_env_file_ref() {
        assert_eq!(
            FileRef::parse("env://current/work/report.pdf"),
            Ok(FileRef {
                raw: "env://current/work/report.pdf".to_string(),
                scheme: FileScheme::Env,
                body: "current/work/report.pdf".to_string(),
            })
        );
    }

    #[test]
    fn classifies_known_provider_schemes() {
        assert_eq!(
            FileRef::parse("oai_library://file_123")
                .expect("library ref should parse")
                .scheme(),
            FileScheme::Library
        );
        assert_eq!(
            FileRef::parse("connector://google_drive/file_123")
                .expect("connector ref should parse")
                .scheme(),
            FileScheme::Connector
        );
    }

    #[test]
    fn rejects_ambiguous_refs() {
        assert_eq!(
            FileRef::parse("report.pdf"),
            Err(FileRefParseError::MissingScheme)
        );
        assert_eq!(
            FileRef::parse("env://"),
            Err(FileRefParseError::MissingBody)
        );
    }
}

use async_trait::async_trait;

use crate::ConfigDocumentErrorSpan;
use crate::ConfigDocumentRead;
use crate::ConfigDocumentStore;
use crate::ConfigStoreResult;
use crate::ReadConfigDocumentParams;

/// Filesystem-backed implementation of [ConfigDocumentStore].
///
/// This implementation reads the requested path from the local filesystem and parses it as TOML.
/// It does not apply config-layer ordering, project trust, relative-path resolution, or fallback
/// behavior for missing documents.
#[derive(Debug, Default, Clone)]
pub struct LocalConfigStore;

#[async_trait]
impl ConfigDocumentStore for LocalConfigStore {
    async fn read_config_document(
        &self,
        params: ReadConfigDocumentParams,
    ) -> ConfigStoreResult<ConfigDocumentRead> {
        match tokio::fs::read_to_string(params.path.as_path()).await {
            Ok(raw_toml) => match toml::from_str(&raw_toml) {
                Ok(value) => Ok(ConfigDocumentRead::Present { value }),
                Err(error) => Ok(ConfigDocumentRead::ParseError {
                    raw_toml,
                    message: error.message().to_string(),
                    span: error.span().map(ConfigDocumentErrorSpan::from),
                }),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(ConfigDocumentRead::Missing)
            }
            Err(err) => Ok(ConfigDocumentRead::ReadError {
                kind: read_error_kind_name(err.kind()).to_string(),
                message: err.to_string(),
            }),
        }
    }
}

fn read_error_kind_name(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::InvalidData => "invalid_data",
        std::io::ErrorKind::InvalidInput => "invalid_input",
        std::io::ErrorKind::TimedOut => "timed_out",
        std::io::ErrorKind::Interrupted => "interrupted",
        std::io::ErrorKind::Unsupported => "unsupported",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use toml::Value as TomlValue;

    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;

    fn read_params(path: AbsolutePathBuf) -> ReadConfigDocumentParams {
        ReadConfigDocumentParams { path }
    }

    #[tokio::test]
    async fn reads_present_config_document() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let path = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("config.toml"))?;
        tokio::fs::write(path.as_path(), "model = \"gpt-5\"\n").await?;

        let got = LocalConfigStore
            .read_config_document(read_params(path))
            .await?;

        let expected = ConfigDocumentRead::Present {
            value: TomlValue::Table(toml::map::Map::from_iter([(
                "model".to_string(),
                TomlValue::String("gpt-5".to_string()),
            )])),
        };
        assert_eq!(got, expected);
        Ok(())
    }

    #[tokio::test]
    async fn reports_missing_config_document() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let path = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("config.toml"))?;

        let got = LocalConfigStore
            .read_config_document(read_params(path))
            .await?;

        assert_eq!(got, ConfigDocumentRead::Missing);
        Ok(())
    }

    #[tokio::test]
    async fn reports_parse_error_with_raw_toml() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let path = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("config.toml"))?;
        let raw_toml = "model = [";
        tokio::fs::write(path.as_path(), raw_toml).await?;

        let got = LocalConfigStore
            .read_config_document(read_params(path))
            .await?;

        match got {
            ConfigDocumentRead::ParseError {
                raw_toml: got_raw,
                message,
                span,
            } => {
                assert_eq!(got_raw, raw_toml);
                assert!(!message.is_empty());
                assert!(span.is_some());
            }
            other => panic!("expected parse error, got {other:?}"),
        }
        Ok(())
    }
}

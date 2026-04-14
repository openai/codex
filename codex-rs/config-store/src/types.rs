use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::ops::Range;
use toml::Value as TomlValue;

/// Request to read one path-addressed config document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadConfigDocumentParams {
    /// Absolute path to the config document to read.
    pub path: AbsolutePathBuf,
}

/// Byte span for a config document parse error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDocumentErrorSpan {
    /// Inclusive byte offset where the error starts.
    pub start: usize,

    /// Exclusive byte offset where the error ends.
    pub end: usize,
}

impl From<Range<usize>> for ConfigDocumentErrorSpan {
    fn from(span: Range<usize>) -> Self {
        Self {
            start: span.start,
            end: span.end,
        }
    }
}

impl From<ConfigDocumentErrorSpan> for Range<usize> {
    fn from(span: ConfigDocumentErrorSpan) -> Self {
        span.start..span.end
    }
}

/// Read and parse state for one config document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConfigDocumentRead {
    /// The backing source was absent.
    Missing,

    /// The backing source was present and parsed successfully.
    Present {
        /// Parsed TOML document.
        value: TomlValue,
    },

    /// The backing source was present but could not be parsed as TOML.
    ///
    /// This is distinct from ConfigDocumentRead::ReadError because project config parse errors are
    /// fatal only after Codex applies project trust policy.
    ParseError {
        /// Original TOML text that failed to parse.
        raw_toml: String,

        /// User-facing parse failure message.
        message: String,

        /// Optional byte span for the parse failure.
        span: Option<ConfigDocumentErrorSpan>,
    },

    /// The provider could not read the backing source.
    ReadError {
        /// Primitive read failure kind, such as "permission_denied" or "other".
        kind: String,

        /// User-facing read failure message.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;
    use toml::Value as TomlValue;

    use super::*;
    use crate::ConfigDocumentStore;
    use crate::ConfigStoreResult;

    struct StaticDocumentStore {
        document: ConfigDocumentRead,
    }

    #[async_trait]
    impl ConfigDocumentStore for StaticDocumentStore {
        async fn read_config_document(
            &self,
            _params: ReadConfigDocumentParams,
        ) -> ConfigStoreResult<ConfigDocumentRead> {
            Ok(self.document.clone())
        }
    }

    #[tokio::test]
    async fn store_trait_can_return_config_documents() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path =
            AbsolutePathBuf::from_absolute_path(temp_dir.path().join("config.toml")).expect("abs");
        let value = TomlValue::Table(toml::map::Map::new());
        let document = ConfigDocumentRead::Present { value };
        let store = StaticDocumentStore {
            document: document.clone(),
        };

        let got = store
            .read_config_document(ReadConfigDocumentParams { path })
            .await
            .expect("read document");

        assert_eq!(got, document);
    }
}

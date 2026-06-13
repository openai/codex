use std::path::PathBuf;

use codex_workload_identity::FileSubjectTokenSource;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;

const AZURE_FEDERATED_TOKEN_FILE_ENV: &str = "AZURE_FEDERATED_TOKEN_FILE";

#[derive(Clone, Debug)]
pub struct AzureSubjectTokenProvider {
    source: Option<FileSubjectTokenSource>,
}

impl AzureSubjectTokenProvider {
    pub fn new(token_file: Option<PathBuf>) -> Self {
        let token_file = token_file
            .or_else(|| std::env::var_os(AZURE_FEDERATED_TOKEN_FILE_ENV).map(PathBuf::from));
        Self {
            source: token_file.map(|path| FileSubjectTokenSource::for_source("azure", path)),
        }
    }
}

impl SubjectTokenProvider for AzureSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        match &self.source {
            Some(source) => source.subject_token().await,
            None => Err(SubjectTokenError::MissingPrerequisite {
                provider: "azure",
                prerequisite: AZURE_FEDERATED_TOKEN_FILE_ENV.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn reads_and_rereads_projected_token() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join("azure-token");
        tokio::fs::write(&path, "first.azure.token\n").await?;
        let source = AzureSubjectTokenProvider::new(Some(path.clone()));

        assert_eq!(
            source.subject_token().await?,
            SubjectToken::jwt("first.azure.token", "azure")?
        );
        tokio::fs::write(path, "second.azure.token\n").await?;
        assert_eq!(
            source.subject_token().await?,
            SubjectToken::jwt("second.azure.token", "azure")?
        );
        Ok(())
    }
}

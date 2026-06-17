use std::path::PathBuf;

use codex_workload_identity::FileSubjectTokenSource;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;

const AZURE_FEDERATED_TOKEN_FILE_ENV: &str = "AZURE_FEDERATED_TOKEN_FILE";

#[derive(Clone, Debug)]
pub struct AzureSubjectTokenProvider {
    token_file: Option<PathBuf>,
}

impl AzureSubjectTokenProvider {
    pub fn new(token_file: Option<PathBuf>) -> Self {
        let token_file = token_file
            .or_else(|| std::env::var_os(AZURE_FEDERATED_TOKEN_FILE_ENV).map(PathBuf::from));
        Self { token_file }
    }
}

impl SubjectTokenProvider for AzureSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        let token_file =
            self.token_file
                .as_ref()
                .ok_or_else(|| SubjectTokenError::MissingPrerequisite {
                    provider: "azure",
                    prerequisite: AZURE_FEDERATED_TOKEN_FILE_ENV.to_string(),
                })?;
        if !token_file.is_absolute() {
            return Err(SubjectTokenError::InvalidConfiguration { provider: "azure" });
        }
        FileSubjectTokenSource::for_source("azure", token_file.clone())
            .subject_token()
            .await
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

    #[tokio::test]
    async fn rejects_relative_token_path() {
        let source = AzureSubjectTokenProvider::new(Some(PathBuf::from("azure-token")));

        assert!(matches!(
            source.subject_token().await,
            Err(SubjectTokenError::InvalidConfiguration { provider: "azure" })
        ));
    }
}

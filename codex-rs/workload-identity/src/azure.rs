use std::path::PathBuf;

use crate::WorkloadIdentityError;

const AZURE_FEDERATED_TOKEN_FILE_ENV: &str = "AZURE_FEDERATED_TOKEN_FILE";

#[derive(Clone, Debug)]
pub(crate) struct AzureSubjectTokenSource {
    token_file: Option<PathBuf>,
}

impl AzureSubjectTokenSource {
    pub(crate) fn new(token_file: Option<PathBuf>) -> Self {
        Self { token_file }
    }

    pub(crate) async fn subject_token(&self) -> Result<String, WorkloadIdentityError> {
        let token_file = match self.token_file.clone() {
            Some(token_file) => token_file,
            None => std::env::var_os(AZURE_FEDERATED_TOKEN_FILE_ENV)
                .map(PathBuf::from)
                .ok_or(WorkloadIdentityError::MissingAzureFederatedTokenFile)?,
        };
        let token = tokio::fs::read_to_string(&token_file)
            .await
            .map_err(|source| WorkloadIdentityError::ReadSubjectToken {
                path: token_file,
                source,
            })?;
        let token = token.trim();
        if token.is_empty() {
            return Err(WorkloadIdentityError::EmptySubjectToken);
        }
        Ok(token.to_string())
    }
}

#[cfg(test)]
#[path = "azure_tests.rs"]
mod tests;

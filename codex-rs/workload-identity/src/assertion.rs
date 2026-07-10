use std::path::PathBuf;

use tokio::io::AsyncReadExt;

use crate::WorkloadIdentityError;

const MAX_ASSERTION_BYTES: u64 = 16 * 1024;

/// Source of the upstream assertion. File sources are reread for every exchange.
#[derive(Clone)]
pub enum WorkloadIdentityAssertionSource {
    Environment(String),
    File(PathBuf),
}

impl WorkloadIdentityAssertionSource {
    pub(crate) async fn assertion(&self) -> Result<String, WorkloadIdentityError> {
        let assertion = match self {
            Self::Environment(assertion) => assertion.clone(),
            Self::File(path) => {
                let file = tokio::fs::File::open(path).await.map_err(|source| {
                    WorkloadIdentityError::TokenFile {
                        path: path.clone(),
                        source,
                    }
                })?;
                let mut bytes = Vec::new();
                file.take(MAX_ASSERTION_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|source| WorkloadIdentityError::TokenFile {
                        path: path.clone(),
                        source,
                    })?;
                if bytes.len() as u64 > MAX_ASSERTION_BYTES {
                    return Err(WorkloadIdentityError::AssertionTooLarge);
                }
                String::from_utf8(bytes).map_err(|_| WorkloadIdentityError::InvalidAssertion)?
            }
        };
        let assertion = assertion.trim();
        if assertion.len() as u64 > MAX_ASSERTION_BYTES {
            return Err(WorkloadIdentityError::AssertionTooLarge);
        }
        if assertion.is_empty() || assertion.as_bytes().contains(&0) {
            return Err(WorkloadIdentityError::InvalidAssertion);
        }
        Ok(assertion.to_string())
    }
}

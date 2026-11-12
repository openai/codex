use std::path::Path;

use tokio::io::AsyncReadExt;

use crate::AssertionSource;
use crate::WorkloadIdentityError;

const MAX_ASSERTION_BYTES: u64 = 16 * 1024;

impl AssertionSource {
    pub(crate) async fn acquire(&self) -> Result<String, WorkloadIdentityError> {
        match self {
            Self::File(path) => read_assertion(path).await,
            Self::Spiffe(source) => source.fetch_assertion().await,
        }
    }
}

/// Reopens the assertion file for each exchange so its owner can rotate the credential.
pub(crate) async fn read_assertion(path: &Path) -> Result<String, WorkloadIdentityError> {
    let file = tokio::fs::File::open(path).await.map_err(|source| {
        WorkloadIdentityError::AssertionFile {
            path: path.to_path_buf(),
            source: source.into(),
        }
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_ASSERTION_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|source| WorkloadIdentityError::AssertionFile {
            path: path.to_path_buf(),
            source: source.into(),
        })?;
    if bytes.len() as u64 > MAX_ASSERTION_BYTES {
        return Err(WorkloadIdentityError::AssertionTooLarge);
    }
    let assertion =
        String::from_utf8(bytes).map_err(|_| WorkloadIdentityError::InvalidAssertion)?;
    validate_assertion(&assertion)
}

pub(crate) fn validate_assertion(assertion: &str) -> Result<String, WorkloadIdentityError> {
    if assertion.len() as u64 > MAX_ASSERTION_BYTES {
        return Err(WorkloadIdentityError::AssertionTooLarge);
    }
    let assertion = assertion.trim();
    if assertion.is_empty() || assertion.as_bytes().contains(&0) {
        return Err(WorkloadIdentityError::InvalidAssertion);
    }
    Ok(assertion.to_string())
}

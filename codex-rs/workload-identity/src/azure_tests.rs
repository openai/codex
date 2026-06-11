use std::path::PathBuf;

use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn reads_and_trims_projected_token_file() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let token_file = temp_dir.path().join("azure-token");
    tokio::fs::write(&token_file, "  header.payload.signature\n").await?;
    let source = AzureSubjectTokenSource::new(Some(token_file));

    assert_eq!(source.subject_token().await?, "header.payload.signature");
    Ok(())
}

#[tokio::test]
async fn rejects_empty_projected_token_file() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let token_file = temp_dir.path().join("azure-token");
    tokio::fs::write(&token_file, " \n").await?;
    let source = AzureSubjectTokenSource::new(Some(PathBuf::from(token_file)));

    assert!(matches!(
        source.subject_token().await,
        Err(WorkloadIdentityError::EmptySubjectToken)
    ));
    Ok(())
}

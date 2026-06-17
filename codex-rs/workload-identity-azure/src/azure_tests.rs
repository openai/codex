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

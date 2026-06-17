use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn file_source_rereads_rotated_token() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("subject-token");
    tokio::fs::write(&path, "first.token.value\n").await?;
    let source = FileSubjectTokenSource::new(path.clone());

    assert_eq!(source.subject_token().await?.value(), "first.token.value");
    tokio::fs::write(path, "second.token.value\n").await?;
    assert_eq!(source.subject_token().await?.value(), "second.token.value");
    Ok(())
}

#[tokio::test]
async fn environment_source_captures_startup_value() -> anyhow::Result<()> {
    const VARIABLE: &str = "CODEX_WIF_SOURCE_CAPTURE_TEST";
    let source = EnvironmentSubjectTokenSource {
        variable: VARIABLE.to_string(),
        captured: CapturedEnvironmentValue::Value("first.token.value".to_string()),
    };

    assert_eq!(source.subject_token().await?.value(), "first.token.value");
    Ok(())
}

#[test]
fn subject_token_debug_is_redacted() -> anyhow::Result<()> {
    let token = SubjectToken::jwt("secret.subject.token", "test")?;

    assert_eq!(
        format!("{token:?}"),
        "SubjectToken { value: \"[REDACTED]\", token_type: \"urn:ietf:params:oauth:token-type:jwt\" }"
    );
    Ok(())
}

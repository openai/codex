#![cfg(test)]

use super::*;
use std::sync::Arc;

#[tokio::test]
async fn spawn_trace_omits_sensitive_inputs() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    let capture = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(Arc::new(capture.reopen()?))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let result = spawn_child_async(SpawnChildRequest {
        program: dir.path().join("missing-test-program"),
        args: vec!["canary-argument".into()],
        arg0: Some("canary-arg0"),
        cwd: AbsolutePathBuf::from_absolute_path(dir.path())?,
        network_sandbox_policy: NetworkSandboxPolicy::Enabled,
        network: None,
        stdio_policy: StdioPolicy::RedirectForShellTool,
        env: HashMap::from([("SETTING".into(), "canary-env-value".into())]),
    })
    .await;
    assert!(result.is_err());

    let logs = std::fs::read_to_string(capture.path())?;
    assert!(logs.contains("missing-test-program"));
    assert!(!logs.contains("canary-"));
    Ok(())
}

use anyhow::Result;
use codex_exec_server::RemoveOptions;
use core_test_support::PathBufExt;
use core_test_support::get_remote_test_env;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path_abs = remote_test_file_path().abs();
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_abs, payload.clone())
        .await?;
    let actual = file_system.read_file(&file_path_abs).await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_abs,
            RemoveOptions {
                recursive: false,
                force: true,
            },
        )
        .await?;

    Ok(())
}
fn remote_test_file_path() -> PathBuf {
    let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    PathBuf::from(format!(
        "/tmp/codex-remote-test-env-{}-{nanos}.txt",
        std::process::id()
    ))
}

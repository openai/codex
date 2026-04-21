mod common;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::EnvironmentSpec;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::RemoteExecServerTransport;
use codex_exec_server::StaticEnvironmentProvider;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn configured_command_environment_connects_lazily_over_stdio() -> anyhow::Result<()> {
    let helper_paths = common::exec_server::test_codex_helper_paths()?;
    let temp_dir = tempfile::tempdir()?;
    let target_path = temp_dir.path().join("target.txt");
    let marker_path = temp_dir.path().join("spawned.txt");
    tokio::fs::write(&target_path, "ok").await?;

    let provider = StaticEnvironmentProvider {
        default_environment: Some("remote".to_string()),
        environments: vec![EnvironmentSpec {
            id: "remote".to_string(),
            transport: RemoteExecServerTransport::Command {
                command: format!(
                    "echo spawned > {marker_path:?}; exec {codex_exe:?} exec-server --listen stdio://",
                    marker_path = marker_path,
                    codex_exe = helper_paths.codex_exe,
                ),
            },
        }],
    };
    let manager = EnvironmentManager::try_from_provider(
        &provider,
        ExecServerRuntimePaths::new(helper_paths.codex_exe.clone(), None)?,
    )
    .await?;
    let environment = manager.default_environment().expect("default environment");

    assert!(
        tokio::fs::metadata(&marker_path).await.is_err(),
        "command transport should not connect before the first remote operation"
    );

    let metadata = environment
        .get_filesystem()
        .get_metadata(
            &AbsolutePathBuf::from_absolute_path(&target_path)?,
            /*sandbox*/ None,
        )
        .await?;

    assert_eq!(metadata.is_file, true);
    assert_eq!(tokio::fs::read_to_string(&marker_path).await?, "spawned\n");
    Ok(())
}

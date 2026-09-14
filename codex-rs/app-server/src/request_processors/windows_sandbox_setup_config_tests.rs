//! Exercise the setup RPC's real configuration loading without changing Windows accounts or ACLs.

use super::ConfigManager;
use super::load_setup_config;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn omitted_cwd_does_not_make_the_server_directory_a_workspace() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let install = tempfile::tempdir()?;
    let manager = ConfigManager::without_managed_config_for_tests(home.path().to_path_buf());
    let (config, command_cwd) =
        load_setup_config(&manager, install.path(), /*requested_cwd*/ None).await?;

    assert_eq!(
        (command_cwd, config.effective_workspace_roots()),
        (install.path().to_path_buf(), Vec::new()),
    );
    Ok(())
}

#[tokio::test]
async fn explicit_cwd_remains_the_setup_workspace() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let install = tempfile::tempdir()?;
    let project = tempfile::tempdir()?;
    let manager = ConfigManager::without_managed_config_for_tests(home.path().to_path_buf());
    let (config, command_cwd) =
        load_setup_config(&manager, install.path(), Some(project.path().to_path_buf())).await?;
    let expected = AbsolutePathBuf::from_absolute_path(project.path())?;

    assert_eq!(
        (command_cwd, config.effective_workspace_roots()),
        (project.path().to_path_buf(), vec![expected]),
    );
    Ok(())
}

use anyhow::Context;
use anyhow::Result;
use app_test_support::EnvironmentConfigFile;
use app_test_support::TestAppServerEnvironment;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use core_test_support::TestEnvironment;
use core_test_support::test_environment;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test]
async fn generated_config_registers_selected_remote_as_default() -> Result<()> {
    let mut environment = TestAppServerEnvironment::new().await?;
    let environments_toml =
        environment.generated_environments_toml(&[], EnvironmentConfigFile::Missing)?;

    match test_environment() {
        TestEnvironment::Local => assert_eq!(environments_toml, None),
        TestEnvironment::Docker { .. } | TestEnvironment::WineExec => {
            let contents = environments_toml.context("generated environments.toml")?;
            let codex_home = TempDir::new()?;
            std::fs::write(codex_home.path().join("environments.toml"), &contents)?;
            let local_runtime_paths = ExecServerRuntimePaths::new(
                std::env::current_exe()?,
                /*codex_linux_sandbox_exe*/ None,
            )?;
            let manager =
                EnvironmentManager::from_codex_home(codex_home.path(), Some(local_runtime_paths))
                    .await?;

            assert_eq!(
                manager.default_environment_id(),
                Some(REMOTE_ENVIRONMENT_ID)
            );
            match test_environment() {
                TestEnvironment::Local => unreachable!("remote test branch"),
                TestEnvironment::Docker { .. } => {
                    assert!(manager.try_local_environment().is_some());
                }
                TestEnvironment::WineExec => {
                    assert!(manager.try_local_environment().is_none());
                }
            }
            let remote = manager
                .get_environment(REMOTE_ENVIRONMENT_ID)
                .context("remote environment")?;
            assert!(remote.exec_server_url().is_some());
        }
    }

    Ok(())
}

#[tokio::test]
async fn generated_config_respects_explicit_overrides_or_rejects_them_under_wine() -> Result<()> {
    let mut environment = TestAppServerEnvironment::new().await?;
    let generated = environment.generated_environments_toml(&[], EnvironmentConfigFile::Missing)?;

    match test_environment() {
        TestEnvironment::Local => {
            assert_eq!(generated, None);
            assert_eq!(
                environment.generated_environments_toml(
                    &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("none"))],
                    EnvironmentConfigFile::Missing,
                )?,
                None
            );
        }
        TestEnvironment::Docker { .. } => {
            assert!(generated.is_some());

            for override_value in [Some("none"), None] {
                assert_eq!(
                    environment.generated_environments_toml(
                        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, override_value)],
                        EnvironmentConfigFile::Missing,
                    )?,
                    None
                );
            }
            assert_eq!(
                environment.generated_environments_toml(&[], EnvironmentConfigFile::TestOwned)?,
                None
            );
        }
        TestEnvironment::WineExec => {
            assert!(generated.is_some());

            for override_value in [Some("none"), None] {
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    environment.generated_environments_toml(
                        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, override_value)],
                        EnvironmentConfigFile::Missing,
                    )
                }));
                assert!(panic.is_err());
            }
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                environment.generated_environments_toml(&[], EnvironmentConfigFile::TestOwned)
            }));
            assert!(panic.is_err());
        }
    }

    Ok(())
}

#[tokio::test]
async fn thread_environment_is_available_only_for_harness_generated_config() -> Result<()> {
    match test_environment() {
        TestEnvironment::Local => {
            let mut environment = TestAppServerEnvironment::new().await?;
            assert_eq!(
                environment.generated_environments_toml(&[], EnvironmentConfigFile::Missing)?,
                None
            );
            assert_eq!(environment.generated_thread_environment(), None);
        }
        TestEnvironment::Docker { .. } | TestEnvironment::WineExec => {
            let mut generated_environment = TestAppServerEnvironment::new().await?;
            generated_environment
                .generated_environments_toml(&[], EnvironmentConfigFile::Missing)?
                .context("generated environments.toml")?;
            let selected = generated_environment
                .generated_thread_environment()
                .context("generated thread environment")?;
            assert_eq!(selected.environment_id, REMOTE_ENVIRONMENT_ID);
            assert!(selected.cwd.to_inferred_path_uri().is_some());

            let mut restarted_environment = TestAppServerEnvironment::new().await?;
            assert_eq!(
                restarted_environment
                    .generated_environments_toml(&[], EnvironmentConfigFile::HarnessGenerated,)?,
                None
            );
            assert!(
                restarted_environment
                    .generated_thread_environment()
                    .is_some()
            );

            match test_environment() {
                TestEnvironment::Local => unreachable!("remote test branch"),
                TestEnvironment::Docker { .. } => {
                    let mut configured_environment = TestAppServerEnvironment::new().await?;
                    assert_eq!(
                        configured_environment
                            .generated_environments_toml(&[], EnvironmentConfigFile::TestOwned,)?,
                        None
                    );
                    assert_eq!(configured_environment.generated_thread_environment(), None);

                    let mut overridden_environment = TestAppServerEnvironment::new().await?;
                    assert_eq!(
                        overridden_environment.generated_environments_toml(
                            &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("test-owned"))],
                            EnvironmentConfigFile::Missing,
                        )?,
                        None
                    );
                    assert_eq!(overridden_environment.generated_thread_environment(), None);
                }
                TestEnvironment::WineExec => {}
            }
        }
    }

    Ok(())
}

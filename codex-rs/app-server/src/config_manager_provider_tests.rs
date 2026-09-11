//! Checks retained providers against managed policy without reloading session configuration.

use super::*;
use anyhow::Result;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::ThreadConfigContext;
use codex_config::ThreadConfigLoadError;
use codex_config::ThreadConfigLoadErrorCode;
use codex_config::ThreadConfigLoaderFuture;
use codex_config::ThreadConfigSource;
use codex_config::test_support::CloudConfigBundleFixture;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;

#[test_case(AMAZON_BEDROCK_PROVIDER_ID; "bedrock")]
#[test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID; "bedrock_runtime")]
#[tokio::test]
async fn provider_requirements_resolve_bedrock_overrides(provider_id: &str) -> Result<()> {
    let home = tempdir()?;
    let requirements = format!(
        "model_provider = '{provider_id}'\n[model_providers.{provider_id}.aws]\nregion = 'us-west-2'"
    );
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(&requirements),
    );
    let current = manager.load_latest_config(/*fallback_cwd*/ None).await?;
    manager.check_thread_model_provider(&current).await?;

    let changed = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(
            requirements.replace("us-west-2", "us-east-1"),
        ),
    );
    assert_eq!(
        changed
            .check_thread_model_provider(&current)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied,
    );
    Ok(())
}

struct UnavailableThreadConfig;

impl ThreadConfigLoader for UnavailableThreadConfig {
    fn load(
        &self,
        _context: ThreadConfigContext,
    ) -> ThreadConfigLoaderFuture<'_, Vec<ThreadConfigSource>> {
        Box::pin(async {
            Err(ThreadConfigLoadError::new(
                ThreadConfigLoadErrorCode::RequestFailed,
                /*status_code*/ None,
                "thread config service unavailable",
            ))
        })
    }
}

#[tokio::test]
async fn provider_requirements_do_not_reload_thread_config() -> Result<()> {
    let home = tempdir()?;
    let mut manager = ConfigManager::without_managed_config_for_tests(home.path().to_path_buf());
    let current = manager.load_latest_config(/*fallback_cwd*/ None).await?;
    manager.thread_config_loader = Arc::new(UnavailableThreadConfig);
    assert!(
        manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
            .is_err()
    );
    manager.check_thread_model_provider(&current).await?;

    manager.cloud_config_bundle = Arc::new(RwLock::new(
        CloudConfigBundleFixture::loader_with_enterprise_requirement("model_provider = 'other'"),
    ));
    assert_eq!(
        manager
            .check_thread_model_provider(&current)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied,
    );
    Ok(())
}

#[tokio::test]
async fn provider_requirements_ignore_system_defaults_but_reject_requirement_changes() -> Result<()>
{
    let home = tempdir()?;
    let system_config_path = home.path().join("system-config.toml");
    let requirements_path = home.path().join("requirements.toml");
    let managed_config_path = home.path().join("managed-config.toml");
    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.system_config_path = Some(system_config_path.clone());
    overrides.system_requirements_path = Some(requirements_path.clone());
    overrides.managed_config_path = Some(managed_config_path.clone());
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        overrides,
        CloudConfigBundleLoader::default(),
    );
    let current = manager.load_latest_config(/*fallback_cwd*/ None).await?;

    std::fs::write(&system_config_path, "invalid toml !!!")?;
    manager.check_thread_model_provider(&current).await?;

    std::fs::write(&requirements_path, "model_provider = 'other'")?;
    assert_eq!(
        manager
            .check_thread_model_provider(&current)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied,
    );

    std::fs::write(&requirements_path, "invalid toml !!!")?;
    assert_eq!(
        manager
            .check_thread_model_provider(&current)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData,
    );

    std::fs::write(
        &requirements_path,
        "[model_providers.gateway]\nbase_url = 'https://example.test'",
    )?;
    assert_eq!(
        manager
            .check_thread_model_provider(&current)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData,
    );
    std::fs::remove_file(requirements_path)?;

    std::fs::write(managed_config_path, "invalid toml !!!")?;
    assert_eq!(
        manager
            .check_thread_model_provider(&current)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData,
    );
    Ok(())
}

#[tokio::test]
async fn provider_requirement_load_errors_reject_input() -> Result<()> {
    let home = tempdir()?;
    let previous = ConfigManager::without_managed_config_for_tests(home.path().to_path_buf())
        .load_latest_config(/*fallback_cwd*/ None)
        .await?;
    for loader in [
        CloudConfigBundleFixture::loader_with_enterprise_requirement("invalid toml !!!"),
        CloudConfigBundleLoader::new(async {
            Err(CloudConfigBundleLoadError::new(
                CloudConfigBundleLoadErrorCode::RequestFailed,
                /*status_code*/ None,
                "policy unavailable",
            ))
        }),
    ] {
        let manager = ConfigManager::new_for_tests(
            home.path().to_path_buf(),
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            loader,
        );
        assert!(
            manager
                .check_thread_model_provider(&previous)
                .await
                .is_err()
        );
    }
    Ok(())
}

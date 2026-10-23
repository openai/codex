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

#[test_case(AMAZON_BEDROCK_PROVIDER_ID, "aws.region = 'us-west-2'", "us-west-2", "us-east-1"; "bedrock")]
#[test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "aws.region = 'us-west-2'", "us-west-2", "us-east-1"; "bedrock_runtime")]
#[test_case(GCP_PROVIDER_ID, "project_id = 'example-project'\nlocation = 'us-central1'", "example-project", "other-project"; "gcp_project")]
#[test_case(GCP_PROVIDER_ID, "project_id = 'example-project'\nlocation = 'us-central1'", "us-central1", "us-east4"; "gcp_location")]
#[test_case(GCP_PROVIDER_ID, "project_id = 'example-project'\nauth.command = 'print-token'", "print-token", "other-token"; "gcp_auth")]
#[tokio::test]
async fn provider_requirements_resolve_builtin_overrides(
    provider_id: &str,
    provider_config: &str,
    original: &str,
    replacement: &str,
) -> Result<()> {
    let home = tempdir()?;
    let workspace = tempdir()?;
    let requirements = format!(
        "model_provider = '{provider_id}'\n[model_providers.{provider_id}]\n{provider_config}"
    );
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(&requirements),
    );
    let current = manager
        .load_latest_config(Some(workspace.path().to_path_buf()))
        .await?;
    manager.check_thread_model_provider(&current).await?;

    let changed = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(
            requirements.replace(original, replacement),
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
    manager.thread_config_loader = Arc::new(codex_config::StaticThreadConfigLoader::new(vec![
        ThreadConfigSource::Session(codex_config::SessionThreadConfig {
            model_provider: Some("retained".into()),
            model_providers: toml::from_str("[retained]\nname = 'Retained'")?,
            ..Default::default()
        }),
    ]));
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
        CloudConfigBundleFixture::loader_with_enterprise_requirement("model_provider = 'retained'"),
    ));
    let retained = manager
        .load_retained_session_config(&current.config_layer_stack, &current.cwd)
        .await?;
    assert_eq!(retained.model_provider, current.model_provider);
    let factory = current.http_client_factory();
    assert_eq!(retained.http_client_factory(), factory);

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

/// A managed startup selection must disappear when its policy is removed.
#[tokio::test]
async fn catalog_reload_uses_latest_model_provider() -> Result<()> {
    let home = tempdir()?;
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(
            "model_provider = 'gateway'\n[model_providers.gateway]\nname = 'Gateway'\nbase_url = 'https://example.test'",
        ),
    );
    let startup = manager.load_non_project_config().await?;
    assert_eq!(startup.model_provider_id, "gateway");
    manager.clear_chatgpt_config_loader();
    let refreshed = manager
        .load_startup_config(/*fallback_cwd*/ None)
        .await?
        .config;
    assert_eq!(refreshed.model_provider_id, "openai");
    Ok(())
}

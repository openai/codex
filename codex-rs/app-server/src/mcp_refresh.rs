use crate::config_manager::ConfigManager;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_protocol::ThreadId;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::Op;
use std::io;
use std::sync::Arc;
use tracing::warn;

pub(crate) async fn queue_strict_refresh(
    thread_manager: &Arc<ThreadManager>,
    config_manager: &ConfigManager,
) -> io::Result<()> {
    config_manager
        .load_latest_config(/*fallback_cwd*/ None)
        .await?;
    let mut refreshes = Vec::new();
    for thread_id in thread_manager.list_thread_ids().await {
        let thread = thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|err| io::Error::other(format!("failed to load thread {thread_id}: {err}")))?;
        let config =
            build_refresh_config(thread_manager, config_manager, thread.config().await).await?;
        refreshes.push((thread_id, thread, config));
    }
    for (thread_id, thread, config) in refreshes {
        queue_refresh(thread_id, thread, config).await?;
    }
    Ok(())
}

pub(crate) async fn queue_best_effort_refresh(
    thread_manager: &Arc<ThreadManager>,
    config_manager: &ConfigManager,
) {
    for thread_id in thread_manager.list_thread_ids().await {
        let thread = match thread_manager.get_thread(thread_id).await {
            Ok(thread) => thread,
            Err(err) => {
                warn!("failed to load thread {thread_id} for MCP refresh: {err}");
                continue;
            }
        };
        let config =
            match build_refresh_config(thread_manager, config_manager, thread.config().await).await
            {
                Ok(config) => config,
                Err(err) => {
                    warn!("failed to build MCP refresh config for thread {thread_id}: {err}");
                    continue;
                }
            };
        if let Err(err) = queue_refresh(thread_id, thread, config).await {
            warn!("{err}");
        }
    }
}

async fn build_refresh_config(
    thread_manager: &ThreadManager,
    config_manager: &ConfigManager,
    thread_config: Arc<Config>,
) -> io::Result<McpServerRefreshConfig> {
    let config = config_manager
        .load_latest_config_for_thread(thread_config.as_ref())
        .await?;
    let mcp_servers = thread_manager
        .mcp_manager()
        .configured_servers(&config)
        .await;
    Ok(McpServerRefreshConfig {
        mcp_servers: serde_json::to_value(mcp_servers).map_err(io::Error::other)?,
        mcp_oauth_credentials_store_mode: serde_json::to_value(
            config.mcp_oauth_credentials_store_mode,
        )
        .map_err(io::Error::other)?,
    })
}

async fn queue_refresh(
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    config: McpServerRefreshConfig,
) -> io::Result<()> {
    thread
        .submit(Op::RefreshMcpServers { config })
        .await
        .map(|_| ())
        .map_err(|err| {
            io::Error::other(format!(
                "failed to queue MCP refresh for thread {thread_id}: {err}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::thread_extensions;
    use async_trait::async_trait;
    use codex_arg0::Arg0DispatchPaths;
    use codex_config::CloudRequirementsLoader;
    use codex_config::LoaderOverrides;
    use codex_config::ThreadConfigContext;
    use codex_config::ThreadConfigLoadError;
    use codex_config::ThreadConfigLoadErrorCode;
    use codex_config::ThreadConfigLoader;
    use codex_config::ThreadConfigSource;
    use codex_core::config::ConfigOverrides;
    use codex_core::init_state_db;
    use codex_core::thread_store_from_config;
    use codex_exec_server::EnvironmentManager;
    use codex_login::AuthManager;
    use codex_login::CodexAuth;
    use codex_protocol::protocol::SessionSource;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    #[tokio::test]
    async fn strict_refresh_reports_thread_planning_failures() -> anyhow::Result<()> {
        let (_temp_dir, thread_manager, config_manager, _loader) = refresh_test_state().await?;

        let err = queue_strict_refresh(&thread_manager, &config_manager)
            .await
            .expect_err("strict refresh should fail");

        assert_eq!(err.to_string(), "failed to load refresh config");
        Ok(())
    }

    #[tokio::test]
    async fn best_effort_refresh_attempts_every_loaded_thread() -> anyhow::Result<()> {
        let (_temp_dir, thread_manager, config_manager, loader) = refresh_test_state().await?;

        queue_best_effort_refresh(&thread_manager, &config_manager).await;

        assert_eq!(loader.good_loads.load(Ordering::Relaxed), 1);
        assert_eq!(loader.bad_loads.load(Ordering::Relaxed), 1);
        Ok(())
    }

    async fn refresh_test_state() -> anyhow::Result<(
        TempDir,
        Arc<ThreadManager>,
        ConfigManager,
        Arc<CountingThreadConfigLoader>,
    )> {
        let temp_dir = TempDir::new()?;
        let good_cwd = temp_dir.path().join("good");
        let bad_cwd = temp_dir.path().join("bad");
        std::fs::create_dir_all(&good_cwd)?;
        std::fs::create_dir_all(&bad_cwd)?;

        let initial_config_manager =
            ConfigManager::without_managed_config_for_tests(temp_dir.path().to_path_buf());
        let good_config = initial_config_manager
            .load_for_cwd(
                /*request_overrides*/ None,
                ConfigOverrides::default(),
                Some(good_cwd.clone()),
            )
            .await?;
        let bad_config = initial_config_manager
            .load_for_cwd(
                /*request_overrides*/ None,
                ConfigOverrides::default(),
                Some(bad_cwd.clone()),
            )
            .await?;

        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
        let state_db = init_state_db(&good_config)
            .await
            .expect("refresh tests require state db");
        let thread_store = thread_store_from_config(&good_config, Some(state_db.clone()));
        let thread_manager = Arc::new(ThreadManager::new(
            &good_config,
            auth_manager,
            SessionSource::Exec,
            Arc::new(EnvironmentManager::default_for_tests()),
            thread_extensions(),
            /*analytics_events_client*/ None,
            thread_store,
            Some(state_db.clone()),
            "11111111-1111-4111-8111-111111111111".to_string(),
            /*attestation_provider*/ None,
        ));
        thread_manager.start_thread(good_config).await?;
        thread_manager.start_thread(bad_config).await?;

        let loader = Arc::new(CountingThreadConfigLoader {
            good_cwd: AbsolutePathBuf::try_from(good_cwd)?,
            bad_cwd: AbsolutePathBuf::try_from(bad_cwd)?,
            good_loads: AtomicUsize::new(0),
            bad_loads: AtomicUsize::new(0),
        });
        let config_manager = ConfigManager::new(
            temp_dir.path().to_path_buf(),
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            CloudRequirementsLoader::default(),
            Arg0DispatchPaths::default(),
            loader.clone(),
        );

        Ok((temp_dir, thread_manager, config_manager, loader))
    }

    struct CountingThreadConfigLoader {
        good_cwd: AbsolutePathBuf,
        bad_cwd: AbsolutePathBuf,
        good_loads: AtomicUsize,
        bad_loads: AtomicUsize,
    }

    #[async_trait]
    impl ThreadConfigLoader for CountingThreadConfigLoader {
        async fn load(
            &self,
            context: ThreadConfigContext,
        ) -> Result<Vec<ThreadConfigSource>, ThreadConfigLoadError> {
            if context.cwd.as_ref() == Some(&self.good_cwd) {
                self.good_loads.fetch_add(1, Ordering::Relaxed);
            }
            if context.cwd.as_ref() == Some(&self.bad_cwd) {
                self.bad_loads.fetch_add(1, Ordering::Relaxed);
                return Err(ThreadConfigLoadError::new(
                    ThreadConfigLoadErrorCode::Internal,
                    /*status_code*/ None,
                    "failed to load refresh config",
                ));
            }
            Ok(Vec::new())
        }
    }
}

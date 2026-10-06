//! Cancelling partially initialized sessions must release their persistent writers.

use codex_protocol::protocol::TurnEnvironmentSelection;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::config::Config;
use codex_core::config::ThreadStoreConfig;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_rollout::RolloutRecorder;
use codex_thread_store::LocalThreadStore;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Notify;
use tokio::time::timeout;

struct StartupMcpBarrier {
    block_next: AtomicBool,
    entered: Notify,
}

impl McpServerContributor<Config> for StartupMcpBarrier {
    fn id(&self) -> &'static str {
        "cancelled_startup_barrier"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            if self.block_next.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                std::future::pending::<()>().await;
            }
            Vec::new()
        })
    }
}

#[tokio::test]
async fn cancelled_resume_releases_writer_while_mcp_startup_is_pending() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let barrier = Arc::new(StartupMcpBarrier {
        block_next: AtomicBool::new(false),
        entered: Notify::new(),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(barrier.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| config.experimental_thread_store = ThreadStoreConfig::Local)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let environments = test.codex.environment_selections().await;
    test.codex.ensure_rollout_materialized().await;
    let rollout_path = test.codex.rollout_path().context("thread rollout")?;
    test.codex.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&thread_id).await;
    let history = RolloutRecorder::get_rollout_history(&rollout_path).await?;
    let store = test
        .thread_store
        .as_any()
        .downcast_ref::<LocalThreadStore>()
        .context("local thread store")?;
    let resume_options = || StartThreadOptions {
        initial_history: history.clone(),
        environments: Some(
            environments
                .clone()
                .into_iter()
                .map(TurnEnvironmentSelection::into_request)
                .collect(),
        ),
        ..StartThreadOptions::new(test.config.clone())
    };

    barrier.block_next.store(true, Ordering::SeqCst);
    let mut resume = Box::pin(test.thread_manager.start_thread(resume_options()));
    timeout(Duration::from_secs(10), async {
        tokio::select! {
            biased;
            _ = &mut resume => panic!("resume must wait for MCP startup"),
            _ = async {
                barrier.entered.notified().await;
                while store.live_rollout_path(thread_id).await.is_err() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            } => {}
        }
    })
    .await
    .context("resume should open persistence while MCP startup waits")?;
    drop(resume);

    // The guard schedules asynchronous cleanup. Resuming then also waits for discard's writer lock.
    let resumed = timeout(Duration::from_secs(10), async {
        while store.live_rollout_path(thread_id).await.is_ok() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        test.thread_manager.start_thread(resume_options()).await
    })
    .await
    .context("cancelled startup should release its writer")??;
    assert_eq!(resumed.thread_id, thread_id);
    resumed.thread.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn failed_resume_releases_writer_before_returning() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| config.experimental_thread_store = ThreadStoreConfig::Local)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let environments = test.codex.environment_selections().await;
    test.codex.ensure_rollout_materialized().await;
    let rollout_path = test.codex.rollout_path().context("thread rollout")?;
    test.codex.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&thread_id).await;
    let history = RolloutRecorder::get_rollout_history(&rollout_path).await?;

    let mcp_server = start_mock_server().await;
    let (http_server, control) = AppsTestServer::mount_with_startup_control(&mcp_server).await?;
    control.fail_next_initialize_attempts(/*attempts*/ 1);
    let mut config = test.config.clone();
    let mut servers = config.mcp_servers.get().clone();
    servers.insert(
        "fails-once".to_owned(),
        serde_json::from_value(json!({
            "url": format!("{}/api/codex/ps/mcp", http_server.chatgpt_base_url),
            "http_headers": { "Authorization": "Bearer synthetic-test-token" },
            "required": true,
        }))?,
    );
    config.mcp_servers.set(servers)?;
    let resume_options = || StartThreadOptions {
        initial_history: history.clone(),
        environments: Some(
            environments
                .clone()
                .into_iter()
                .map(TurnEnvironmentSelection::into_request)
                .collect(),
        ),
        ..StartThreadOptions::new(config.clone())
    };

    assert!(
        test.thread_manager
            .start_thread(resume_options())
            .await
            .is_err(),
        "first resume should fail MCP initialization"
    );
    let resumed = test.thread_manager.start_thread(resume_options()).await?;

    assert_eq!(resumed.thread_id, thread_id);
    resumed.thread.shutdown_and_wait().await?;
    Ok(())
}

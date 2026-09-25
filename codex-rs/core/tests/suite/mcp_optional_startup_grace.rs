//! Covers optional MCP startup deadlines in the initial model-visible tool catalog.

use std::time::Duration;

use anyhow::Context;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

const SERVER_NAME: &str = "optional_startup";
const TOOL_NAMESPACE: &str = "mcp__optional_startup";
const TOOL_NAME: &str = "calendar_create_event";
// Allow the whole turn to finish on slow runners without reaching the server's own timeout.
const TURN_TIMEOUT: Duration = Duration::from_secs(5);
const PENDING_SERVER_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum StartupGraceScenario {
    ShortGraceOmitsPending,
    CustomGraceIncludesReady,
    DisabledGraceWaitsForStartup,
    DisabledGraceRespectsStartupTimeout,
}

#[derive(Clone, Copy)]
enum ConfigRefreshPath {
    Runtime,
    Mcp,
}

#[test_case(StartupGraceScenario::ShortGraceOmitsPending; "custom grace omits a pending server")]
#[test_case(StartupGraceScenario::CustomGraceIncludesReady; "custom grace admits a ready server")]
#[test_case(StartupGraceScenario::DisabledGraceWaitsForStartup; "zero grace waits for server startup")]
#[test_case(StartupGraceScenario::DisabledGraceRespectsStartupTimeout; "zero grace respects server startup timeout")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn optional_mcp_startup_grace_controls_initial_turn_tool_catalog(
    scenario: StartupGraceScenario,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_grace = match scenario {
        StartupGraceScenario::ShortGraceOmitsPending => Duration::from_millis(50),
        StartupGraceScenario::CustomGraceIncludesReady => Duration::from_millis(250),
        StartupGraceScenario::DisabledGraceWaitsForStartup
        | StartupGraceScenario::DisabledGraceRespectsStartupTimeout => Duration::ZERO,
    };
    let startup_timeout = match scenario {
        StartupGraceScenario::ShortGraceOmitsPending => PENDING_SERVER_TIMEOUT,
        StartupGraceScenario::DisabledGraceRespectsStartupTimeout => Duration::from_millis(250),
        StartupGraceScenario::CustomGraceIncludesReady
        | StartupGraceScenario::DisabledGraceWaitsForStartup => Duration::from_secs(1),
    };
    let responses_server = responses::start_mock_server().await;
    let mcp_server = responses::start_mock_server().await;
    let (http_server, startup_control) =
        AppsTestServer::mount_with_startup_control(&mcp_server).await?;
    let release_startup = startup_control.hold_next_successful_initialize();
    let server_url = format!("{}/api/codex/ps/mcp", http_server.chatgpt_base_url);
    let response = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-optional-startup"),
            responses::ev_assistant_message("msg-optional-startup", "done"),
            responses::ev_completed("resp-optional-startup"),
        ]),
    )
    .await;

    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(move |config| {
            config.mcp_optional_startup_grace = startup_grace;
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                serde_json::from_value(json!({
                    "url": server_url,
                    "http_headers": { "Authorization": "Bearer synthetic-test-token" },
                    "enabled_tools": [TOOL_NAME],
                    "startup_timeout_sec": startup_timeout.as_secs_f64(),
                }))
                .expect("synthetic optional MCP server configuration"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("optional MCP server should satisfy test configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;

    tokio::time::timeout(Duration::from_secs(5), async {
        while startup_control.initialize_attempts() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("optional MCP initialization should begin before the first turn")?;

    let mut turn = Box::pin(fixture.submit_turn("show optional MCP tools"));
    match scenario {
        StartupGraceScenario::ShortGraceOmitsPending => {
            tokio::time::timeout(TURN_TIMEOUT, &mut turn)
                .await
                .context("the configured grace should omit the pending server")??;
            release_startup
                .send(())
                .expect("pending optional MCP startup should remain in flight");
        }
        StartupGraceScenario::CustomGraceIncludesReady => {
            assert!(
                tokio::time::timeout(Duration::from_millis(25), &mut turn)
                    .await
                    .is_err(),
                "the first turn should wait while the custom startup grace remains available"
            );
            release_startup
                .send(())
                .expect("optional MCP startup should remain in flight");
            tokio::time::timeout(Duration::from_secs(2), &mut turn)
                .await
                .context("a server ready within the configured grace should reach the model")??;
        }
        StartupGraceScenario::DisabledGraceWaitsForStartup => {
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut turn)
                    .await
                    .is_err(),
                "zero grace should wait for startup rather than omitting the optional server"
            );
            release_startup
                .send(())
                .expect("zero-grace optional MCP startup should remain in flight");
            tokio::time::timeout(Duration::from_secs(2), &mut turn)
                .await
                .context("zero grace should include a server ready before its startup timeout")??;
        }
        StartupGraceScenario::DisabledGraceRespectsStartupTimeout => {
            assert!(
                tokio::time::timeout(Duration::from_millis(100), &mut turn)
                    .await
                    .is_err(),
                "zero grace should keep waiting until the server-specific startup timeout"
            );
            tokio::time::timeout(Duration::from_secs(1), &mut turn)
                .await
                .context("zero grace should stop waiting once the server startup times out")??;
            release_startup
                .send(())
                .expect("the test MCP initialization request should remain gated");
        }
    }

    let request = response.single_request().body_json();
    let server_tool = responses::namespace_child_tool(&request, TOOL_NAMESPACE, TOOL_NAME);
    match scenario {
        StartupGraceScenario::ShortGraceOmitsPending
        | StartupGraceScenario::DisabledGraceRespectsStartupTimeout => assert!(
            server_tool.is_none(),
            "a pending optional MCP tool must be absent after its configured grace expires"
        ),
        StartupGraceScenario::CustomGraceIncludesReady
        | StartupGraceScenario::DisabledGraceWaitsForStartup => assert!(
            server_tool.is_some(),
            "an optional MCP tool ready before its startup deadline must reach the model"
        ),
    }

    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(ConfigRefreshPath::Runtime; "runtime configuration refresh")]
#[test_case(ConfigRefreshPath::Mcp; "MCP configuration refresh")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn running_thread_uses_refreshed_optional_mcp_startup_grace(
    refresh_path: ConfigRefreshPath,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = responses::start_mock_server().await;
    let mcp_server = responses::start_mock_server().await;
    let (http_server, startup_control) =
        AppsTestServer::mount_with_startup_control(&mcp_server).await?;
    let release_startup = startup_control.hold_next_successful_initialize();
    let server_url = format!("{}/api/codex/ps/mcp", http_server.chatgpt_base_url);
    let initial_response = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-initial-grace"),
            responses::ev_assistant_message("msg-initial-grace", "done"),
            responses::ev_completed("resp-initial-grace"),
        ]),
    )
    .await;

    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(move |config| {
            config.mcp_optional_startup_grace = Duration::from_millis(50);
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                serde_json::from_value(json!({
                    "url": server_url,
                    "http_headers": { "Authorization": "Bearer synthetic-test-token" },
                    "enabled_tools": [TOOL_NAME],
                    "startup_timeout_sec": PENDING_SERVER_TIMEOUT.as_secs(),
                }))
                .expect("synthetic optional MCP server configuration"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("optional MCP server should satisfy test configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;

    tokio::time::timeout(Duration::from_secs(5), async {
        while startup_control.initialize_attempts() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("optional MCP initialization should begin before the initial turn")?;

    // Allow for turn setup on remote workers while still finishing before the
    // pending MCP startup timeout. The server remains gated throughout.
    tokio::time::timeout(
        TURN_TIMEOUT,
        fixture.submit_turn("show initial optional MCP tools"),
    )
    .await
    .context("the initial startup grace should omit the pending server")??;
    assert!(
        responses::namespace_child_tool(
            &initial_response.single_request().body_json(),
            TOOL_NAMESPACE,
            TOOL_NAME,
        )
        .is_none(),
        "the pending optional MCP tool should be absent before configuration refresh"
    );

    let refreshed_grace = Duration::from_millis(250);
    let mut refreshed_config = fixture.config.clone();
    refreshed_config.mcp_optional_startup_grace = refreshed_grace;
    match refresh_path {
        ConfigRefreshPath::Runtime => {
            fixture.codex.refresh_runtime_config(refreshed_config).await;
        }
        ConfigRefreshPath::Mcp => {
            fixture.codex.refresh_mcp_config(refreshed_config).await;
        }
    }
    assert_eq!(
        fixture.codex.config().await.mcp_optional_startup_grace,
        refreshed_grace,
        "the existing thread should retain the refreshed optional MCP startup grace"
    );

    let refreshed_response = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-refreshed-grace"),
            responses::ev_assistant_message("msg-refreshed-grace", "done"),
            responses::ev_completed("resp-refreshed-grace"),
        ]),
    )
    .await;
    let mut refreshed_turn = Box::pin(fixture.submit_turn("show refreshed optional MCP tools"));
    assert!(
        tokio::time::timeout(Duration::from_millis(25), &mut refreshed_turn)
            .await
            .is_err(),
        "the refreshed startup grace should wait for the existing pending MCP server"
    );
    release_startup
        .send(())
        .expect("the existing optional MCP startup should remain in flight");
    tokio::time::timeout(Duration::from_secs(2), &mut refreshed_turn)
        .await
        .context("an optional MCP ready within the refreshed grace should reach the model")??;
    assert!(
        responses::namespace_child_tool(
            &refreshed_response.single_request().body_json(),
            TOOL_NAMESPACE,
            TOOL_NAME,
        )
        .is_some(),
        "the refreshed optional MCP startup grace should expose the ready tool"
    );

    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}

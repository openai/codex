//! Checks attribution when a session rebuild discovers credentials after anonymous startup.

use super::streamable_http_test_support::arm_session_post_failure;
use super::streamable_http_test_support::call_echo_tool;
use super::streamable_http_test_support::expected_echo_result;
use super::streamable_http_test_support::initialize_client;
use super::streamable_http_test_support::spawn_streamable_http_server;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::auth_storage::AuthStorageOriginator;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::save_oauth_tokens;
use oauth2::AccessToken;
use oauth2::basic::BasicTokenType;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthTokenResponse;
use std::collections::BTreeMap;

#[tokio::test]
async fn recovery_retains_originator_without_initial_credentials() -> anyhow::Result<()> {
    if std::env::var_os("CODEX_MCP_RECOVERY_TELEMETRY_TEST_CHILD").is_none() {
        let home = tempfile::tempdir()?;
        let output = tokio::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "telemetry_tests::recovery_retains_originator_without_initial_credentials",
                "--nocapture",
            ])
            .env("CODEX_MCP_RECOVERY_TELEMETRY_TEST_CHILD", "1")
            .env("CODEX_HOME", home.path())
            .output()
            .await?;
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    let metrics = codex_otel::install_global_metrics(MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
    )?);
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let url = format!("{base_url}/mcp");
    let client = AuthStorageOriginator::from_client_name("codex_vscode")
        .scope(async {
            let client = RmcpClient::new_streamable_http_client(
                "recovery-telemetry",
                &url,
                /*bearer_token*/ None,
                /*http_headers*/ None,
                /*env_http_headers*/ None,
                OAuthCredentialsStoreMode::File,
                AuthKeyringBackendKind::Direct,
                Environment::default_for_tests().get_http_client(),
                /*auth_provider*/ None,
            )
            .await?;
            initialize_client(&client).await?;
            anyhow::Ok(client)
        })
        .await?;
    assert_eq!(
        call_echo_tool(&client, "warmup").await?,
        expected_echo_result("warmup")
    );

    for recovery in 0..3 {
        if recovery == 1 {
            // Credentials arrive after startup and the first anonymous recovery.
            save_oauth_tokens(
                "recovery-telemetry",
                &StoredOAuthTokens {
                    server_name: "recovery-telemetry".into(),
                    url: url.clone(),
                    issuer: Some(url.clone()),
                    client_id: "client-id".into(),
                    token_response: WrappedOAuthTokenResponse(OAuthTokenResponse::new(
                        AccessToken::new("saved-access-token".into()),
                        BasicTokenType::Bearer,
                        Default::default(),
                    )),
                    expires_at: None,
                },
                OAuthCredentialsStoreMode::File,
                AuthKeyringBackendKind::Direct,
            )
            .await?;
        }
        arm_session_post_failure(
            &base_url,
            /*status*/ 404,
            /*remaining*/ 1,
            /*www_authenticate_headers*/ &[],
        )
        .await?;
        assert_eq!(
            call_echo_tool(&client, "recovered").await?,
            expected_echo_result("recovered")
        );
    }

    let snapshot = metrics.snapshot()?;
    let mut loads = Vec::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        if metric.name() == "codex.auth_storage.operation"
            && let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data()
        {
            for point in sum.data_points() {
                let tags: BTreeMap<_, _> = point
                    .attributes()
                    .map(|tag| (tag.key.to_string(), tag.value.to_string()))
                    .collect();
                if tags["credential_kind"] == "mcp" && tags["operation"] == "load" {
                    loads.push((
                        tags["originator"].clone(),
                        tags["storage_phase"].clone(),
                        tags["outcome"].clone(),
                        point.value(),
                    ));
                }
            }
        }
    }
    loads.sort();
    assert_eq!(
        loads,
        vec![
            ("codex_vscode".into(), "pinned".into(), "success".into(), 1),
            (
                "codex_vscode".into(),
                "policy".into(),
                "not_found".into(),
                2
            ),
            ("codex_vscode".into(), "policy".into(), "success".into(), 1),
        ]
    );
    metrics.shutdown()?;
    Ok(())
}

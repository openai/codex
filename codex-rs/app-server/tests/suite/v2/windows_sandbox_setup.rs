use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WindowsSandboxReadiness;
use codex_app_server_protocol::WindowsSandboxReadinessResponse;
use codex_app_server_protocol::WindowsSandboxSetupCompletedNotification;
use codex_app_server_protocol::WindowsSandboxSetupMode;
use codex_app_server_protocol::WindowsSandboxSetupStartParams;
use codex_app_server_protocol::WindowsSandboxSetupStartResponse;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[test_case::test_case(false, false)]
#[test_case::test_case(true, false)]
#[test_case::test_case(true, true)]
#[tokio::test]
async fn startup_mxc_preference_is_resolved_before_readiness(
    prefer_mxc: bool,
    deny_local_binding: bool,
) -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut config_toml = "[features]\nprefer_mxc = false\n".to_string();
    if deny_local_binding {
        config_toml
            .push_str("[features.network_proxy]\nenabled = true\nallow_local_binding = false\n");
    }
    let config_path = codex_home.path().join("config.toml");
    std::fs::write(&config_path, &config_toml)?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_args(&["-c", &format!("features.prefer_mxc={prefer_mxc}")])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let mxc_selected =
        prefer_mxc && !deny_local_binding && codex_sandboxing::windows_mxc_available();
    let request_id = mcp
        .send_raw_request("windowsSandbox/readiness", /*params*/ None)
        .await?;
    let readiness: WindowsSandboxReadinessResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(
        readiness,
        WindowsSandboxReadinessResponse {
            status: if mxc_selected {
                WindowsSandboxReadiness::Ready
            } else {
                WindowsSandboxReadiness::NotConfigured
            }
        }
    );
    assert_eq!(std::fs::read_to_string(config_path)?, config_toml);
    Ok(())
}

#[tokio::test]
async fn windows_sandbox_setup_start_emits_completion_notification() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 500_000,
        Some(false),
        "mock_provider",
        "compact prompt",
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_windows_sandbox_setup_start_request(WindowsSandboxSetupStartParams {
            mode: WindowsSandboxSetupMode::Unelevated,
            cwd: None,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let start_payload: WindowsSandboxSetupStartResponse = to_response(response)?;
    assert!(start_payload.started);

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("windowsSandbox/setupCompleted"),
    )
    .await??;
    let payload: WindowsSandboxSetupCompletedNotification = serde_json::from_value(
        notification
            .params
            .context("missing windowsSandbox/setupCompleted params")?,
    )?;

    assert_eq!(payload.mode, WindowsSandboxSetupMode::Unelevated);
    Ok(())
}

#[cfg(target_os = "windows")]
#[test_case::test_case(
    "sandbox_mode = 'danger-full-access'",
    true,
    "app runtime provisioning service is unavailable; refusing helper fallback";
    "full access reaches the service"
)]
#[test_case::test_case(
    "sandbox_mode = 'workspace-write'",
    true,
    "app runtime provisioning service is unavailable; refusing helper fallback";
    "supported restricted permissions reach the service"
)]
#[test_case::test_case(
    "default_permissions = 'workspace'\n[permissions.workspace.filesystem]\n':workspace_roots' = 'write'",
    true,
    "elevated Windows sandbox requires effective `:root` read access";
    "unsupported restricted permissions fail before the service"
)]
#[test_case::test_case(
    "sandbox_mode = 'danger-full-access'\n[features]\nwindows_sandbox_service = true",
    false,
    "only managed permission profiles can be enforced by the Windows sandbox";
    "legacy full access skips the service and reaches shared setup"
)]
#[tokio::test]
async fn setup_validates_permissions_before_provisioning(
    config: &str,
    registered_core: bool,
    expected_error: &str,
) -> Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join("config.toml");
    std::fs::write(&config_path, config)?;
    // Registered tests use a nonexistent service; an invalid hint detects any legacy service call.
    let service_family = if registered_core {
        format!(
            "CodexSetupTest{}_0000000000000",
            uuid::Uuid::now_v7().simple()
        )
    } else {
        "invalid-service-family".into()
    };
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .without_managed_config()
        .with_env_overrides(&[
            (
                "CODEX_WINDOWS_REGISTERED_CORE",
                Some(if registered_core { "1" } else { "0" }),
            ),
            (
                "CODEX_WINDOWS_SANDBOX_PACKAGE_FAMILY",
                Some(&service_family),
            ),
            (
                codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR,
                None,
            ),
            (
                codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
                None,
            ),
        ])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_windows_sandbox_setup_start_request(WindowsSandboxSetupStartParams {
            mode: WindowsSandboxSetupMode::Elevated,
            cwd: None,
        })
        .await?;
    let response: WindowsSandboxSetupStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response, WindowsSandboxSetupStartResponse { started: true });

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("windowsSandbox/setupCompleted"),
    )
    .await??;
    let completion: WindowsSandboxSetupCompletedNotification = serde_json::from_value(
        notification
            .params
            .context("missing setup completion params")?,
    )?;
    assert_eq!(
        completion,
        WindowsSandboxSetupCompletedNotification {
            mode: WindowsSandboxSetupMode::Elevated,
            success: false,
            error: Some(expected_error.into()),
        }
    );
    assert_eq!(std::fs::read_to_string(config_path)?, config);
    Ok(())
}

#[tokio::test]
async fn windows_sandbox_setup_start_rejects_relative_cwd() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "windowsSandbox/setupStart",
            Some(serde_json::json!({
                "mode": "unelevated",
                "cwd": "relative-root",
            })),
        )
        .await?;

    let err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("Invalid request"));
    Ok(())
}

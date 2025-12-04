use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigLayerName;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WriteStatus;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn write_config(codex_home: &TempDir, contents: &str) -> Result<()> {
    Ok(std::fs::write(
        codex_home.path().join("config.toml"),
        contents,
    )?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_read_returns_effective_and_layers() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_config(
        &codex_home,
        r#"
model = "gpt-user"
sandbox_mode = "workspace-write"
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config,
        origins,
        layers,
    } = to_response(resp)?;

    assert_eq!(config.get("model"), Some(&json!("gpt-user")));
    assert_eq!(
        origins.get("model").expect("origin").name,
        ConfigLayerName::User
    );
    let layers = layers.expect("layers present");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].name, ConfigLayerName::SessionFlags);
    assert_eq!(layers[1].name, ConfigLayerName::User);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_read_includes_system_layer_and_overrides() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_config(
        &codex_home,
        r#"
model = "gpt-user"
approval_policy = "on-request"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = ["/user"]
network_access = true
"#,
    )?;

    let managed_path = codex_home.path().join("managed_config.toml");
    std::fs::write(
        &managed_path,
        r#"
model = "gpt-system"
approval_policy = "never"

[sandbox_workspace_write]
writable_roots = ["/system"]
"#,
    )?;

    let managed_path_str = managed_path.display().to_string();

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[("CODEX_MANAGED_CONFIG_PATH", Some(&managed_path_str))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config,
        origins,
        layers,
    } = to_response(resp)?;

    assert_eq!(config.get("model"), Some(&json!("gpt-system")));
    assert_eq!(
        origins.get("model").expect("origin").name,
        ConfigLayerName::System
    );

    assert_eq!(config.get("approval_policy"), Some(&json!("never")));
    assert_eq!(
        origins.get("approval_policy").expect("origin").name,
        ConfigLayerName::System
    );

    assert_eq!(config.get("sandbox_mode"), Some(&json!("workspace-write")));
    assert_eq!(
        origins.get("sandbox_mode").expect("origin").name,
        ConfigLayerName::User
    );

    assert_eq!(
        config
            .get("sandbox_workspace_write")
            .and_then(|v| v.get("writable_roots")),
        Some(&json!(["/system"]))
    );
    assert_eq!(
        origins
            .get("sandbox_workspace_write.writable_roots.0")
            .expect("origin")
            .name,
        ConfigLayerName::System
    );

    assert_eq!(
        config
            .get("sandbox_workspace_write")
            .and_then(|v| v.get("network_access")),
        Some(&json!(true))
    );
    assert_eq!(
        origins
            .get("sandbox_workspace_write.network_access")
            .expect("origin")
            .name,
        ConfigLayerName::User
    );

    let layers = layers.expect("layers present");
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].name, ConfigLayerName::System);
    assert_eq!(layers[1].name, ConfigLayerName::SessionFlags);
    assert_eq!(layers[2].name, ConfigLayerName::User);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_value_write_replaces_value() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_config(
        &codex_home,
        r#"
model = "gpt-old"
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let read_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let read: ConfigReadResponse = to_response(read_resp)?;
    let expected_version = read.origins.get("model").map(|m| m.version.clone());

    let write_id = mcp
        .send_config_value_write_request(ConfigValueWriteParams {
            file_path: None,
            key_path: "model".to_string(),
            value: json!("gpt-new"),
            merge_strategy: MergeStrategy::Replace,
            expected_version,
        })
        .await?;
    let write_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_id)),
    )
    .await??;
    let write: ConfigWriteResponse = to_response(write_resp)?;
    let expected_file_path = codex_home
        .path()
        .join("config.toml")
        .canonicalize()
        .unwrap()
        .display()
        .to_string();

    assert_eq!(write.status, WriteStatus::Ok);
    assert_eq!(write.file_path, expected_file_path);
    assert!(write.overridden_metadata.is_none());

    let verify_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
        })
        .await?;
    let verify_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(verify_id)),
    )
    .await??;
    let verify: ConfigReadResponse = to_response(verify_resp)?;
    assert_eq!(verify.config.get("model"), Some(&json!("gpt-new")));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_value_write_rejects_version_conflict() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_config(
        &codex_home,
        r#"
model = "gpt-old"
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let write_id = mcp
        .send_config_value_write_request(ConfigValueWriteParams {
            file_path: Some(codex_home.path().join("config.toml").display().to_string()),
            key_path: "model".to_string(),
            value: json!("gpt-new"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: Some("sha256:stale".to_string()),
        })
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(write_id)),
    )
    .await??;
    let code = err
        .error
        .data
        .as_ref()
        .and_then(|d| d.get("config_write_error_code"))
        .and_then(|v| v.as_str());
    assert_eq!(code, Some("configVersionConflict"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_batch_write_applies_multiple_edits() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_config(&codex_home, "")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let batch_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            file_path: Some(codex_home.path().join("config.toml").display().to_string()),
            edits: vec![
                ConfigEdit {
                    key_path: "sandbox_mode".to_string(),
                    value: json!("workspace-write"),
                    merge_strategy: MergeStrategy::Replace,
                },
                ConfigEdit {
                    key_path: "sandbox_workspace_write".to_string(),
                    value: json!({
                        "writable_roots": ["/tmp"],
                        "network_access": false
                    }),
                    merge_strategy: MergeStrategy::Replace,
                },
            ],
            expected_version: None,
        })
        .await?;
    let batch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(batch_id)),
    )
    .await??;
    let batch_write: ConfigWriteResponse = to_response(batch_resp)?;
    assert_eq!(batch_write.status, WriteStatus::Ok);
    let expected_file_path = codex_home
        .path()
        .join("config.toml")
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    assert_eq!(batch_write.file_path, expected_file_path);

    let read_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let read: ConfigReadResponse = to_response(read_resp)?;
    assert_eq!(
        read.config.get("sandbox_mode"),
        Some(&json!("workspace-write"))
    );
    assert_eq!(
        read.config
            .get("sandbox_workspace_write")
            .and_then(|v| v.get("writable_roots")),
        Some(&json!(["/tmp"]))
    );
    assert_eq!(
        read.config
            .get("sandbox_workspace_write")
            .and_then(|v| v.get("network_access")),
        Some(&json!(false))
    );

    Ok(())
}

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::test_path_buf_with_windows;
use app_test_support::test_tmp_path_buf;
use app_test_support::to_response;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::ToolsV2;
use codex_app_server_protocol::WriteStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
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
    let codex_home_path = codex_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(codex_home_path.join("config.toml"))?;

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

    assert_eq!(config.model.as_deref(), Some("gpt-user"));
    assert_eq!(
        origins.get("model").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    let layers = layers.expect("layers present");
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, ConfigLayerSource::User { file: user_file });

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_read_includes_tools() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_config(
        &codex_home,
        r#"
model = "gpt-user"

[tools]
web_search = true
view_image = false
"#,
    )?;
    let codex_home_path = codex_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(codex_home_path.join("config.toml"))?;

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

    let tools = config.tools.expect("tools present");
    assert_eq!(
        tools,
        ToolsV2 {
            web_search: Some(true),
            view_image: Some(false),
        }
    );
    assert_eq!(
        origins.get("tools.web_search").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    assert_eq!(
        origins.get("tools.view_image").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let layers = layers.expect("layers present");
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, ConfigLayerSource::User { file: user_file });

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_read_includes_system_layer_and_overrides() -> Result<()> {
    let codex_home = TempDir::new()?;
    let user_dir = test_path_buf_with_windows("/user", Some(r"C:\Users\user"));
    let system_dir = test_path_buf_with_windows("/system", Some(r"C:\System"));
    write_config(
        &codex_home,
        &format!(
            r#"
model = "gpt-user"
approval_policy = "on-request"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [{}]
network_access = true
"#,
            serde_json::json!(user_dir)
        ),
    )?;
    let codex_home_path = codex_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(codex_home_path.join("config.toml"))?;

    let managed_path = codex_home.path().join("managed_config.toml");
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone())?;
    std::fs::write(
        &managed_path,
        format!(
            r#"
model = "gpt-system"
approval_policy = "never"

[sandbox_workspace_write]
writable_roots = [{}]
"#,
            serde_json::json!(system_dir.clone())
        ),
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

    assert_eq!(config.model.as_deref(), Some("gpt-system"));
    assert_eq!(
        origins.get("model").expect("origin").name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone(),
        }
    );

    assert_eq!(config.approval_policy, Some(AskForApproval::Never));
    assert_eq!(
        origins.get("approval_policy").expect("origin").name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone(),
        }
    );

    assert_eq!(config.sandbox_mode, Some(SandboxMode::WorkspaceWrite));
    assert_eq!(
        origins.get("sandbox_mode").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let sandbox = config
        .sandbox_workspace_write
        .as_ref()
        .expect("sandbox workspace write");
    assert_eq!(sandbox.writable_roots, vec![system_dir]);
    assert_eq!(
        origins
            .get("sandbox_workspace_write.writable_roots.0")
            .expect("origin")
            .name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone(),
        }
    );

    assert!(sandbox.network_access);
    assert_eq!(
        origins
            .get("sandbox_workspace_write.network_access")
            .expect("origin")
            .name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let layers = layers.expect("layers present");
    assert_eq!(layers.len(), 2);
    assert_eq!(
        layers[0].name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file }
    );
    assert_eq!(layers[1].name, ConfigLayerSource::User { file: user_file });

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_value_write_replaces_value() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let codex_home = temp_dir.path().canonicalize()?;
    write_config(
        &temp_dir,
        r#"
model = "gpt-old"
"#,
    )?;

    let mut mcp = McpProcess::new(&codex_home).await?;
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
    let expected_file_path = AbsolutePathBuf::resolve_path_against_base("config.toml", codex_home)?;

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
    assert_eq!(verify.config.model.as_deref(), Some("gpt-new"));

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
    let tmp_dir = TempDir::new()?;
    let codex_home = tmp_dir.path().canonicalize()?;
    write_config(&tmp_dir, "")?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let writable_root = test_tmp_path_buf();
    let batch_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            file_path: Some(codex_home.join("config.toml").display().to_string()),
            edits: vec![
                ConfigEdit {
                    key_path: "sandbox_mode".to_string(),
                    value: json!("workspace-write"),
                    merge_strategy: MergeStrategy::Replace,
                },
                ConfigEdit {
                    key_path: "sandbox_workspace_write".to_string(),
                    value: json!({
                        "writable_roots": [writable_root.clone()],
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
    let expected_file_path = AbsolutePathBuf::resolve_path_against_base("config.toml", codex_home)?;
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
    assert_eq!(read.config.sandbox_mode, Some(SandboxMode::WorkspaceWrite));
    let sandbox = read
        .config
        .sandbox_workspace_write
        .as_ref()
        .expect("sandbox workspace write");
    assert_eq!(sandbox.writable_roots, vec![writable_root]);
    assert!(!sandbox.network_access);

    Ok(())
}

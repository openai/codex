mod common;

use codex_exec_server::CAPABILITY_ROOTS_DISCOVER_METHOD;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::CapabilityRootsDiscoverParams;
use codex_exec_server::CapabilityRootsDiscoverResponse;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server_protocol::CapabilityRootDiscoverRequest;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_a_complete_capability_bundle_in_one_request() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    write_file(
        &root.path().join(".codex-plugin/plugin.json"),
        r#"{
  "name": "demo",
  "interface": {"displayName": "Demo Plugin"},
  "mcpServers": "./config/mcp.json",
  "apps": "./config/apps.json"
}"#,
    )?;
    write_file(
        &root.path().join(".claude-plugin/plugin.json"),
        r#"{"name":"lower-priority-claude"}"#,
    )?;
    write_file(
        &root.path().join(".cursor-plugin/plugin.json"),
        r#"{"name":"lower-priority-cursor"}"#,
    )?;
    write_file(
        &root.path().join("config/mcp.json"),
        r#"{"mcpServers":{"demo":{"command":"demo-server"}}}"#,
    )?;
    write_file(
        &root.path().join("config/apps.json"),
        r#"{"apps":{"demo":{"connector_id":"connector-demo"}}}"#,
    )?;
    write_file(
        &root.path().join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploy the service.\n---\n\nDeploy instructions.\n",
    )?;
    write_file(
        &root.path().join("skills/deploy/agents/openai.yaml"),
        "policy:\n  allow_implicit_invocation: false\n",
    )?;
    write_file(
        &root.path().join("nested/.claude-plugin/plugin.json"),
        r#"{"name":"nested"}"#,
    )?;
    write_file(
        &root.path().join("nested/skills/audit/SKILL.md"),
        "---\nname: audit\ndescription: Audit the service.\n---\n",
    )?;
    write_file(
        &root.path().join("nested-cursor/.cursor-plugin/plugin.json"),
        r#"{"name":"cursor-nested"}"#,
    )?;
    write_file(
        &root.path().join("nested-cursor/skills/review/SKILL.md"),
        "---\nname: review\ndescription: Review the service.\n---\n",
    )?;

    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let root_uri = PathUri::from_host_native_path(root.path())?;
    let discovery = discover_root(&mut server, "demo@1", root_uri.clone()).await?;

    assert_eq!(discovery.id, "demo@1");
    assert_eq!(discovery.path, root_uri);
    assert_eq!(discovery.error, None);
    assert_eq!(discovery.warnings, Vec::<String>::new());
    let plugin = discovery.plugin.as_ref().expect("root plugin");
    assert_eq!(
        plugin.manifest.path,
        root_uri.join(".codex-plugin/plugin.json")?
    );
    assert!(plugin.manifest.contents.contains("Demo Plugin"));
    assert_eq!(
        plugin.mcp_config.as_ref().map(|file| &file.path),
        Some(&root_uri.join("config/mcp.json")?)
    );
    assert_eq!(
        plugin.apps_config.as_ref().map(|file| &file.path),
        Some(&root_uri.join("config/apps.json")?)
    );
    assert_eq!(
        discovery
            .namespace_manifests
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>(),
        vec![
            root_uri.join(".codex-plugin/plugin.json")?,
            root_uri.join("nested/.claude-plugin/plugin.json")?,
            root_uri.join("nested-cursor/.cursor-plugin/plugin.json")?,
        ]
    );
    assert_eq!(
        discovery
            .skills
            .iter()
            .map(|skill| (
                skill.instructions.path.clone(),
                skill
                    .metadata
                    .as_ref()
                    .map(|metadata| metadata.path.clone()),
            ))
            .collect::<Vec<_>>(),
        vec![
            (root_uri.join("nested-cursor/skills/review/SKILL.md")?, None,),
            (root_uri.join("nested/skills/audit/SKILL.md")?, None,),
            (
                root_uri.join("skills/deploy/SKILL.md")?,
                Some(root_uri.join("skills/deploy/agents/openai.yaml")?),
            ),
        ]
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_cursor_plugin_without_reading_default_mcp_for_inline_servers()
-> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    write_file(
        &root.path().join(".cursor-plugin/plugin.json"),
        r#"{"name":"cursor-demo","mcpServers":{"inline":{"command":"inline"}}}"#,
    )?;
    write_file(
        &root.path().join(".mcp.json"),
        r#"{"mcpServers":{"should-not-load":{"command":"wrong"}}}"#,
    )?;

    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let root_uri = PathUri::from_host_native_path(root.path())?;
    let discovery = discover_root(&mut server, "cursor@1", root_uri.clone()).await?;

    assert_eq!(discovery.error, None);
    assert_eq!(discovery.warnings, Vec::<String>::new());
    let plugin = discovery.plugin.expect("cursor plugin");
    assert_eq!(
        plugin.manifest.path,
        root_uri.join(".cursor-plugin/plugin.json")?
    );
    assert_eq!(plugin.mcp_config, None);
    assert_eq!(
        discovery
            .namespace_manifests
            .iter()
            .map(|manifest| manifest.path.clone())
            .collect::<Vec<_>>(),
        vec![root_uri.join(".cursor-plugin/plugin.json")?]
    );

    server.shutdown().await?;
    Ok(())
}

async fn discover_root(
    server: &mut common::exec_server::ExecServerHarness,
    id: &str,
    path: PathUri,
) -> anyhow::Result<CapabilityRootDiscovery> {
    let request_id = server
        .send_request(
            CAPABILITY_ROOTS_DISCOVER_METHOD,
            serde_json::to_value(CapabilityRootsDiscoverParams {
                roots: vec![CapabilityRootDiscoverRequest {
                    id: id.to_string(),
                    path,
                }],
            })?,
        )
        .await?;
    let response = server.next_event().await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        anyhow::bail!("expected discovery response, received {response:?}");
    };
    assert_eq!(id, request_id);
    let response: CapabilityRootsDiscoverResponse = serde_json::from_value(result)?;
    let [discovery] = response.roots.as_slice() else {
        anyhow::bail!("expected exactly one discovered root");
    };
    Ok(discovery.clone())
}

async fn initialize(server: &mut common::exec_server::ExecServerHarness) -> anyhow::Result<()> {
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "capability-discovery-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(event, JSONRPCMessage::Response(response) if response.id == initialize_id)
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        unreachable!("wait predicate only accepts a response");
    };
    let _: InitializeResponse = serde_json::from_value(result)?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    Ok(())
}

fn write_file(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("test file should have a parent"))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, contents)?;
    Ok(())
}

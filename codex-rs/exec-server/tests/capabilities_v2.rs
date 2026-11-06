//! V2 RPC metadata refresh and sandbox isolation over a real connection.
mod common;

use anyhow::Context;
use codex_exec_server::CAPABILITIES_DISCOVER_V2_METHOD;
use codex_exec_server::DiscoverV2CapabilitiesRequest;
use codex_exec_server::DiscoverV2CapabilitiesResponse;
use codex_exec_server::InitializeResponse;
use codex_exec_server::SkillDiscoveryRoot;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_utils_path_uri::PathUri;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_rpc_refreshes_metadata_and_enforces_sandbox() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let home_path = std::fs::canonicalize(home.path())?;
    let codex_home = tempfile::tempdir()?;
    let codex_home_path = std::fs::canonicalize(codex_home.path())?;
    let skills = codex_home_path.join("skills/demo/SKILL.md");
    let plugin = codex_home_path.join("plugins/cache/market/mail/1.0.0");
    write(
        &plugin.join(".codex-plugin/plugin.json"),
        r#"{"name":"mail"}"#,
    )?;
    write(
        &plugin.join("skills/search/SKILL.md"),
        "---\nname: search\ndescription: Search mail\n---\n",
    )?;
    write(
        &skills,
        "---\nname: demo\ndescription: original\n---\nPRIVATE INSTRUCTION BODY",
    )?;
    let mut server = exec_server_with_env(
        [
            ("HOME", &home_path),
            ("USERPROFILE", &home_path),
            ("CODEX_HOME", &codex_home_path),
        ],
        &[],
    )
    .await?;
    // Existing clients need no discovery-specific initialize field.
    let initialized: InitializeResponse = serde_json::from_value(
        request(
            &mut server,
            "initialize",
            serde_json::json!({"clientName": "v2-client"}),
        )
        .await?,
    )?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    let info = initialized
        .environment_info
        .context("initialize environment info")?;
    assert!(info.capabilities.capability_discovery_v2);
    // Keep the requested cwd isolated from the test runner's repository.
    let project = tempfile::tempdir()?;
    let cwd = PathUri::from_host_native_path(project.path())?;
    let params = DiscoverV2CapabilitiesRequest {
        cwd: cwd.clone(),
        sandbox: None,
    };
    let first = discover(&mut server, &params).await?;
    assert!(
        first
            .skills
            .iter()
            .any(|skill| skill.name == "demo" && skill.description == "original")
    );
    let expected_root = Some(SkillDiscoveryRoot {
        id: "codex-home-skills".to_string(),
        path: PathUri::from_host_native_path(codex_home_path.join("skills"))?,
    });
    assert_eq!(
        first
            .skills
            .iter()
            .find(|skill| skill.name == "demo")
            .context("demo skill")?
            .discovery_root,
        expected_root,
    );
    assert!(!serde_json::to_string(&first)?.contains("PRIVATE INSTRUCTION BODY"));

    assert!(
        first
            .plugins
            .iter()
            .any(|plugin| plugin.id == "mail@market" && plugin.skills.len() == 1)
    );

    // Exercise one content refresh through the real RPC and native watcher.
    write(&skills, "---\nname: demo\ndescription: updated\n---\n")?;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
        loop {
            interval.tick().await;
            if discover(&mut server, &params)
                .await?
                .skills
                .iter()
                .any(|skill| skill.name == "demo" && skill.description == "updated")
            {
                break anyhow::Ok(());
            }
        }
    })
    .await
    .context("waiting for discovery refresh")??;

    // A restricted request must not reuse the unrestricted context's cached plugin metadata.
    {
        use codex_exec_server::FileSystemSandboxContext;
        use codex_protocol::models::PermissionProfile;
        use codex_protocol::permissions::FileSystemAccessMode;
        use codex_protocol::permissions::FileSystemSandboxEntry;
        use codex_protocol::permissions::FileSystemSandboxPolicy;
        use codex_protocol::permissions::NetworkSandboxPolicy;
        use codex_utils_absolute_path::AbsolutePathBuf;
        let allowed = codex_home_path.join("skills");
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            AbsolutePathBuf::from_absolute_path(&allowed)?.into(),
            FileSystemAccessMode::Read,
        )]);
        let sandbox = FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
            cwd.clone(),
        );
        #[cfg(windows)]
        let sandbox = FileSystemSandboxContext {
            windows_sandbox_selection: codex_exec_server::WindowsSandboxSelection::RestrictedToken,
            ..sandbox
        };
        let restricted = DiscoverV2CapabilitiesRequest {
            cwd: cwd.clone(),
            sandbox: Some(sandbox),
        };
        // Linux (Bubblewrap) and macOS (Seatbelt) enforce the allowed directory directly.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let response = discover(&mut server, &restricted).await?;
            assert!(response.plugins.is_empty());
            assert_eq!(
                response
                    .skills
                    .iter()
                    .find(|skill| skill.name == "demo")
                    .context("sandboxed demo skill")?
                    .discovery_root,
                expected_root,
            );
            assert!(!serde_json::to_string(&response)?.contains("mail@market"));
        }
        // Windows restricted tokens reject split read access; a supported read-only policy succeeds.
        #[cfg(windows)]
        {
            let Err(error) = discover(&mut server, &restricted).await else {
                anyhow::bail!("unsupported read restrictions must not reuse cached metadata");
            };
            assert!(
                error.chain().any(|cause| cause
                    .to_string()
                    .contains("cannot enforce split filesystem read restrictions")),
                "{error:#}"
            );
            let mut sandbox = FileSystemSandboxContext::from_permission_profile(
                PermissionProfile::read_only(),
                cwd.clone(),
            );
            sandbox.windows_sandbox_selection =
                codex_exec_server::WindowsSandboxSelection::RestrictedToken;
            let permitted = DiscoverV2CapabilitiesRequest {
                cwd: cwd.clone(),
                sandbox: Some(sandbox),
            };
            let response = discover(&mut server, &permitted).await?;
            assert!(
                response
                    .plugins
                    .iter()
                    .any(|plugin| plugin.id == "mail@market")
            );
            assert_eq!(
                response
                    .skills
                    .iter()
                    .find(|skill| skill.name == "demo")
                    .context("sandboxed demo skill")?
                    .discovery_root,
                expected_root,
            );
        }
    }

    server.shutdown().await?;
    Ok(())
}

async fn discover(
    server: &mut ExecServerHarness,
    params: &DiscoverV2CapabilitiesRequest,
) -> anyhow::Result<DiscoverV2CapabilitiesResponse> {
    Ok(serde_json::from_value(
        request(
            server,
            CAPABILITIES_DISCOVER_V2_METHOD,
            serde_json::to_value(params)?,
        )
        .await
        .with_context(|| format!("discover V2 for {}", params.cwd))?,
    )?)
}

async fn request(
    server: &mut ExecServerHarness,
    method: &str,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let id = server.send_request(method, params).await?;
    let message = server
        .wait_for_event(|event| {
            matches!(event,
                JSONRPCMessage::Response(response) if response.id == id
            ) || matches!(event, JSONRPCMessage::Error(error) if error.id == id)
        })
        .await
        .with_context(|| format!("waiting for {method}"))?;
    match message {
        JSONRPCMessage::Response(response) => Ok(response.result),
        other => anyhow::bail!("{method} failed: {other:?}"),
    }
}

fn write(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

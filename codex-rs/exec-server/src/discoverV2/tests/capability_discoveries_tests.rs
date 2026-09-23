//! Covers metadata parity and inventory discovery independent of plugin enablement.

use pretty_assertions::assert_eq;

use super::MAX_CAPABILITIES;
use super::MAX_RESPONSE_BYTES;
use super::MAX_WARNINGS;
use crate::CapabilityRootDiscoverRequest;
use crate::DiscoverV2CapabilitiesRequest;
use crate::ExecutorSkill;
use crate::LocalFileSystem;
use crate::capability_discovery::discover_root;
use crate::discover_v2::capability_locations::tests::MANIFEST;
use crate::discover_v2::capability_locations::tests::request;
use crate::discover_v2::capability_locations::tests::write;
use crate::discover_v2::capability_manager::CapabilityManager;
use codex_utils_path_uri::PathUri;

const SKILL: &str =
    "---\nname: deploy\ndescription: Deploy the service.\n---\nPRIVATE_INSTRUCTION_BODY_SENTINEL\n";

#[tokio::test]
async fn discovery_preserves_inventory_and_v1_metadata() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path())?;
    write(
        temp.path(),
        "codex/config.toml",
        "[plugins.\"demo@test\"]\n[plugins.\"disabled@test\"]\nenabled = false\n",
    )?;
    // Inventory includes disabled and unconfigured plugins; Core applies enablement.
    for name in ["disabled", "unconfigured"] {
        write(
            temp.path(),
            &format!("codex/plugins/cache/test/{name}/local/.codex-plugin/plugin.json"),
            MANIFEST,
        )?;
    }
    let root = "codex/plugins/cache/test/demo/local";
    write(
        temp.path(),
        &format!("{root}/.codex-plugin/plugin.json"),
        r#"{"name":"demo","skills":"./skills"}"#,
    )?;
    let mcp_path = ".mcp.json";
    write(
        temp.path(),
        &format!("{root}/{mcp_path}"),
        r#"{"mcpServers":{}}"#,
    )?;
    for relative in [
        "skills/deploy/SKILL.md",
        "examples/deploy/SKILL.md",
        ".codex-plugin/migrated-command-skills/deploy/SKILL.md",
        "skills/a/b/c/d/e/f/deploy/SKILL.md",
    ] {
        write(temp.path(), &format!("{root}/{relative}"), SKILL)?;
    }
    write(
        temp.path(),
        "codex/skills/deploy  skill/SKILL.md",
        &SKILL.replace("name: deploy\n", ""),
    )?;
    for nested in [format!("{root}/nested"), "codex/skills/nested".to_string()] {
        write(
            temp.path(),
            &format!("{nested}/.codex-plugin/plugin.json"),
            r#"{"name":"team"}"#,
        )?;
        write(
            temp.path(),
            &format!("{nested}/skills/deploy/SKILL.md"),
            SKILL,
        )?;
    }
    let file_system = LocalFileSystem::unsandboxed();
    let plugin_root = request.codex_home.join("plugins/cache/test/demo/local")?;
    let v1 = discover_root(
        &file_system,
        CapabilityRootDiscoverRequest {
            id: "demo@test".to_string(),
            path: plugin_root.clone(),
            sandbox: None,
        },
    )
    .await;
    // V1 keeps undeclared/migrated skills and excludes the skill beyond its depth limit.
    assert_eq!(v1.skills.len(), 4);
    let manager = CapabilityManager::new(file_system);
    manager.prewarm_locations(request.clone()).await?;
    let response = manager
        .get_or_refresh_discovery(
            request.clone(),
            DiscoverV2CapabilitiesRequest {
                cwd: PathUri::from_host_native_path(temp.path().join("project"))?,
                sandbox: None,
            },
        )
        .await?;
    assert_eq!(
        response
            .plugins
            .iter()
            .map(|plugin| plugin.id.as_str())
            .collect::<Vec<_>>(),
        vec!["demo@test", "disabled@test", "unconfigured@test"],
    );
    let plugin = response
        .plugins
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing demo plugin"))?;
    assert_eq!(
        plugin.files.manifest.path,
        plugin_root.join(".codex-plugin/plugin.json")?,
    );
    assert_eq!(
        plugin.files.mcp_config.as_ref().map(|file| &file.path),
        Some(&plugin_root.join(mcp_path)?),
    );
    assert_eq!(plugin.root, plugin_root);
    assert_eq!(plugin.version, "local");
    assert_eq!(Some(&plugin.files), v1.plugin.as_ref());
    assert_eq!(
        plugin.skills,
        v1.skills
            .into_iter()
            .map(|skill| {
                let namespace = if skill
                    .instructions
                    .path
                    .starts_with(&plugin.root.join("nested")?)
                {
                    "team"
                } else {
                    "demo"
                };
                Ok(expected_skill(skill.instructions.path, Some(namespace)))
            })
            .collect::<anyhow::Result<Vec<_>>>()?
    );
    assert_eq!(
        response.skills,
        vec![
            ExecutorSkill {
                name: "deploy skill".to_string(),
                ..expected_skill(
                    request.codex_home.join("skills/deploy  skill/SKILL.md")?,
                    /*namespace*/ None,
                )
            },
            expected_skill(
                request
                    .codex_home
                    .join("skills/nested/skills/deploy/SKILL.md")?,
                Some("team")
            ),
        ],
    );
    assert_eq!(response.warnings, v1.warnings);
    let json = serde_json::to_string(&response)?;
    assert!(
        response.plugins.len() + plugin.skills.len() + response.skills.len() <= MAX_CAPABILITIES
    );
    assert!(json.len() <= MAX_RESPONSE_BYTES);
    assert!(response.warnings.len() <= MAX_WARNINGS);
    assert!(!json.contains("PRIVATE_INSTRUCTION_BODY_SENTINEL"));
    Ok(())
}

fn expected_skill(path: PathUri, namespace: Option<&str>) -> ExecutorSkill {
    ExecutorSkill {
        name: "deploy".to_string(),
        namespace: namespace.map(str::to_string),
        description: "Deploy the service.".to_string(),
        short_description: None,
        path,
        metadata: None,
    }
}

#[tokio::test]
async fn discovery_reports_location_failures_without_paths() -> anyhow::Result<()> {
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;

    let temp = tempfile::tempdir()?;
    request(temp.path())?;
    let locations = crate::discover_v2::capability_locations::CapabilityLocations {
        warnings: vec!["could not scan plugin cache /private/plugins/cache".into()],
        ..Default::default()
    };
    let sandbox = crate::FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(Vec::new()),
            NetworkSandboxPolicy::Restricted,
        ),
        PathUri::from_host_native_path(temp.path().join("project"))?,
    );
    for sandbox in [None, Some(&sandbox)] {
        let response = super::load_capability_discoveries(
            &LocalFileSystem::unsandboxed(),
            &locations,
            sandbox,
        )
        .await?;
        assert_eq!(
            *response,
            crate::DiscoverV2CapabilitiesResponse {
                warnings: vec![
                    "some capability locations could not be resolved; discovery may be incomplete"
                        .into(),
                ],
                ..Default::default()
            }
        );
    }
    Ok(())
}

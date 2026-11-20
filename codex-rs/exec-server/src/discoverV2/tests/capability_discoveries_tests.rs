//! Covers metadata parity and inventory discovery independent of plugin enablement.

use pretty_assertions::assert_eq;

use super::MAX_CAPABILITIES;
use super::MAX_RESPONSE_BYTES;
use super::MAX_WARNINGS;
use crate::CapabilityRootDiscoverRequest;
use crate::DiscoverV2CapabilitiesRequest;
use crate::ExecutorSkill;
use crate::LocalFileSystem;
use crate::SkillDiscoveryRoot;
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
    let inline_root = "codex/plugins/cache/test/unconfigured/local";
    write(
        temp.path(),
        "codex/plugins/cache/test/disabled/local/.codex-plugin/plugin.json",
        r#"{"name":" ","interface":{"displayName":" "},"mcpServers":"./missing-mcp.json","apps":"./missing-apps.json"}"#,
    )?;
    let inline_manifest = r#"{"name":"inline","interface":{"displayName":" Inline Plugin "},"mcpServers":{"inline":{"command":"node"}}}"#;
    write(
        temp.path(),
        &format!("{inline_root}/.codex-plugin/plugin.json"),
        inline_manifest,
    )?;
    // Inline declarations take precedence over a conventional MCP file.
    write(
        temp.path(),
        &format!("{inline_root}/.mcp.json"),
        r#"{"wrong":{"command":"wrong"}}"#,
    )?;
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
    let inline_v1 = discover_root(
        &file_system,
        CapabilityRootDiscoverRequest {
            id: "unconfigured@test".to_string(),
            path: PathUri::from_host_native_path(temp.path().join(inline_root))?,
            sandbox: None,
        },
    )
    .await;
    let inline_v1_files = inline_v1.plugin.expect("inline plugin");
    assert_eq!(inline_v1_files.mcp_config, None);
    let manager = CapabilityManager::new(file_system);
    manager.seed_prewarmed_locations(request.clone()).await?;
    let response = manager
        .get_or_refresh_discovery(DiscoverV2CapabilitiesRequest {
            force_reload: false,
            cwd: PathUri::from_host_native_path(temp.path().join("project"))?,
            sandbox: None,
        })
        .await?;
    assert_eq!(
        response
            .plugins
            .iter()
            .map(|plugin| plugin.id.as_str())
            .collect::<Vec<_>>(),
        vec!["demo@test", "disabled@test", "unconfigured@test"],
    );
    assert_eq!(
        response
            .plugins
            .iter()
            .map(|plugin| plugin.display_name.as_str())
            .collect::<Vec<_>>(),
        vec!["demo", "local", "Inline Plugin"],
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
                let name = if skill
                    .instructions
                    .path
                    .starts_with(&plugin.root.join("nested")?)
                {
                    "team:deploy"
                } else {
                    "demo:deploy"
                };
                Ok(expected_skill(skill.instructions.path, name))
            })
            .collect::<anyhow::Result<Vec<_>>>()?
    );
    assert_eq!(
        response.skills,
        vec![
            expected_skill(
                request.codex_home.join("skills/deploy  skill/SKILL.md")?,
                "deploy skill",
            ),
            expected_skill(
                request
                    .codex_home
                    .join("skills/nested/skills/deploy/SKILL.md")?,
                "team:deploy"
            ),
        ]
        .into_iter()
        .map(|mut skill| {
            skill.discovery_root = Some(SkillDiscoveryRoot {
                id: "codex-home-skills".to_string(),
                path: request.codex_home.join("skills").expect("skills root"),
            });
            skill
        })
        .collect::<Vec<_>>(),
    );
    let inline_plugin = response
        .plugins
        .iter()
        .find(|plugin| plugin.id == "unconfigured@test")
        .expect("inline plugin");
    assert_eq!(inline_plugin.files.manifest, inline_v1_files.manifest);
    assert_eq!(
        inline_plugin.files.mcp_config,
        Some(crate::CapabilityTextFile {
            path: inline_v1_files.manifest.path,
            contents: serde_json::json!({"inline": {"command": "node"}}).to_string(),
        })
    );
    assert_eq!(
        response.warnings,
        vec!["some capability metadata could not be loaded; discovery may be incomplete"],
    );
    let json = serde_json::to_string(&response)?;
    assert!(
        response.plugins.len() + plugin.skills.len() + response.skills.len() <= MAX_CAPABILITIES
    );
    assert!(json.len() <= MAX_RESPONSE_BYTES);
    assert!(response.warnings.len() <= MAX_WARNINGS);
    assert!(!json.contains("PRIVATE_INSTRUCTION_BODY_SENTINEL"));
    Ok(())
}

fn expected_skill(path: PathUri, name: &str) -> ExecutorSkill {
    ExecutorSkill {
        name: name.to_string(),
        description: "Deploy the service.".to_string(),
        short_description: None,
        path,
        discovery_root: None,
        dependencies: None,
        policy: None,
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

#[tokio::test]
async fn discovery_resolves_optional_skill_metadata() -> anyhow::Result<()> {
    use codex_protocol::protocol::Product;
    use codex_skills::SkillDependencies;
    use codex_skills::SkillPolicy;
    use codex_skills::SkillToolDependency;

    let dependencies = Some(SkillDependencies {
        tools: vec![SkillToolDependency {
            r#type: "mcp".into(),
            value: "deploy".into(),
            description: None,
            transport: Some("stdio".into()),
            command: Some("deploy-mcp".into()),
            url: None,
            oauth_callback_port: Some(3118),
        }],
    });
    let policy = Some(SkillPolicy {
        allow_implicit_invocation: Some(false),
        products: vec![Product::Codex, Product::Chatgpt],
    });
    for (yaml, dependencies, policy) in [
        (None, None, None),
        (Some("interface: [invalid]"), None, None),
        (Some("dependencies:\n  tools: [{type: mcp}]"), None, None),
        (Some("dependencies: {tools: []}"), None, None),
        (
            Some(
                r#"dependencies:
  tools:
    - type: " mcp "
      value: " deploy "
      transport: stdio
      command: deploy-mcp
      oauth: {callback_port: 3118}
    - type: mcp
      value: " "
policy:
  allow_implicit_invocation: false
  products: [codex, CHATGPT]
"#,
            ),
            dependencies,
            policy,
        ),
    ] {
        let temp = tempfile::tempdir()?;
        let request = request(temp.path())?;
        let plugin_root = "codex/plugins/cache/test/demo/local";
        write(
            temp.path(),
            &format!("{plugin_root}/.codex-plugin/plugin.json"),
            MANIFEST,
        )?;
        for root in [
            "codex/skills/deploy".to_string(),
            format!("{plugin_root}/skills/deploy"),
        ] {
            write(temp.path(), &format!("{root}/SKILL.md"), SKILL)?;
            if let Some(yaml) = yaml {
                write(temp.path(), &format!("{root}/agents/openai.yaml"), yaml)?;
            }
        }
        let manager = CapabilityManager::new(LocalFileSystem::unsandboxed());
        manager.seed_prewarmed_locations(request.clone()).await?;
        let response = manager
            .get_or_refresh_discovery(DiscoverV2CapabilitiesRequest {
                force_reload: false,
                cwd: PathUri::from_host_native_path(temp.path().join("project"))?,
                sandbox: None,
            })
            .await?;
        let mut expected =
            expected_skill(request.codex_home.join("skills/deploy/SKILL.md")?, "deploy");
        expected.dependencies = dependencies;
        expected.policy = policy;
        expected.discovery_root = Some(SkillDiscoveryRoot {
            id: "codex-home-skills".to_string(),
            path: request.codex_home.join("skills")?,
        });
        assert_eq!(response.skills, vec![expected.clone()]);
        expected.path = request
            .codex_home
            .join("plugins/cache/test/demo/local/skills/deploy/SKILL.md")?;
        expected.name = "demo:deploy".into();
        expected.discovery_root = None;
        assert_eq!(response.plugins[0].skills, vec![expected]);
        assert_eq!(response.warnings, Vec::<String>::new());
    }
    Ok(())
}

#[tokio::test]
async fn standalone_skills_retain_their_search_roots() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path())?;
    let roots = [
        ("codex-home-skills", "codex/skills"),
        ("user-agent-skills", "home/.agents/skills"),
        ("system-skills", "etc/codex/skills"),
    ];
    let mut expected = Vec::new();
    for (id, root) in roots {
        for relative in ["deploy/SKILL.md", "nested/deploy/SKILL.md"] {
            let path = format!("{root}/{relative}");
            write(temp.path(), &path, SKILL)?;
            let mut skill = expected_skill(
                PathUri::from_host_native_path(temp.path().join(path))?,
                "deploy",
            );
            skill.discovery_root = Some(SkillDiscoveryRoot {
                id: id.to_string(),
                path: PathUri::from_host_native_path(temp.path().join(root))?,
            });
            expected.push(skill);
        }
    }
    let manager = CapabilityManager::new(LocalFileSystem::unsandboxed());
    manager.seed_prewarmed_locations(request.clone()).await?;
    let query = DiscoverV2CapabilitiesRequest {
        force_reload: false,
        cwd: PathUri::from_host_native_path(temp.path().join("project"))?,
        sandbox: None,
    };
    let response = manager.get_or_refresh_discovery(query.clone()).await?;
    assert_eq!(response.skills, expected);
    // Cached discovery must retain the same root metadata.
    assert_eq!(manager.get_or_refresh_discovery(query).await?, response);
    Ok(())
}

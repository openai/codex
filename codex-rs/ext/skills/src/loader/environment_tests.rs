use std::fs;

use codex_exec_server::CapabilityRootDiscoverRequest;
use codex_exec_server::CapabilityRootsDiscoverParams;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::discover_capability_roots;
use codex_protocol::protocol::Product;
use codex_skills::EnvironmentSkillMetadata;
use codex_skills::SkillDependencies;
use codex_skills::SkillPolicy;
use codex_skills::SkillToolDependency;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::load_environment_skills_from_discovery;
use super::load_environment_skills_from_root;

#[tokio::test]
async fn direct_environment_loader_preserves_plugin_dependencies_and_product_policy() {
    let root = tempdir().expect("tempdir");
    let skill_dir = root.path().join("skills/deploy");
    fs::create_dir_all(root.path().join(".codex-plugin")).expect("manifest dir");
    fs::create_dir_all(skill_dir.join("agents")).expect("metadata dir");
    fs::write(
        root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"demo-plugin"}"#,
    )
    .expect("manifest");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy the service.\n---\n",
    )
    .expect("skill");
    fs::write(
        skill_dir.join("agents/openai.yaml"),
        r#"
dependencies:
  tools:
    - type: mcp
      value: deploy-server
      description: Deploy MCP
      transport: stdio
      command: deploy-mcp
policy:
  allow_implicit_invocation: false
  products: [codex, atlas]
"#,
    )
    .expect("metadata");

    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome =
        load_environment_skills_from_root(LOCAL_FS.as_ref(), &root_uri, Some(Product::Codex)).await;

    assert_eq!(
        outcome.skills,
        vec![EnvironmentSkillMetadata {
            path_to_skills_md: PathUri::from_host_native_path(skill_dir.join("SKILL.md")).unwrap(),
            name: "demo-plugin:deploy".to_string(),
            description: "Deploy the service.".to_string(),
            short_description: None,
            dependencies: Some(SkillDependencies {
                tools: vec![SkillToolDependency {
                    r#type: "mcp".to_string(),
                    value: "deploy-server".to_string(),
                    description: Some("Deploy MCP".to_string()),
                    transport: Some("stdio".to_string()),
                    command: Some("deploy-mcp".to_string()),
                    url: None,
                    oauth_callback_port: None,
                }],
            }),
            policy: Some(SkillPolicy {
                allow_implicit_invocation: Some(false),
                products: vec![Product::Codex, Product::Atlas],
            }),
        }]
    );
    let atlas =
        load_environment_skills_from_root(LOCAL_FS.as_ref(), &root_uri, Some(Product::Atlas)).await;
    assert_eq!(atlas.skills, outcome.skills);
    let filtered =
        load_environment_skills_from_root(LOCAL_FS.as_ref(), &root_uri, Some(Product::Chatgpt))
            .await;
    assert!(filtered.skills.is_empty());
}

#[tokio::test]
async fn executor_bundle_parser_matches_direct_environment_loader() {
    let root = tempdir().expect("tempdir");
    let plugin_manifest = root.path().join(".codex-plugin/plugin.json");
    let nested_manifest = root.path().join("nested/.claude-plugin/plugin.json");
    let deploy_skill = root.path().join("skills/deploy/SKILL.md");
    let deploy_metadata = root.path().join("skills/deploy/agents/openai.yaml");
    let audit_skill = root.path().join("nested/skills/audit/SKILL.md");
    for (path, contents) in [
        (&plugin_manifest, r#"{"name":"demo"}"#),
        (&nested_manifest, r#"{"name":"nested"}"#),
        (
            &deploy_skill,
            "---\nname: deploy\ndescription: Deploy the service.\n---\n\nDeploy.\n",
        ),
        (
            &deploy_metadata,
            "policy:\n  allow_implicit_invocation: false\n",
        ),
        (
            &audit_skill,
            "---\nname: audit\ndescription: Audit the service.\n---\n\nAudit.\n",
        ),
    ] {
        fs::create_dir_all(path.parent().expect("test file parent")).expect("test directory");
        fs::write(path, contents).expect("test file");
    }

    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let existing = load_environment_skills_from_root(
        LOCAL_FS.as_ref(),
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;
    let response = discover_capability_roots(
        LOCAL_FS.as_ref(),
        CapabilityRootsDiscoverParams {
            roots: vec![CapabilityRootDiscoverRequest {
                id: "demo@1".to_string(),
                path: root_uri,
                sandbox: None,
            }],
        },
    )
    .await
    .expect("capability discovery");
    let bundled = load_environment_skills_from_discovery(
        response.roots.first().expect("discovered root"),
        /*restriction_product*/ None,
    );

    assert_eq!(bundled.warnings, existing.warnings);
    assert_eq!(
        bundled
            .skills
            .iter()
            .map(|skill| skill.metadata.clone())
            .collect::<Vec<_>>(),
        existing.skills
    );
    assert_eq!(
        bundled
            .skills
            .iter()
            .map(|skill| skill.instructions.as_str())
            .collect::<Vec<_>>(),
        vec![
            "---\nname: deploy\ndescription: Deploy the service.\n---\n\nDeploy.\n",
            "---\nname: audit\ndescription: Audit the service.\n---\n\nAudit.\n",
        ]
    );
}

#[tokio::test]
async fn executor_bundle_preserves_parent_namespace_and_manifest_precedence() {
    let plugin = tempdir().expect("tempdir");
    for (relative_path, name) in [
        (".codex-plugin/plugin.json", "codex-name"),
        (".claude-plugin/plugin.json", "claude-name"),
        (".cursor-plugin/plugin.json", "cursor-name"),
    ] {
        let manifest = plugin.path().join(relative_path);
        fs::create_dir_all(manifest.parent().expect("manifest parent"))
            .expect("manifest directory");
        fs::write(&manifest, format!(r#"{{"name":"{name}"}}"#)).expect("manifest");
    }
    let skills_root = plugin.path().join("skills");
    let skill_path = skills_root.join("search/SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill directory");
    fs::write(
        &skill_path,
        "---\nname: search\ndescription: Search the project.\n---\n\nSearch.\n",
    )
    .expect("skill");

    let root_uri = PathUri::from_host_native_path(&skills_root).expect("skills root URI");
    let existing = load_environment_skills_from_root(
        LOCAL_FS.as_ref(),
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;
    let response = discover_capability_roots(
        LOCAL_FS.as_ref(),
        CapabilityRootsDiscoverParams {
            roots: vec![CapabilityRootDiscoverRequest {
                id: "skills-only".to_string(),
                path: root_uri,
                sandbox: None,
            }],
        },
    )
    .await
    .expect("capability discovery");
    let discovery = response.roots.first().expect("discovered root");
    let bundled =
        load_environment_skills_from_discovery(discovery, /*restriction_product*/ None);

    assert_eq!(discovery.plugin, None);
    assert_eq!(discovery.namespace_manifests.len(), 1);
    assert!(
        discovery.namespace_manifests[0]
            .path
            .to_string()
            .ends_with("/.codex-plugin/plugin.json")
    );
    assert_eq!(bundled.warnings, existing.warnings);
    assert_eq!(
        bundled
            .skills
            .iter()
            .map(|skill| skill.metadata.clone())
            .collect::<Vec<_>>(),
        existing.skills
    );
    assert_eq!(bundled.skills[0].metadata.name, "codex-name:search");
}

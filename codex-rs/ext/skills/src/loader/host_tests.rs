use std::fs;
use std::sync::Arc;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillDependencies;
use codex_skills::SkillInterface;
use codex_skills::SkillMetadata;
use codex_skills::SkillModel;
use codex_skills::SkillPolicy;
use codex_skills::SkillToolDependency;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use crate::loader::MAX_NAME_LEN;

use super::HostSkillRoot;
use super::load_host_skill_root;

fn write_skill(root: &TempDir, directory: &str, frontmatter: &str) -> AbsolutePathBuf {
    let skill_dir = root.path().join(directory);
    fs::create_dir_all(&skill_dir).expect("create skill directory");
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(
        &skill_path,
        format!("---\n{frontmatter}\n---\n\n# Instructions\n"),
    )
    .expect("write skill");
    AbsolutePathBuf::from_absolute_path(fs::canonicalize(skill_path).expect("canonical skill path"))
        .expect("absolute skill path")
}

fn write_metadata(root: &TempDir, directory: &str, contents: &str) {
    let metadata_dir = root.path().join(directory).join("agents");
    fs::create_dir_all(&metadata_dir).expect("create metadata directory");
    fs::write(metadata_dir.join("openai.yaml"), contents).expect("write metadata");
}

fn root_for(temp_dir: &TempDir, scope: SkillScope) -> HostSkillRoot {
    HostSkillRoot::host(
        AbsolutePathBuf::from_absolute_path(temp_dir.path()).expect("absolute root"),
        scope,
        Arc::clone(&LOCAL_FS),
    )
}

struct PluginSkillFixture {
    root: TempDir,
    plugin_root: AbsolutePathBuf,
    skill_path: AbsolutePathBuf,
}

impl PluginSkillFixture {
    fn new() -> Self {
        let root = TempDir::new().expect("temp dir");
        let plugin_root = AbsolutePathBuf::from_absolute_path(
            fs::canonicalize(root.path()).expect("canonical plugin root"),
        )
        .expect("absolute plugin root");
        let skill_path = write_skill(
            &root,
            "skills/send-message",
            "name: send-message\ndescription: Send messages",
        );
        Self {
            root,
            plugin_root,
            skill_path,
        }
    }

    fn write_asset(&self, relative_path: &str) {
        let asset_path = self.root.path().join(relative_path);
        fs::create_dir_all(asset_path.parent().expect("asset parent"))
            .expect("create asset directory");
        fs::write(asset_path, "<svg/>").expect("write asset");
    }

    fn write_metadata(&self, contents: &str) {
        write_metadata(&self.root, "skills/send-message", contents);
    }

    fn host_root(&self) -> HostSkillRoot {
        HostSkillRoot::plugin(
            PluginSkillRoot {
                path: self.plugin_root.join("skills"),
                plugin_identity: PluginIdentity {
                    plugin_id: "fixture@test".to_string(),
                    remote_plugin_id: None,
                },
                plugin_namespace: "plugin".to_string(),
                plugin_root: self.plugin_root.clone(),
                discovery_mode: SkillDiscoveryMode::Recursive,
            },
            Arc::clone(&LOCAL_FS),
        )
    }
}

#[tokio::test]
async fn loads_host_frontmatter_dependencies_and_policy() {
    let root = TempDir::new().expect("temp dir");
    let skill_path = write_skill(
        &root,
        "demo",
        "name: demo\ndescription: Demo skill\nmodel: luna\nmetadata:\n  short-description: Short demo",
    );
    write_metadata(
        &root,
        "demo",
        r##"dependencies:
  tools:
    - type: mcp
      value: demo-tool
      description: Demo tool
      oauth:
        callbackPort: 3118
policy:
  allow_implicit_invocation: false
  products: [codex, CHATGPT, atlas]
"##,
    );

    let snapshot = load_host_skill_root(root_for(&root, SkillScope::User)).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "demo".to_string(),
            description: "Demo skill".to_string(),
            short_description: Some("Short demo".to_string()),
            model: Some(SkillModel::Luna),
            interface: None,
            dependencies: Some(SkillDependencies {
                tools: vec![SkillToolDependency {
                    r#type: "mcp".to_string(),
                    value: "demo-tool".to_string(),
                    description: Some("Demo tool".to_string()),
                    transport: None,
                    command: None,
                    url: None,
                    oauth_callback_port: Some(3118),
                }],
            }),
            policy: Some(SkillPolicy {
                allow_implicit_invocation: Some(false),
                products: vec![Product::Codex, Product::Chatgpt, Product::Atlas],
            }),
            path_to_skills_md: skill_path,
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn invalid_optional_metadata_fails_open() {
    let root = TempDir::new().expect("temp dir");
    let skill_path = write_skill(&root, "demo", "name: demo\ndescription: Demo skill");
    write_metadata(&root, "demo", "interface: [not-an-interface]");

    let snapshot = load_host_skill_root(root_for(&root, SkillScope::Repo)).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "demo".to_string(),
            description: "Demo skill".to_string(),
            short_description: None,
            model: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: skill_path,
            scope: SkillScope::Repo,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_host_interface_metadata_and_local_asset_paths() {
    let root = TempDir::new().expect("temp dir");
    let skill_path = write_skill(&root, "demo", "name: demo\ndescription: Demo skill");
    fs::create_dir_all(root.path().join("demo/assets")).expect("create assets");
    write_metadata(
        &root,
        "demo",
        r##"interface:
  display_name: Demo
  short_description: Interface summary
  icon_small: assets/icon.svg
  icon_large: assets/icon-large.png
  brand_color: "#123ABC"
  default_prompt: Run the demo
"##,
    );

    let snapshot = load_host_skill_root(root_for(&root, SkillScope::User)).await;
    let skill_dir = skill_path.parent().expect("skill parent");

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "demo".to_string(),
            description: "Demo skill".to_string(),
            short_description: None,
            model: None,
            interface: Some(SkillInterface {
                display_name: Some("Demo".to_string()),
                short_description: Some("Interface summary".to_string()),
                icon_small: Some(skill_dir.join("assets/icon.svg")),
                icon_large: Some(skill_dir.join("assets/icon-large.png")),
                brand_color: Some("#123ABC".to_string()),
                default_prompt: Some("Run the demo".to_string()),
            }),
            dependencies: None,
            policy: None,
            path_to_skills_md: skill_path,
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_plugin_skill_interface_icons_from_local_and_shared_assets() {
    let fixture = PluginSkillFixture::new();
    fixture.write_asset("skills/send-message/assets/icon.svg");
    fixture.write_asset("assets/logo.svg");
    fixture.write_metadata(
        r#"interface:
  icon_small: "assets/icon.svg"
  icon_large: "../../assets/logo.svg"
"#,
    );

    let snapshot = load_host_skill_root(fixture.host_root()).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "plugin:send-message".to_string(),
            description: "Send messages".to_string(),
            short_description: None,
            model: None,
            interface: Some(SkillInterface {
                display_name: None,
                short_description: None,
                icon_small: Some(
                    fixture
                        .plugin_root
                        .join("skills/send-message/assets/icon.svg"),
                ),
                icon_large: Some(fixture.plugin_root.join("assets/logo.svg")),
                brand_color: None,
                default_prompt: None,
            }),
            dependencies: None,
            policy: None,
            path_to_skills_md: fixture.skill_path,
            scope: SkillScope::User,
            plugin_id: Some("fixture@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn rejects_plugin_skill_interface_icons_outside_shared_assets() {
    let fixture = PluginSkillFixture::new();
    fixture.write_asset("other/icon.svg");
    fixture.write_metadata(
        r#"interface:
  display_name: Send Message
  icon_small: "../../other/icon.svg"
"#,
    );

    let snapshot = load_host_skill_root(fixture.host_root()).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "plugin:send-message".to_string(),
            description: "Send messages".to_string(),
            short_description: None,
            model: None,
            interface: Some(SkillInterface {
                display_name: Some("Send Message".to_string()),
                short_description: None,
                icon_small: None,
                icon_large: None,
                brand_color: None,
                default_prompt: None,
            }),
            dependencies: None,
            policy: None,
            path_to_skills_md: fixture.skill_path,
            scope: SkillScope::User,
            plugin_id: Some("fixture@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn rejects_interface_fields_that_escape_or_fail_validation() {
    let root = TempDir::new().expect("temp dir");
    let skill_path = write_skill(&root, "demo", "name: demo\ndescription: Demo skill");
    write_metadata(
        &root,
        "demo",
        "interface:\n  icon_small: ../outside.svg\n  brand_color: blue\n",
    );

    let snapshot = load_host_skill_root(root_for(&root, SkillScope::User)).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "demo".to_string(),
            description: "Demo skill".to_string(),
            short_description: None,
            model: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: skill_path,
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn skips_hidden_host_skills() {
    let root = TempDir::new().expect("temp dir");
    let visible_path = write_skill(
        &root,
        "visible",
        "name: visible\ndescription: Visible skill",
    );
    write_skill(&root, ".hidden", "name: hidden\ndescription: Hidden skill");

    let snapshot = load_host_skill_root(root_for(&root, SkillScope::User)).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(snapshot.skills.len(), 1);
    assert_eq!(snapshot.skills[0].path_to_skills_md, visible_path);
}

#[tokio::test]
async fn discovers_nested_plugin_namespace_without_plugin_identity() {
    let root = TempDir::new().expect("temp dir");
    let plugin_manifest = root.path().join("nested/.codex-plugin/plugin.json");
    fs::create_dir_all(plugin_manifest.parent().expect("plugin manifest parent"))
        .expect("create plugin manifest directory");
    fs::write(&plugin_manifest, r#"{"name":"plugin-name"}"#).expect("write plugin manifest");
    let skill_path = write_skill(
        &root,
        "nested/skills/demo",
        "name: demo\ndescription: Demo skill",
    );

    let snapshot = load_host_skill_root(root_for(&root, SkillScope::User)).await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "plugin-name:demo".to_string(),
            description: "Demo skill".to_string(),
            short_description: None,
            model: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: skill_path,
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn plugin_root_accepts_maximum_length_qualified_skill_name() {
    let root = TempDir::new().expect("temp dir");
    let plugin_namespace = "p".repeat(MAX_NAME_LEN);
    let skill_name = "s".repeat(MAX_NAME_LEN);
    let skill_path = write_skill(
        &root,
        "skills/search",
        &format!("name: {skill_name}\ndescription: Search skill"),
    );
    let plugin_root = AbsolutePathBuf::from_absolute_path(root.path()).expect("plugin root");

    let snapshot = load_host_skill_root(HostSkillRoot::plugin(
        PluginSkillRoot {
            path: plugin_root.join("skills"),
            plugin_identity: PluginIdentity {
                plugin_id: "demo@test".to_string(),
                remote_plugin_id: None,
            },
            plugin_namespace: plugin_namespace.clone(),
            plugin_root,
            discovery_mode: SkillDiscoveryMode::Recursive,
        },
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: format!("{plugin_namespace}:{skill_name}"),
            description: "Search skill".to_string(),
            short_description: None,
            model: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: skill_path,
            scope: SkillScope::User,
            plugin_id: Some("demo@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn plugin_root_rejects_overlong_qualified_skill_name() {
    let root = TempDir::new().expect("temp dir");
    let skill_name = "s".repeat(MAX_NAME_LEN);
    write_skill(
        &root,
        "skills/search",
        &format!("name: {skill_name}\ndescription: Search skill"),
    );
    let plugin_root = AbsolutePathBuf::from_absolute_path(root.path()).expect("plugin root");

    let snapshot = load_host_skill_root(HostSkillRoot::plugin(
        PluginSkillRoot {
            path: plugin_root.join("skills"),
            plugin_identity: PluginIdentity {
                plugin_id: "demo@test".to_string(),
                remote_plugin_id: None,
            },
            plugin_namespace: "p".repeat(MAX_NAME_LEN + 1),
            plugin_root,
            discovery_mode: SkillDiscoveryMode::Recursive,
        },
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.skills, Vec::new());
    assert_eq!(snapshot.errors.len(), 1);
    assert!(
        snapshot.errors[0]
            .message
            .contains("invalid qualified name")
    );
}

#[tokio::test]
async fn recursive_plugin_root_preserves_owner_namespace_and_shared_asset_policy() {
    let root = TempDir::new().expect("temp dir");
    let skill_path = write_skill(
        &root,
        "skills/group/demo",
        "name: demo\ndescription: Demo skill",
    );
    let nested_manifest = root.path().join("skills/group/.codex-plugin/plugin.json");
    fs::create_dir_all(nested_manifest.parent().expect("nested manifest parent"))
        .expect("create nested plugin manifest directory");
    fs::write(nested_manifest, r#"{"name":"conflicting-plugin"}"#)
        .expect("write nested plugin manifest");
    write_metadata(
        &root,
        "skills/group/demo",
        "interface:\n  icon_small: ../../../assets/logo.svg\n",
    );
    fs::create_dir_all(root.path().join("assets")).expect("create assets directory");
    fs::write(root.path().join("assets/logo.svg"), "<svg/>").expect("write asset");
    let plugin_root = AbsolutePathBuf::from_absolute_path(root.path()).expect("plugin root");
    let canonical_plugin_root = AbsolutePathBuf::from_absolute_path(
        fs::canonicalize(root.path()).expect("canonicalize plugin root"),
    )
    .expect("canonical plugin root");

    let snapshot = load_host_skill_root(HostSkillRoot::plugin(
        PluginSkillRoot {
            path: plugin_root.join("skills"),
            plugin_identity: PluginIdentity {
                plugin_id: "demo@test".to_string(),
                remote_plugin_id: Some("remote-demo".to_string()),
            },
            plugin_namespace: "plugin".to_string(),
            plugin_root,
            discovery_mode: SkillDiscoveryMode::Recursive,
        },
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "plugin:demo".to_string(),
            description: "Demo skill".to_string(),
            short_description: None,
            model: None,
            interface: Some(SkillInterface {
                display_name: None,
                short_description: None,
                icon_small: Some(canonical_plugin_root.join("assets/logo.svg")),
                icon_large: None,
                brand_color: None,
                default_prompt: None,
            }),
            dependencies: None,
            policy: None,
            path_to_skills_md: skill_path.clone(),
            scope: SkillScope::User,
            plugin_id: Some("demo@test".to_string()),
            remote_plugin_id: Some("remote-demo".to_string()),
        }]
    );
    assert_eq!(
        snapshot.skill_discovery_path_by_path.get(&skill_path),
        Some(&skill_path)
    );
}

#[tokio::test]
async fn direct_child_plugin_root_ignores_nested_skills() {
    let root = TempDir::new().expect("temp dir");
    let direct_path = write_skill(
        &root,
        "skills/direct",
        "name: direct\ndescription: Direct skill",
    );
    write_skill(
        &root,
        "skills/nested/too-deep",
        "name: nested\ndescription: Nested skill",
    );
    let plugin_root = AbsolutePathBuf::from_absolute_path(root.path()).expect("plugin root");

    let snapshot = load_host_skill_root(HostSkillRoot::plugin(
        PluginSkillRoot {
            path: plugin_root.join("skills"),
            plugin_identity: PluginIdentity {
                plugin_id: "demo@test".to_string(),
                remote_plugin_id: None,
            },
            plugin_namespace: "plugin".to_string(),
            plugin_root,
            discovery_mode: SkillDiscoveryMode::DirectChildren,
        },
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "plugin:direct".to_string(),
            description: "Direct skill".to_string(),
            short_description: None,
            model: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: direct_path,
            scope: SkillScope::User,
            plugin_id: Some("demo@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn direct_child_plugin_root_skips_skills_resolving_outside_plugin_root() {
    let root = TempDir::new().expect("temp dir");
    let outside_root = TempDir::new().expect("outside temp dir");
    write_skill(
        &outside_root,
        "escaped",
        "name: escaped\ndescription: Escaped skill",
    );
    fs::create_dir_all(root.path().join("skills")).expect("create skills root");
    std::os::unix::fs::symlink(
        outside_root.path().join("escaped"),
        root.path().join("skills/escaped"),
    )
    .expect("create skill symlink");
    let plugin_root = AbsolutePathBuf::from_absolute_path(root.path()).expect("plugin root");

    let snapshot = load_host_skill_root(HostSkillRoot::plugin(
        PluginSkillRoot {
            path: plugin_root.join("skills"),
            plugin_identity: PluginIdentity {
                plugin_id: "demo@test".to_string(),
                remote_plugin_id: None,
            },
            plugin_namespace: "plugin".to_string(),
            plugin_root,
            discovery_mode: SkillDiscoveryMode::DirectChildren,
        },
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(snapshot.skills, Vec::new());
}

#[cfg(unix)]
#[tokio::test]
async fn recursive_plugin_root_preserves_symlinked_skill_discovery_path() {
    let root = TempDir::new().expect("temp dir");
    let target = TempDir::new().expect("target temp dir");
    let target_skill = write_skill(&target, "demo", "name: demo\ndescription: Symlinked skill");
    fs::create_dir_all(root.path().join("skills")).expect("create skills root");
    std::os::unix::fs::symlink(target.path().join("demo"), root.path().join("skills/alias"))
        .expect("create skill symlink");
    let plugin_root = AbsolutePathBuf::from_absolute_path(root.path()).expect("plugin root");

    let snapshot = load_host_skill_root(HostSkillRoot::plugin(
        PluginSkillRoot {
            path: plugin_root.join("skills"),
            plugin_identity: PluginIdentity {
                plugin_id: "demo@test".to_string(),
                remote_plugin_id: None,
            },
            plugin_namespace: "plugin".to_string(),
            plugin_root,
            discovery_mode: SkillDiscoveryMode::Recursive,
        },
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot.skills,
        vec![SkillMetadata {
            name: "plugin:demo".to_string(),
            description: "Symlinked skill".to_string(),
            short_description: None,
            model: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: target_skill.clone(),
            scope: SkillScope::User,
            plugin_id: Some("demo@test".to_string()),
            remote_plugin_id: None,
        }]
    );
    assert_eq!(
        snapshot.skill_discovery_path_by_path.get(&target_skill),
        Some(&snapshot.root.join("alias/SKILL.md"))
    );
}

#[cfg(unix)]
#[tokio::test]
async fn follows_directory_symlinks_except_for_system_scope() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("temp dir");
    let target = TempDir::new().expect("target temp dir");
    let target_skill = write_skill(&target, "demo", "name: demo\ndescription: Symlinked skill");
    symlink(target.path().join("demo"), root.path().join("alias")).expect("create symlink");

    for scope in [SkillScope::User, SkillScope::Repo, SkillScope::Admin] {
        let snapshot = load_host_skill_root(root_for(&root, scope)).await;

        assert_eq!(snapshot.errors, Vec::new());
        assert_eq!(snapshot.skills.len(), 1);
        assert_eq!(
            (
                &snapshot.skills[0].path_to_skills_md,
                snapshot.skills[0].scope
            ),
            (&target_skill, scope)
        );
        assert_eq!(
            snapshot.skill_discovery_path_by_path.get(&target_skill),
            Some(&snapshot.root.join("alias/SKILL.md"))
        );
    }

    let system_snapshot = load_host_skill_root(root_for(&root, SkillScope::System)).await;
    assert_eq!(system_snapshot.errors, Vec::new());
    assert_eq!(system_snapshot.skills, Vec::new());
}

use std::fs;
use std::sync::Arc;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillDependencies;
use codex_skills::SkillMetadata;
use codex_skills::SkillPolicy;
use codex_skills::SkillToolDependency;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

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
    HostSkillRoot {
        path: AbsolutePathBuf::from_absolute_path(temp_dir.path()).expect("absolute root"),
        scope,
        file_system: Arc::clone(&LOCAL_FS),
    }
}

#[tokio::test]
async fn loads_host_frontmatter_dependencies_and_policy() {
    let root = TempDir::new().expect("temp dir");
    let skill_path = write_skill(
        &root,
        "demo",
        "name: demo\ndescription: Demo skill\nmetadata:\n  short-description: Short demo",
    );
    write_metadata(
        &root,
        "demo",
        r##"dependencies:
  tools:
    - type: mcp
      value: demo-tool
      description: Demo tool
policy:
  allow_implicit_invocation: false
  products: [codex]
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
            interface: None,
            dependencies: Some(SkillDependencies {
                tools: vec![SkillToolDependency {
                    r#type: "mcp".to_string(),
                    value: "demo-tool".to_string(),
                    description: Some("Demo tool".to_string()),
                    transport: None,
                    command: None,
                    url: None,
                }],
            }),
            policy: Some(SkillPolicy {
                allow_implicit_invocation: Some(false),
                products: vec![Product::Codex],
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

#[cfg(unix)]
#[tokio::test]
async fn follows_directory_symlinks_for_user_but_not_system_scope() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("temp dir");
    let target = TempDir::new().expect("target temp dir");
    let target_skill = write_skill(&target, "demo", "name: demo\ndescription: Symlinked skill");
    symlink(target.path().join("demo"), root.path().join("alias")).expect("create symlink");

    let user_snapshot = load_host_skill_root(root_for(&root, SkillScope::User)).await;
    let system_snapshot = load_host_skill_root(root_for(&root, SkillScope::System)).await;

    assert_eq!(user_snapshot.errors, Vec::new());
    assert_eq!(user_snapshot.skills.len(), 1);
    assert_eq!(user_snapshot.skills[0].path_to_skills_md, target_skill);
    assert_eq!(system_snapshot.errors, Vec::new());
    assert_eq!(system_snapshot.skills, Vec::new());
}

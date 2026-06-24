use std::fs;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use crate::model::SkillDependencies;
use crate::model::SkillPolicy;
use crate::model::SkillToolDependency;

use super::EnvironmentSkillMetadata;
use super::load_environment_skills_from_root;

#[tokio::test]
async fn loads_plugin_namespace_dependencies_and_policy() {
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
policy:
  allow_implicit_invocation: false
  products: [codex]
"#,
    )
    .expect("metadata");

    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome =
        load_environment_skills_from_root(LOCAL_FS.as_ref(), &root_uri, Some(Product::Codex)).await;

    assert_eq!(
        outcome.skills,
        vec![EnvironmentSkillMetadata {
            path_to_skills_md: PathUri::from_host_native_path(skill_dir.join("SKILL.md"),).unwrap(),
            name: "demo-plugin:deploy".to_string(),
            description: "Deploy the service.".to_string(),
            short_description: None,
            dependencies: Some(SkillDependencies {
                tools: vec![SkillToolDependency {
                    r#type: "mcp".to_string(),
                    value: "deploy-server".to_string(),
                    description: Some("Deploy MCP".to_string()),
                    transport: None,
                    command: None,
                    url: None,
                }],
            }),
            policy: Some(SkillPolicy {
                allow_implicit_invocation: Some(false),
                products: vec![Product::Codex],
            }),
        }]
    );
    let filtered =
        load_environment_skills_from_root(LOCAL_FS.as_ref(), &root_uri, Some(Product::Chatgpt))
            .await;
    assert!(filtered.skills.is_empty());
}

#[tokio::test]
async fn uses_nearest_plugin_namespace_below_mixed_capability_root() {
    let root = tempdir().expect("tempdir");
    let standalone_skill = root.path().join("standalone/SKILL.md");
    let outer_root = root.path().join("plugins/outer");
    let outer_skill = outer_root.join("skills/deploy/SKILL.md");
    let inner_root = outer_root.join("nested/inner");
    let inner_skill = inner_root.join("skills/audit/SKILL.md");

    for path in [&standalone_skill, &outer_skill, &inner_skill] {
        fs::create_dir_all(path.parent().expect("skill parent")).expect("skill dir");
    }
    for (plugin_root, name) in [(&outer_root, "outer"), (&inner_root, "inner")] {
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("manifest dir");
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(r#"{{"name":"{name}"}}"#),
        )
        .expect("manifest");
    }
    for (path, name) in [
        (&standalone_skill, "standalone"),
        (&outer_skill, "deploy"),
        (&inner_skill, "audit"),
    ] {
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
    }

    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = load_environment_skills_from_root(
        LOCAL_FS.as_ref(),
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(
        outcome.skills,
        vec![
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&inner_skill).unwrap(),
                name: "inner:audit".to_string(),
                description: "audit skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&outer_skill).unwrap(),
                name: "outer:deploy".to_string(),
                description: "deploy skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&standalone_skill).unwrap(),
                name: "standalone".to_string(),
                description: "standalone skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
        ]
    );

    let outer_root_uri = PathUri::from_host_native_path(&outer_root).expect("outer root URI");
    let outcome = load_environment_skills_from_root(
        LOCAL_FS.as_ref(),
        &outer_root_uri,
        /*restriction_product*/ None,
    )
    .await;

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(
        outcome.skills,
        vec![
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&inner_skill).unwrap(),
                name: "inner:audit".to_string(),
                description: "audit skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&outer_skill).unwrap(),
                name: "outer:deploy".to_string(),
                description: "deploy skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
        ]
    );
}

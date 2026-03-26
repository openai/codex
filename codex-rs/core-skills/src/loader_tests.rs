use super::*;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::MacOsAutomationPermission;
use codex_protocol::models::MacOsContactsPermission;
use codex_protocol::models::MacOsPreferencesPermission;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use toml::Value as TomlValue;

const REPO_ROOT_CONFIG_DIR_NAME: &str = ".codex";

struct TestConfig {
    cwd: PathBuf,
    config_layer_stack: ConfigLayerStack,
}

async fn make_config(codex_home: &TempDir) -> TestConfig {
    make_config_for_cwd(codex_home, codex_home.path().to_path_buf()).await
}

fn config_file(path: PathBuf) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(path).expect("config file path should be absolute")
}

fn project_layers_for_cwd(cwd: &Path) -> Vec<ConfigLayerEntry> {
    let cwd_dir = if cwd.is_dir() {
        cwd.to_path_buf()
    } else {
        cwd.parent()
            .expect("file cwd should have a parent directory")
            .to_path_buf()
    };
    let project_root = cwd_dir
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .unwrap_or(cwd_dir.as_path())
        .to_path_buf();

    let mut layers = cwd_dir
        .ancestors()
        .scan(false, |done, dir| {
            if *done {
                None
            } else {
                if dir == project_root {
                    *done = true;
                }
                Some(dir.to_path_buf())
            }
        })
        .collect::<Vec<_>>();
    layers.reverse();

    layers
        .into_iter()
        .filter_map(|dir| {
            let dot_codex = dir.join(REPO_ROOT_CONFIG_DIR_NAME);
            dot_codex.is_dir().then(|| {
                ConfigLayerEntry::new(
                    ConfigLayerSource::Project {
                        dot_codex_folder: AbsolutePathBuf::from_absolute_path(dot_codex)
                            .expect("project .codex path should be absolute"),
                    },
                    TomlValue::Table(toml::map::Map::new()),
                )
            })
        })
        .collect()
}

async fn make_config_for_cwd(codex_home: &TempDir, cwd: PathBuf) -> TestConfig {
    let user_config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let system_config_path = codex_home.path().join("etc/codex/config.toml");
    fs::create_dir_all(
        system_config_path
            .parent()
            .expect("system config path should have a parent"),
    )
    .expect("create fake system config dir");

    let mut layers = vec![
        ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: config_file(system_config_path),
            },
            TomlValue::Table(toml::map::Map::new()),
        ),
        ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: config_file(user_config_path),
            },
            TomlValue::Table(toml::map::Map::new()),
        ),
    ];
    layers.extend(project_layers_for_cwd(&cwd));

    TestConfig {
        cwd,
        config_layer_stack: ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("valid config layer stack"),
    }
}

fn load_skills_for_test(config: &TestConfig) -> SkillLoadOutcome {
    // Keep unit tests hermetic by never scanning the real `$HOME/.agents/skills`.
    super::load_skills_from_roots(super::skill_roots_with_home_dir(
        &config.config_layer_stack,
        &config.cwd,
        None,
        Vec::new(),
    ))
}

fn mark_as_git_repo(dir: &Path) {
    // Config/project-root discovery only checks for the presence of `.git` (file or dir),
    // so we can avoid shelling out to `git init` in tests.
    fs::write(dir.join(".git"), "gitdir: fake\n").unwrap();
}

fn normalized(path: &Path) -> PathBuf {
    canonicalize_path(path).unwrap_or_else(|_| path.to_path_buf())
}

#[test]
fn skill_roots_from_layer_stack_maps_user_to_user_and_system_cache_and_system_to_admin()
-> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let system_folder = tmp.path().join("etc/codex");
    let home_folder = tmp.path().join("home");
    let user_folder = home_folder.join("codex");
    fs::create_dir_all(&system_folder)?;
    fs::create_dir_all(&user_folder)?;

    // The file path doesn't need to exist; it's only used to derive the config folder.
    let system_file = AbsolutePathBuf::from_absolute_path(system_folder.join("config.toml"))?;
    let user_file = AbsolutePathBuf::from_absolute_path(user_folder.join("config.toml"))?;

    let layers = vec![
        ConfigLayerEntry::new(
            ConfigLayerSource::System { file: system_file },
            TomlValue::Table(toml::map::Map::new()),
        ),
        ConfigLayerEntry::new(
            ConfigLayerSource::User { file: user_file },
            TomlValue::Table(toml::map::Map::new()),
        ),
    ];
    let stack = ConfigLayerStack::new(
        layers,
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )?;

    let got = skill_roots_from_layer_stack(&stack, Some(&home_folder))
        .into_iter()
        .map(|root| (root.scope, root.path))
        .collect::<Vec<_>>();

    assert_eq!(
        got,
        vec![
            (SkillScope::User, user_folder.join("skills")),
            (
                SkillScope::User,
                home_folder.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME)
            ),
            (
                SkillScope::System,
                user_folder.join("skills").join(".system")
            ),
            (SkillScope::Admin, system_folder.join("skills")),
        ]
    );

    Ok(())
}

#[test]
fn skill_roots_from_layer_stack_includes_disabled_project_layers() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let home_folder = tmp.path().join("home");
    let user_folder = home_folder.join("codex");
    fs::create_dir_all(&user_folder)?;

    let project_root = tmp.path().join("repo");
    let dot_codex = project_root.join(".codex");
    fs::create_dir_all(&dot_codex)?;

    let user_file = AbsolutePathBuf::from_absolute_path(user_folder.join("config.toml"))?;
    let project_dot_codex = AbsolutePathBuf::from_absolute_path(&dot_codex)?;

    let layers = vec![
        ConfigLayerEntry::new(
            ConfigLayerSource::User { file: user_file },
            TomlValue::Table(toml::map::Map::new()),
        ),
        ConfigLayerEntry::new_disabled(
            ConfigLayerSource::Project {
                dot_codex_folder: project_dot_codex,
            },
            TomlValue::Table(toml::map::Map::new()),
            "marked untrusted",
        ),
    ];
    let stack = ConfigLayerStack::new(
        layers,
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )?;

    let got = skill_roots_from_layer_stack(&stack, Some(&home_folder))
        .into_iter()
        .map(|root| (root.scope, root.path))
        .collect::<Vec<_>>();

    assert_eq!(
        got,
        vec![
            (SkillScope::Repo, dot_codex.join("skills")),
            (SkillScope::User, user_folder.join("skills")),
            (
                SkillScope::User,
                home_folder.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME)
            ),
            (
                SkillScope::System,
                user_folder.join("skills").join(".system")
            ),
        ]
    );

    Ok(())
}

#[test]
fn loads_skills_from_home_agents_dir_for_user_scope() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let home_folder = tmp.path().join("home");
    let user_folder = home_folder.join("codex");
    fs::create_dir_all(&user_folder)?;

    let user_file = AbsolutePathBuf::from_absolute_path(user_folder.join("config.toml"))?;
    let layers = vec![ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        TomlValue::Table(toml::map::Map::new()),
    )];
    let stack = ConfigLayerStack::new(
        layers,
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )?;

    let skill_path = write_skill_at(
        &home_folder.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME),
        "agents-home",
        "agents-home-skill",
        "from home agents",
    );

    let outcome = load_skills_from_roots(skill_roots_from_layer_stack(&stack, Some(&home_folder)));
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "agents-home-skill".to_string(),
            description: "from home agents".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );

    Ok(())
}

fn write_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) -> PathBuf {
    write_skill_at(&codex_home.path().join("skills"), dir, name, description)
}

fn write_system_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) -> PathBuf {
    write_skill_at(
        &codex_home.path().join("skills/.system"),
        dir,
        name,
        description,
    )
}

fn write_skill_at(root: &Path, dir: &str, name: &str, description: &str) -> PathBuf {
    let skill_dir = root.join(dir);
    fs::create_dir_all(&skill_dir).unwrap();
    let indented_description = description.replace('\n', "\n  ");
    let content =
        format!("---\nname: {name}\ndescription: |-\n  {indented_description}\n---\n\n# Body\n");
    let path = skill_dir.join(SKILLS_FILENAME);
    fs::write(&path, content).unwrap();
    path
}

fn write_raw_skill_at(root: &Path, dir: &str, frontmatter: &str) -> PathBuf {
    let skill_dir = root.join(dir);
    fs::create_dir_all(&skill_dir).unwrap();
    let path = skill_dir.join(SKILLS_FILENAME);
    let content = format!("---\n{frontmatter}\n---\n\n# Body\n");
    fs::write(&path, content).unwrap();
    path
}

fn write_skill_metadata_at(skill_dir: &Path, contents: &str) -> PathBuf {
    let path = skill_dir
        .join(SKILLS_METADATA_DIR)
        .join(SKILLS_METADATA_FILENAME);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

fn write_skill_interface_at(skill_dir: &Path, contents: &str) -> PathBuf {
    write_skill_metadata_at(skill_dir, contents)
}

#[tokio::test]
async fn loads_skill_dependencies_metadata_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "dep-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
{
  "dependencies": {
    "tools": [
      {
        "type": "env_var",
        "value": "GITHUB_TOKEN",
        "description": "GitHub API token with repo scopes"
      },
      {
        "type": "mcp",
        "value": "github",
        "description": "GitHub MCP server",
        "transport": "streamable_http",
        "url": "https://example.com/mcp"
      },
      {
        "type": "cli",
        "value": "gh",
        "description": "GitHub CLI"
      },
      {
        "type": "mcp",
        "value": "local-gh",
        "description": "Local GH MCP server",
        "transport": "stdio",
        "command": "gh-mcp"
      }
    ]
  }
}
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "dep-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: None,
            dependencies: Some(SkillDependencies {
                tools: vec![
                    SkillToolDependency {
                        r#type: "env_var".to_string(),
                        value: "GITHUB_TOKEN".to_string(),
                        description: Some("GitHub API token with repo scopes".to_string()),
                        transport: None,
                        command: None,
                        url: None,
                    },
                    SkillToolDependency {
                        r#type: "mcp".to_string(),
                        value: "github".to_string(),
                        description: Some("GitHub MCP server".to_string()),
                        transport: Some("streamable_http".to_string()),
                        command: None,
                        url: Some("https://example.com/mcp".to_string()),
                    },
                    SkillToolDependency {
                        r#type: "cli".to_string(),
                        value: "gh".to_string(),
                        description: Some("GitHub CLI".to_string()),
                        transport: None,
                        command: None,
                        url: None,
                    },
                    SkillToolDependency {
                        r#type: "mcp".to_string(),
                        value: "local-gh".to_string(),
                        description: Some("Local GH MCP server".to_string()),
                        transport: Some("stdio".to_string()),
                        command: Some("gh-mcp".to_string()),
                        url: None,
                    },
                ],
            }),
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn loads_skill_interface_metadata_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");
    let normalized_skill_dir = normalized(skill_dir);

    write_skill_interface_at(
        skill_dir,
        r##"
interface:
  display_name: "UI Skill"
  short_description: "  short    desc   "
  icon_small: "./assets/small-400px.png"
  icon_large: "./assets/large-logo.svg"
  brand_color: "#3B82F6"
  default_prompt: "  default   prompt   "
"##,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let user_skills: Vec<SkillMetadata> = outcome
        .skills
        .into_iter()
        .filter(|skill| skill.scope == SkillScope::User)
        .collect();
    assert_eq!(
        user_skills,
        vec![SkillMetadata {
            name: "ui-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: Some(SkillInterface {
                display_name: Some("UI Skill".to_string()),
                short_description: Some("short desc".to_string()),
                icon_small: Some(normalized_skill_dir.join("assets/small-400px.png")),
                icon_large: Some(normalized_skill_dir.join("assets/large-logo.svg")),
                brand_color: Some("#3B82F6".to_string()),
                default_prompt: Some("default prompt".to_string()),
            }),
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(skill_path.as_path()),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn loads_skill_policy_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "policy-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
policy:
  allow_implicit_invocation: false
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].policy,
        Some(SkillPolicy {
            allow_implicit_invocation: Some(false),
            products: vec![],
        })
    );
    assert!(outcome.allowed_skills_for_implicit_invocation().is_empty());
}

#[tokio::test]
async fn empty_skill_policy_defaults_to_allow_implicit_invocation() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "policy-empty", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
policy: {}
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].policy,
        Some(SkillPolicy {
            allow_implicit_invocation: None,
            products: vec![],
        })
    );
    assert_eq!(
        outcome.allowed_skills_for_implicit_invocation(),
        outcome.skills
    );
}

#[tokio::test]
async fn loads_skill_policy_products_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "policy-products", "from yaml");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
policy:
  products:
    - codex
    - CHATGPT
    - atlas
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].policy,
        Some(SkillPolicy {
            allow_implicit_invocation: None,
            products: vec![Product::Codex, Product::Chatgpt, Product::Atlas],
        })
    );
}

#[tokio::test]
async fn loads_skill_permissions_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "permissions-skill", "from yaml");
    let skill_dir = skill_path.parent().expect("skill dir");
    fs::create_dir_all(skill_dir.join("data")).expect("create read path");
    fs::create_dir_all(skill_dir.join("output")).expect("create write path");

    write_skill_metadata_at(
        skill_dir,
        r#"
permissions:
  network:
    enabled: true
  file_system:
    read:
      - "./data"
    write:
      - "./output"
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].permission_profile,
        Some(PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions {
                read: Some(vec![
                    AbsolutePathBuf::try_from(normalized(skill_dir.join("data").as_path()))
                        .expect("absolute data path"),
                ]),
                write: Some(vec![
                    AbsolutePathBuf::try_from(normalized(skill_dir.join("output").as_path()))
                        .expect("absolute output path"),
                ]),
            }),
            macos: None,
        })
    );
    assert_eq!(outcome.skills[0].managed_network_override, None);
}

#[tokio::test]
async fn empty_skill_permissions_do_not_create_profile() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "permissions-empty", "from yaml");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
permissions: {}
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(outcome.skills[0].permission_profile, None);
}

#[test]
fn normalize_permissions_splits_managed_network_overrides() {
    let (permission_profile, managed_network_override) =
        normalize_permissions(Some(SkillPermissionProfile {
            network: Some(SkillNetworkPermissions {
                enabled: Some(true),
                allowed_domains: Some(vec!["skill.example.com".to_string()]),
                denied_domains: Some(vec!["blocked.skill.example.com".to_string()]),
            }),
            file_system: None,
            macos: None,
        }));

    assert_eq!(
        permission_profile,
        Some(PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: None,
            macos: None,
        })
    );
    assert_eq!(
        managed_network_override,
        Some(SkillManagedNetworkOverride {
            allowed_domains: Some(vec!["skill.example.com".to_string()]),
            denied_domains: Some(vec!["blocked.skill.example.com".to_string()]),
        })
    );
}

#[test]
fn normalize_permissions_preserves_network_gate_separately_from_overrides() {
    let (permission_profile, managed_network_override) =
        normalize_permissions(Some(SkillPermissionProfile {
            network: Some(SkillNetworkPermissions {
                enabled: Some(false),
                allowed_domains: Some(vec!["skill.example.com".to_string()]),
                denied_domains: None,
            }),
            file_system: None,
            macos: None,
        }));

    assert_eq!(
        permission_profile,
        Some(PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(false),
            }),
            file_system: None,
            macos: None,
        })
    );
    assert_eq!(
        managed_network_override,
        Some(SkillManagedNetworkOverride {
            allowed_domains: Some(vec!["skill.example.com".to_string()]),
            denied_domains: None,
        })
    );
}

#[test]
fn skill_metadata_parses_macos_permissions_yaml() {
    let parsed = serde_yaml::from_str::<SkillMetadataFile>(
        r#"
permissions:
  macos:
    macos_preferences: "read_write"
    macos_automation:
      - "com.apple.Notes"
    macos_launch_services: true
    macos_accessibility: true
    macos_calendar: true
"#,
    )
    .expect("parse skill metadata");

    assert_eq!(
        parsed.permissions,
        Some(SkillPermissionProfile {
            network: None,
            file_system: None,
            macos: Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Notes".to_string(),
                ]),
                macos_launch_services: true,
                macos_accessibility: true,
                macos_calendar: true,
                macos_reminders: false,
                macos_contacts: MacOsContactsPermission::None,
            }),
        })
    );
}

#[test]
fn skill_metadata_parses_macos_reminders_permission_yaml() {
    let parsed = serde_yaml::from_str::<SkillMetadataFile>(
        r#"
permissions:
  macos:
    macos_reminders: true
"#,
    )
    .expect("parse reminders skill metadata");

    assert_eq!(
        parsed.permissions,
        Some(SkillPermissionProfile {
            network: None,
            file_system: None,
            macos: Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadOnly,
                macos_automation: MacOsAutomationPermission::None,
                macos_launch_services: false,
                macos_accessibility: false,
                macos_calendar: false,
                macos_reminders: true,
                macos_contacts: MacOsContactsPermission::None,
            }),
        })
    );
}

#[test]
fn skill_metadata_parses_network_domain_overrides_under_permissions() {
    let parsed = serde_yaml::from_str::<SkillMetadataFile>(
        r#"
permissions:
  network:
    enabled: true
    allowed_domains:
      - "skill.example.com"
    denied_domains:
      - "blocked.skill.example.com"
"#,
    )
    .expect("parse network skill metadata");

    assert_eq!(
        parsed.permissions,
        Some(SkillPermissionProfile {
            network: Some(SkillNetworkPermissions {
                enabled: Some(true),
                allowed_domains: Some(vec!["skill.example.com".to_string()]),
                denied_domains: Some(vec!["blocked.skill.example.com".to_string()]),
            }),
            file_system: None,
            macos: None,
        })
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn loads_skill_macos_permissions_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "permissions-macos", "from yaml");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
permissions:
  macos:
    macos_preferences: "read_write"
    macos_automation:
      - "com.apple.Notes"
    macos_launch_services: true
    macos_accessibility: true
    macos_calendar: true
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].permission_profile,
        Some(PermissionProfile {
            macos: Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Notes".to_string()
                ],),
                macos_launch_services: true,
                macos_accessibility: true,
                macos_calendar: true,
                macos_reminders: false,
                macos_contacts: MacOsContactsPermission::None,
            }),
            ..Default::default()
        })
    );
}

#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn loads_skill_macos_permissions_from_yaml_non_macos_does_not_create_profile() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "permissions-macos", "from yaml");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
permissions:
  macos:
    macos_preferences: "read_write"
    macos_automation:
      - "com.apple.Notes"
    macos_launch_services: true
    macos_accessibility: true
    macos_calendar: true
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].permission_profile,
        Some(PermissionProfile {
            macos: Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Notes".to_string()
                ],),
                macos_launch_services: true,
                macos_accessibility: true,
                macos_calendar: true,
                macos_reminders: false,
                macos_contacts: MacOsContactsPermission::None,
            }),
            ..Default::default()
        })
    );
}

#[tokio::test]
async fn accepts_icon_paths_under_assets_dir() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");
    let normalized_skill_dir = normalized(skill_dir);

    write_skill_interface_at(
        skill_dir,
        r#"
{
  "interface": {
    "display_name": "UI Skill",
    "icon_small": "assets/icon.png",
    "icon_large": "./assets/logo.svg"
  }
}
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "ui-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: Some(SkillInterface {
                display_name: Some("UI Skill".to_string()),
                short_description: None,
                icon_small: Some(normalized_skill_dir.join("assets/icon.png")),
                icon_large: Some(normalized_skill_dir.join("assets/logo.svg")),
                brand_color: None,
                default_prompt: None,
            }),
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn ignores_invalid_brand_color() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_interface_at(
        skill_dir,
        r#"
{
  "interface": {
    "brand_color": "blue"
  }
}
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "ui-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn ignores_default_prompt_over_max_length() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");
    let normalized_skill_dir = normalized(skill_dir);
    let too_long = "x".repeat(MAX_DEFAULT_PROMPT_LEN + 1);

    write_skill_interface_at(
        skill_dir,
        &format!(
            r##"
{{
  "interface": {{
    "display_name": "UI Skill",
    "icon_small": "./assets/small-400px.png",
    "default_prompt": "{too_long}"
  }}
}}
"##
        ),
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "ui-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: Some(SkillInterface {
                display_name: Some("UI Skill".to_string()),
                short_description: None,
                icon_small: Some(normalized_skill_dir.join("assets/small-400px.png")),
                icon_large: None,
                brand_color: None,
                default_prompt: None,
            }),
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn drops_interface_when_icons_are_invalid() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_interface_at(
        skill_dir,
        r#"
{
  "interface": {
    "icon_small": "icon.png",
    "icon_large": "./assets/../logo.svg"
  }
}
"#,
    );

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "ui-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[tokio::test]
#[cfg(unix)]
async fn loads_skills_via_symlinked_subdir_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let shared_skill_path = write_skill_at(shared.path(), "demo", "linked-skill", "from link");

    fs::create_dir_all(codex_home.path().join("skills")).unwrap();
    symlink_dir(shared.path(), &codex_home.path().join("skills/shared"));

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "linked-skill".to_string(),
            description: "from link".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&shared_skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
#[cfg(unix)]
async fn ignores_symlinked_skill_file_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let shared_skill_path = write_skill_at(shared.path(), "demo", "linked-file-skill", "from link");

    let skill_dir = codex_home.path().join("skills/demo");
    fs::create_dir_all(&skill_dir).unwrap();
    symlink_file(&shared_skill_path, &skill_dir.join(SKILLS_FILENAME));

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills, Vec::new());
}

#[tokio::test]
#[cfg(unix)]
async fn does_not_loop_on_symlink_cycle_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");

    // Create a cycle:
    //   $CODEX_HOME/skills/cycle/loop -> $CODEX_HOME/skills/cycle
    let cycle_dir = codex_home.path().join("skills/cycle");
    fs::create_dir_all(&cycle_dir).unwrap();
    symlink_dir(&cycle_dir, &cycle_dir.join("loop"));

    let skill_path = write_skill_at(&cycle_dir, "demo", "cycle-skill", "still loads");

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "cycle-skill".to_string(),
            description: "still loads".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[test]
#[cfg(unix)]
fn loads_skills_via_symlinked_subdir_for_admin_scope() {
    let admin_root = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let shared_skill_path =
        write_skill_at(shared.path(), "demo", "admin-linked-skill", "from link");
    fs::create_dir_all(admin_root.path()).unwrap();
    symlink_dir(shared.path(), &admin_root.path().join("shared"));

    let outcome = load_skills_from_roots([SkillRoot {
        path: admin_root.path().to_path_buf(),
        scope: SkillScope::Admin,
    }]);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "admin-linked-skill".to_string(),
            description: "from link".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&shared_skill_path),
            scope: SkillScope::Admin,
        }]
    );
}

#[tokio::test]
#[cfg(unix)]
async fn loads_skills_via_symlinked_subdir_for_repo_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());
    let shared = tempfile::tempdir().expect("tempdir");

    let linked_skill_path = write_skill_at(shared.path(), "demo", "repo-linked-skill", "from link");
    let repo_skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    fs::create_dir_all(&repo_skills_root).unwrap();
    symlink_dir(shared.path(), &repo_skills_root.join("shared"));

    let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "repo-linked-skill".to_string(),
            description: "from link".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&linked_skill_path),
            scope: SkillScope::Repo,
        }]
    );
}

#[tokio::test]
#[cfg(unix)]
async fn system_scope_ignores_symlinked_subdir() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    write_skill_at(shared.path(), "demo", "system-linked-skill", "from link");

    let system_root = codex_home.path().join("skills/.system");
    fs::create_dir_all(&system_root).unwrap();
    symlink_dir(shared.path(), &system_root.join("shared"));

    let outcome = load_skills_from_roots([SkillRoot {
        path: system_root,
        scope: SkillScope::System,
    }]);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 0);
}

#[tokio::test]
async fn respects_max_scan_depth_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");

    let within_depth_path = write_skill(
        &codex_home,
        "d0/d1/d2/d3/d4/d5",
        "within-depth-skill",
        "loads",
    );
    let _too_deep_path = write_skill(
        &codex_home,
        "d0/d1/d2/d3/d4/d5/d6",
        "too-deep-skill",
        "should not load",
    );

    let skills_root = codex_home.path().join("skills");
    let outcome = load_skills_from_roots([SkillRoot {
        path: skills_root,
        scope: SkillScope::User,
    }]);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "within-depth-skill".to_string(),
            description: "loads".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&within_depth_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn loads_valid_skill() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "demo-skill", "does things\ncarefully");
    let cfg = make_config(&codex_home).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "demo-skill".to_string(),
            description: "does things carefully".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn falls_back_to_directory_name_when_skill_name_is_missing() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_raw_skill_at(
        &codex_home.path().join("skills"),
        "directory-derived",
        "description: fallback name",
    );
    let cfg = make_config(&codex_home).await;

    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "directory-derived".to_string(),
            description: "fallback name".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn namespaces_plugin_skills_using_plugin_name() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("plugins/sample");
    let skill_path = write_raw_skill_at(
        &plugin_root.join("skills"),
        "sample-search",
        "description: search sample data",
    );
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .unwrap();

    let outcome = load_skills_from_roots([SkillRoot {
        path: plugin_root.join("skills"),
        scope: SkillScope::User,
    }]);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "sample:sample-search".to_string(),
            description: "search sample data".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn loads_short_description_from_metadata() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_dir = codex_home.path().join("skills/demo");
    fs::create_dir_all(&skill_dir).unwrap();
    let contents = "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: short summary\n---\n\n# Body\n";
    let skill_path = skill_dir.join(SKILLS_FILENAME);
    fs::write(&skill_path, contents).unwrap();

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "demo-skill".to_string(),
            description: "long description".to_string(),
            short_description: Some("short summary".to_string()),
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
        }]
    );
}

#[tokio::test]
async fn enforces_short_description_length_limits() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_dir = codex_home.path().join("skills/demo");
    fs::create_dir_all(&skill_dir).unwrap();
    let too_long = "x".repeat(MAX_SHORT_DESCRIPTION_LEN + 1);
    let contents = format!(
        "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: {too_long}\n---\n\n# Body\n"
    );
    fs::write(skill_dir.join(SKILLS_FILENAME), contents).unwrap();

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);
    assert_eq!(outcome.skills.len(), 0);
    assert_eq!(outcome.errors.len(), 1);
    assert!(
        outcome.errors[0]
            .message
            .contains("invalid metadata.short-description"),
        "expected length error, got: {:?}",
        outcome.errors
    );
}

#[tokio::test]
async fn skips_hidden_and_invalid() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let hidden_dir = codex_home.path().join("skills/.hidden");
    fs::create_dir_all(&hidden_dir).unwrap();
    fs::write(
        hidden_dir.join(SKILLS_FILENAME),
        "---\nname: hidden\ndescription: hidden\n---\n",
    )
    .unwrap();

    // Invalid because missing closing frontmatter.
    let invalid_dir = codex_home.path().join("skills/invalid");
    fs::create_dir_all(&invalid_dir).unwrap();
    fs::write(invalid_dir.join(SKILLS_FILENAME), "---\nname: bad").unwrap();

    let cfg = make_config(&codex_home).await;
    let outcome = load_skills_for_test(&cfg);
    assert_eq!(outcome.skills.len(), 0);
    assert_eq!(outcome.errors.len(), 1);
    assert!(
        outcome.errors[0]
            .message
            .contains("missing YAML frontmatter"),
        "expected frontmatter error"
    );
}

#[tokio::test]
async fn enforces_length_limits() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let max_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN);
    write_skill(&codex_home, "max-len", "max-len", &max_desc);
    let cfg = make_config(&codex_home).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);

    let too_long_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN + 1);
    write_skill(&codex_home, "too-long", "too-long", &too_long_desc);
    let outcome = load_skills_for_test(&cfg);
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(outcome.errors.len(), 1);
    assert!(
        outcome.errors[0].message.contains("invalid description"),
        "expected length error"
    );
}

#[tokio::test]
async fn loads_skills_from_repo_root() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());

    let skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let skill_path = write_skill_at(&skills_root, "repo", "repo-skill", "from repo");
    let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "repo-skill".to_string(),
            description: "from repo".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
        }]
    );
}

#[tokio::test]
async fn loads_skills_from_agents_dir_without_codex_dir() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());

    let skill_path = write_skill_at(
        &repo_dir.path().join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME),
        "agents",
        "agents-skill",
        "from agents",
    );
    let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "agents-skill".to_string(),
            description: "from agents".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
        }]
    );
}

#[tokio::test]
async fn loads_skills_from_all_codex_dirs_under_project_root() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());

    let nested_dir = repo_dir.path().join("nested/inner");
    fs::create_dir_all(&nested_dir).unwrap();

    let root_skill_path = write_skill_at(
        &repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "root",
        "root-skill",
        "from root",
    );
    let nested_skill_path = write_skill_at(
        &repo_dir
            .path()
            .join("nested")
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "nested",
        "nested-skill",
        "from nested",
    );

    let cfg = make_config_for_cwd(&codex_home, nested_dir).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "nested-skill".to_string(),
                description: "from nested".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permission_profile: None,
                managed_network_override: None,
                path_to_skills_md: normalized(&nested_skill_path),
                scope: SkillScope::Repo,
            },
            SkillMetadata {
                name: "root-skill".to_string(),
                description: "from root".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permission_profile: None,
                managed_network_override: None,
                path_to_skills_md: normalized(&root_skill_path),
                scope: SkillScope::Repo,
            },
        ]
    );
}

#[tokio::test]
async fn loads_skills_from_codex_dir_when_not_git_repo() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let work_dir = tempfile::tempdir().expect("tempdir");

    let skill_path = write_skill_at(
        &work_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "local",
        "local-skill",
        "from cwd",
    );

    let cfg = make_config_for_cwd(&codex_home, work_dir.path().to_path_buf()).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "local-skill".to_string(),
            description: "from cwd".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
        }]
    );
}

#[tokio::test]
async fn deduplicates_by_path_preferring_first_root() {
    let root = tempfile::tempdir().expect("tempdir");

    let skill_path = write_skill_at(root.path(), "dupe", "dupe-skill", "from repo");

    let outcome = load_skills_from_roots([
        SkillRoot {
            path: root.path().to_path_buf(),
            scope: SkillScope::Repo,
        },
        SkillRoot {
            path: root.path().to_path_buf(),
            scope: SkillScope::User,
        },
    ]);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "dupe-skill".to_string(),
            description: "from repo".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
        }]
    );
}

#[tokio::test]
async fn keeps_duplicate_names_from_repo_and_user() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());

    let user_skill_path = write_skill(&codex_home, "user", "dupe-skill", "from user");
    let repo_skill_path = write_skill_at(
        &repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "repo",
        "dupe-skill",
        "from repo",
    );

    let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: "from repo".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permission_profile: None,
                managed_network_override: None,
                path_to_skills_md: normalized(&repo_skill_path),
                scope: SkillScope::Repo,
            },
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: "from user".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permission_profile: None,
                managed_network_override: None,
                path_to_skills_md: normalized(&user_skill_path),
                scope: SkillScope::User,
            },
        ]
    );
}

#[tokio::test]
async fn keeps_duplicate_names_from_nested_codex_dirs() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());

    let nested_dir = repo_dir.path().join("nested/inner");
    fs::create_dir_all(&nested_dir).unwrap();

    let root_skill_path = write_skill_at(
        &repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "root",
        "dupe-skill",
        "from root",
    );
    let nested_skill_path = write_skill_at(
        &repo_dir
            .path()
            .join("nested")
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "nested",
        "dupe-skill",
        "from nested",
    );

    let cfg = make_config_for_cwd(&codex_home, nested_dir).await;
    let outcome = load_skills_for_test(&cfg);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let root_path = canonicalize_path(&root_skill_path).unwrap_or_else(|_| root_skill_path.clone());
    let nested_path =
        canonicalize_path(&nested_skill_path).unwrap_or_else(|_| nested_skill_path.clone());
    let (first_path, second_path, first_description, second_description) =
        if root_path <= nested_path {
            (root_path, nested_path, "from root", "from nested")
        } else {
            (nested_path, root_path, "from nested", "from root")
        };
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: first_description.to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permission_profile: None,
                managed_network_override: None,
                path_to_skills_md: first_path,
                scope: SkillScope::Repo,
            },
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: second_description.to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                permission_profile: None,
                managed_network_override: None,
                path_to_skills_md: second_path,
                scope: SkillScope::Repo,
            },
        ]
    );
}

#[tokio::test]
async fn repo_skills_search_does_not_escape_repo_root() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let outer_dir = tempfile::tempdir().expect("tempdir");
    let repo_dir = outer_dir.path().join("repo");
    fs::create_dir_all(&repo_dir).unwrap();

    let _skill_path = write_skill_at(
        &outer_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "outer",
        "outer-skill",
        "from outer",
    );
    mark_as_git_repo(&repo_dir);

    let cfg = make_config_for_cwd(&codex_home, repo_dir).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 0);
}

#[tokio::test]
async fn loads_skills_when_cwd_is_file_in_repo() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");
    mark_as_git_repo(repo_dir.path());

    let skill_path = write_skill_at(
        &repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "repo",
        "repo-skill",
        "from repo",
    );
    let file_path = repo_dir.path().join("some-file.txt");
    fs::write(&file_path, "contents").unwrap();

    let cfg = make_config_for_cwd(&codex_home, file_path).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "repo-skill".to_string(),
            description: "from repo".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
        }]
    );
}

#[tokio::test]
async fn non_git_repo_skills_search_does_not_walk_parents() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let outer_dir = tempfile::tempdir().expect("tempdir");
    let nested_dir = outer_dir.path().join("nested/inner");
    fs::create_dir_all(&nested_dir).unwrap();

    write_skill_at(
        &outer_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME),
        "outer",
        "outer-skill",
        "from outer",
    );

    let cfg = make_config_for_cwd(&codex_home, nested_dir).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 0);
}

#[tokio::test]
async fn loads_skills_from_system_cache_when_present() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let work_dir = tempfile::tempdir().expect("tempdir");

    let skill_path = write_system_skill(&codex_home, "system", "system-skill", "from system");

    let cfg = make_config_for_cwd(&codex_home, work_dir.path().to_path_buf()).await;

    let outcome = load_skills_for_test(&cfg);
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "system-skill".to_string(),
            description: "from system".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permission_profile: None,
            managed_network_override: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::System,
        }]
    );
}

#[tokio::test]
async fn skill_roots_include_admin_with_lowest_priority() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cfg = make_config(&codex_home).await;

    let scopes: Vec<SkillScope> = super::skill_roots(&cfg.config_layer_stack, &cfg.cwd, Vec::new())
        .into_iter()
        .map(|root| root.scope)
        .collect();
    let mut expected = vec![SkillScope::User, SkillScope::System];
    if home_dir().is_some() {
        expected.insert(1, SkillScope::User);
    }
    expected.push(SkillScope::Admin);
    assert_eq!(scopes, expected);
}

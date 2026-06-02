use super::*;
use anyhow::Result;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn config_layer(source: ConfigLayerSource, contents: &str) -> ConfigLayerEntry {
    ConfigLayerEntry::new(
        source,
        toml::from_str(contents).expect("config TOML should parse"),
    )
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").current_dir(cwd).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed in {}: {}",
            args.join(" "),
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn init_marketplace_repo(root: &Path, marketplace_name: &str, marker: &str) -> Result<()> {
    run_git(root, &["init"])?;
    run_git(root, &["config", "user.email", "codex@example.com"])?;
    run_git(root, &["config", "user.name", "Codex Tests"])?;
    std::fs::create_dir_all(root.join(".agents/plugins"))?;
    std::fs::write(
        root.join(".agents/plugins/marketplace.json"),
        format!(r#"{{"name":"{marketplace_name}","plugins":[]}}"#),
    )?;
    std::fs::write(root.join("marker.txt"), marker)?;
    run_git(root, &["add", "."])?;
    run_git(root, &["commit", "-m", "initial marketplace"])?;
    Ok(())
}

fn managed_marketplace_stack(source: &Path) -> ConfigLayerStack {
    ConfigLayerStack::new(
        vec![config_layer(
            ConfigLayerSource::EnterpriseManaged {
                id: "cfg_marketplaces".to_string(),
                name: "Managed marketplaces".to_string(),
            },
            &format!(
                r#"
[marketplaces.debug]
source_type = "git"
source = "{}"
"#,
                source.display()
            ),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack should build")
}

#[test]
fn configured_git_marketplaces_merge_mdm_system_cloud_and_user_layers() {
    let temp_dir = TempDir::new().expect("tempdir");
    let stack = ConfigLayerStack::new(
        vec![
            config_layer(
                ConfigLayerSource::Mdm {
                    domain: "com.openai.codex".to_string(),
                    key: "config".to_string(),
                },
                r#"
[marketplaces.mdm]
source_type = "git"
source = "https://example.com/mdm.git"
"#,
            ),
            config_layer(
                ConfigLayerSource::System {
                    file: AbsolutePathBuf::try_from(temp_dir.path().join("etc/config.toml"))
                        .expect("system config path should be absolute"),
                },
                r#"
[marketplaces.system]
source_type = "git"
source = "https://example.com/system.git"
"#,
            ),
            config_layer(
                ConfigLayerSource::EnterpriseManaged {
                    id: "cfg_cloud".to_string(),
                    name: "Cloud marketplaces".to_string(),
                },
                r#"
[marketplaces.cloud]
source_type = "git"
source = "https://example.com/cloud.git"
"#,
            ),
            config_layer(
                ConfigLayerSource::User {
                    file: AbsolutePathBuf::try_from(temp_dir.path().join("config.toml"))
                        .expect("user config path should be absolute"),
                    profile: None,
                },
                r#"
[marketplaces.user]
source_type = "git"
source = "https://example.com/user.git"
"#,
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack should build");

    assert_eq!(
        configured_git_marketplaces(&stack),
        vec![
            ConfiguredGitMarketplace {
                name: "cloud".to_string(),
                source: "https://example.com/cloud.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
                last_revision: None,
                persist_to_user_config: false,
            },
            ConfiguredGitMarketplace {
                name: "mdm".to_string(),
                source: "https://example.com/mdm.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
                last_revision: None,
                persist_to_user_config: false,
            },
            ConfiguredGitMarketplace {
                name: "system".to_string(),
                source: "https://example.com/system.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
                last_revision: None,
                persist_to_user_config: false,
            },
            ConfiguredGitMarketplace {
                name: "user".to_string(),
                source: "https://example.com/user.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
                last_revision: None,
                persist_to_user_config: true,
            },
        ]
    );
}

#[test]
fn managed_marketplace_upgrade_does_not_create_user_override() -> Result<()> {
    let codex_home = TempDir::new()?;
    let first_source = TempDir::new()?;
    let second_source = TempDir::new()?;
    init_marketplace_repo(first_source.path(), "debug", "first")?;
    init_marketplace_repo(second_source.path(), "debug", "second")?;

    let first_stack = managed_marketplace_stack(first_source.path());
    let first_outcome =
        upgrade_configured_git_marketplaces(codex_home.path(), &first_stack, Some("debug"));
    let installed_root = marketplace_install_root(codex_home.path()).join("debug");

    assert_eq!(
        first_outcome,
        ConfiguredMarketplaceUpgradeOutcome {
            selected_marketplaces: vec!["debug".to_string()],
            upgraded_roots: vec![AbsolutePathBuf::try_from(installed_root.clone()).unwrap()],
            errors: Vec::new(),
        }
    );
    assert_eq!(
        std::fs::read_to_string(installed_root.join("marker.txt"))?,
        "first"
    );
    assert!(!codex_home.path().join(CONFIG_TOML_FILE).exists());

    let unchanged_outcome =
        upgrade_configured_git_marketplaces(codex_home.path(), &first_stack, Some("debug"));
    assert_eq!(
        unchanged_outcome,
        ConfiguredMarketplaceUpgradeOutcome {
            selected_marketplaces: vec!["debug".to_string()],
            upgraded_roots: Vec::new(),
            errors: Vec::new(),
        }
    );

    let second_stack = managed_marketplace_stack(second_source.path());
    let second_outcome =
        upgrade_configured_git_marketplaces(codex_home.path(), &second_stack, Some("debug"));
    assert_eq!(
        second_outcome,
        ConfiguredMarketplaceUpgradeOutcome {
            selected_marketplaces: vec!["debug".to_string()],
            upgraded_roots: vec![AbsolutePathBuf::try_from(installed_root.clone()).unwrap()],
            errors: Vec::new(),
        }
    );
    assert_eq!(
        std::fs::read_to_string(installed_root.join("marker.txt"))?,
        "second"
    );
    assert!(!codex_home.path().join(CONFIG_TOML_FILE).exists());

    Ok(())
}

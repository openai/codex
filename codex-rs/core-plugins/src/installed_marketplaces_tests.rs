use super::*;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn config_layer(source: ConfigLayerSource, contents: &str) -> ConfigLayerEntry {
    ConfigLayerEntry::new(
        source,
        toml::from_str(contents).expect("config TOML should parse"),
    )
}

fn write_marketplace_manifest(root: &Path, name: &str) {
    std::fs::create_dir_all(root.join(".agents/plugins"))
        .expect("marketplace root should be created");
    std::fs::write(
        root.join(".agents/plugins/marketplace.json"),
        format!(r#"{{"name":"{name}","plugins":[]}}"#),
    )
    .expect("marketplace manifest should be written");
}

#[test]
fn installed_marketplace_roots_include_mdm_system_and_cloud_config() {
    let codex_home = TempDir::new().expect("codex home");
    let install_root = marketplace_install_root(codex_home.path());
    for name in ["cloud", "mdm", "system"] {
        write_marketplace_manifest(&install_root.join(name), name);
    }

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
                    file: AbsolutePathBuf::try_from(codex_home.path().join("etc/config.toml"))
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
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack should build");

    assert_eq!(
        installed_marketplace_roots_from_layer_stack(&stack, codex_home.path()),
        ["cloud", "mdm", "system"]
            .into_iter()
            .map(|name| AbsolutePathBuf::try_from(install_root.join(name)).unwrap())
            .collect::<Vec<_>>()
    );
}

#[test]
fn installed_marketplace_roots_use_normal_config_precedence() {
    let codex_home = TempDir::new().expect("codex home");
    let system_root = codex_home.path().join("system-marketplace");
    let user_root = codex_home.path().join("user-marketplace");
    write_marketplace_manifest(&system_root, "shared");
    write_marketplace_manifest(&user_root, "shared");

    let stack = ConfigLayerStack::new(
        vec![
            config_layer(
                ConfigLayerSource::System {
                    file: AbsolutePathBuf::try_from(codex_home.path().join("etc/config.toml"))
                        .expect("system config path should be absolute"),
                },
                &format!(
                    r#"
[marketplaces.shared]
source_type = "local"
source = "{}"
"#,
                    system_root.display()
                ),
            ),
            config_layer(
                ConfigLayerSource::User {
                    file: AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
                        .expect("user config path should be absolute"),
                    profile: None,
                },
                &format!(
                    r#"
[marketplaces.shared]
source = "{}"
"#,
                    user_root.display()
                ),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack should build");

    assert_eq!(
        installed_marketplace_roots_from_layer_stack(&stack, codex_home.path()),
        vec![AbsolutePathBuf::try_from(user_root).unwrap()]
    );
}

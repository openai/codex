use super::*;
use crate::config_toml::ConfigToml;
use pretty_assertions::assert_eq;

#[test]
fn computer_use_config_round_trips() {
    let config: ConfigToml = toml::from_str(
        r#"
[computer_use]
default_app_access = "deny"

[computer_use.macos.bundle_ids]
"com.apple.Safari" = "allow"

[computer_use.windows.aumids]
"Microsoft.Paint_8wekyb3d8bbwe!App" = "deny"

[[computer_use.windows.exes]]
publisher_name = "CN=Google LLC"
product_name = "Google Chrome"
binary_name = "chrome.exe"
access = "allow"
"#,
    )
    .expect("computer use config should deserialize");

    let expected = ComputerUseConfigToml {
        default_app_access: Some(AllowDenyRequirementToml::Deny),
        macos: Some(ComputerUseMacosConfigToml {
            bundle_ids: Some(BTreeMap::from([(
                "com.apple.Safari".to_string(),
                AllowDenyRequirementToml::Allow,
            )])),
        }),
        windows: Some(ComputerUseWindowsConfigToml {
            aumids: Some(BTreeMap::from([(
                "Microsoft.Paint_8wekyb3d8bbwe!App".to_string(),
                AllowDenyRequirementToml::Deny,
            )])),
            exes: Some(vec![ComputerUseWindowsExeConfigToml {
                publisher_name: "CN=Google LLC".to_string(),
                product_name: "Google Chrome".to_string(),
                binary_name: Some("chrome.exe".to_string()),
                access: AllowDenyRequirementToml::Allow,
            }]),
        }),
    };
    assert_eq!(config.computer_use, Some(expected.clone()));

    let serialized = toml::to_string(&config).expect("computer use config should serialize");
    let reparsed: ConfigToml =
        toml::from_str(&serialized).expect("serialized computer use config should deserialize");
    assert_eq!(reparsed.computer_use, Some(expected));
}

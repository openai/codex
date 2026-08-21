use super::*;
use crate::AllowDenyRequirementToml;
use crate::config_toml::ConfigToml;
use pretty_assertions::assert_eq;

#[test]
fn browser_use_origin_policies_round_trip() {
    let config: ConfigToml = toml::from_str(
        r#"
[browser_use]
allow_history_access = true

[browser_use.default_origin_policy]
access = "deny"
downloads = "allow"
uploads = "deny"
full_cdp_access = "allow"

[browser_use.origins."https://example.com"]
access = "allow"
downloads = "deny"
uploads = "allow"
full_cdp_access = "deny"
"#,
    )
    .expect("browser use config should deserialize");

    let expected = BrowserUseConfigToml {
        allow_history_access: Some(true),
        default_origin_policy: Some(BrowserUseOriginPolicyConfigToml {
            access: Some(AllowDenyRequirementToml::Deny),
            downloads: Some(AllowDenyRequirementToml::Allow),
            uploads: Some(AllowDenyRequirementToml::Deny),
            full_cdp_access: Some(AllowDenyRequirementToml::Allow),
        }),
        origins: Some(BTreeMap::from([(
            "https://example.com".to_string(),
            BrowserUseOriginPolicyConfigToml {
                access: Some(AllowDenyRequirementToml::Allow),
                downloads: Some(AllowDenyRequirementToml::Deny),
                uploads: Some(AllowDenyRequirementToml::Allow),
                full_cdp_access: Some(AllowDenyRequirementToml::Deny),
            },
        )])),
    };
    assert_eq!(config.browser_use, Some(expected.clone()));

    let serialized = toml::to_string(&config).expect("browser use config should serialize");
    let reparsed: ConfigToml =
        toml::from_str(&serialized).expect("serialized browser use config should deserialize");
    assert_eq!(reparsed.browser_use, Some(expected));
}

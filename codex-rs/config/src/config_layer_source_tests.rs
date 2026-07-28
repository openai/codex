use super::CloudManagedLayer;
use super::ConfigLayerSource;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;

#[test]
fn cloud_managed_layers_have_their_contract_precedence() {
    let baseline = ConfigLayerSource::CloudManaged {
        layer: CloudManagedLayer::Baseline,
        id: "baseline-1".to_string(),
        name: "Baseline".to_string(),
    };
    let mdm = ConfigLayerSource::Mdm {
        domain: "com.openai.codex".to_string(),
        key: "config".to_string(),
    };
    let system = ConfigLayerSource::System {
        file: test_path_buf("/etc/codex/config.toml").abs(),
    };
    let overlay = ConfigLayerSource::CloudManaged {
        layer: CloudManagedLayer::SystemOverlay,
        id: "overlay-1".to_string(),
        name: "System overlay".to_string(),
    };
    let user = ConfigLayerSource::User {
        file: test_path_buf("/home/test/.codex/config.toml").abs(),
        profile: None,
    };

    assert!(baseline < mdm);
    assert!(mdm < system);
    assert!(system < overlay);
    assert!(overlay < user);
}

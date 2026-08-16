use super::host_device_kind_from_profile;
use pretty_assertions::assert_eq;

#[test]
fn recognizes_only_the_exact_mac_mini_hardware_name() {
    let mac_mini_profile = br#"{"SPHardwareDataType":[{"machine_name":"Mac mini"}]}"#;
    let macbook_profile = br#"{"SPHardwareDataType":[{"machine_name":"MacBook Pro"}]}"#;
    let misleading_profile = br#"{"SPHardwareDataType":[{"machine_name":"Not a Mac mini"}]}"#;

    assert_eq!(
        host_device_kind_from_profile(mac_mini_profile).expect("valid Mac mini hardware profile"),
        Some("mac_mini")
    );
    assert_eq!(
        host_device_kind_from_profile(macbook_profile).expect("valid MacBook hardware profile"),
        None
    );
    assert_eq!(
        host_device_kind_from_profile(misleading_profile)
            .expect("valid non-Mac-mini hardware profile"),
        None
    );
}

#[test]
fn ignores_missing_hardware_profiles() {
    assert_eq!(
        host_device_kind_from_profile(br#"{"SPHardwareDataType":[]}"#)
            .expect("valid empty hardware profile"),
        None
    );
}

#[test]
fn rejects_malformed_hardware_profiles_so_they_remain_retryable() {
    assert!(host_device_kind_from_profile(br#"{"machine_name":"Mac mini"}"#).is_err());
    assert!(host_device_kind_from_profile(b"not json").is_err());
}

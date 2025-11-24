#![allow(clippy::extra_unused_type_parameters)]

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn test() {
    assert_send_sync::<semver::BuildMetadata>();
    assert_send_sync::<semver::Comparator>();
    assert_send_sync::<semver::Error>();
    assert_send_sync::<semver::Prerelease>();
    assert_send_sync::<semver::Version>();
    assert_send_sync::<semver::VersionReq>();
    assert_send_sync::<semver::Op>();
}

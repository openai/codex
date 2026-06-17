use codex_app_server_protocol::RuntimeInstallManifestParams;
use pretty_assertions::assert_eq;

use super::validate_manifest;
use crate::errors::invalid_params;

const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn manifest_accepts_archive_for_selected_release() {
    let cases = [
        (
            "latest",
            "https://persistent.oaistatic.com/codex-primary-runtime/2026.06.17/runtime.tar.xz",
        ),
        (
            "latest-alpha",
            "https://oaisidekickupdates.blob.core.windows.net/owl/codex-primary-runtime/alpha/2026.06.17/runtime.zip",
        ),
    ];

    for (release, archive_url) in cases {
        assert_eq!(validate_manifest(release, &manifest(archive_url)), Ok(()));
    }
}

#[test]
fn manifest_rejects_unapproved_archive_locations() {
    let cases = [
        (
            "latest",
            "http://persistent.oaistatic.com/codex-primary-runtime/2026.06.17/runtime.tar.xz",
        ),
        (
            "latest",
            "https://persistent.oaistatic.com.example.com/codex-primary-runtime/2026.06.17/runtime.tar.xz",
        ),
        (
            "latest",
            "https://persistent.oaistatic.com/other/2026.06.17/runtime.tar.xz",
        ),
        (
            "latest",
            "https://persistent.oaistatic.com/codex-primary-runtime/2026.06.17/runtime.tar.xz?redirect=https://example.com",
        ),
        (
            "latest",
            "https://oaisidekickupdates.blob.core.windows.net/owl/codex-primary-runtime/alpha/2026.06.17/runtime.zip",
        ),
        (
            "latest-alpha",
            "https://persistent.oaistatic.com/codex-primary-runtime/2026.06.17/runtime.tar.xz",
        ),
    ];

    for (release, archive_url) in cases {
        let error = validate_manifest(release, &manifest(archive_url))
            .expect_err("archive location should be rejected");

        assert_eq!(
            error,
            invalid_params(format!(
                "runtime manifest archiveUrl for release '{release}' must match the approved OpenAI runtime asset location {}/<version>/<archive>",
                approved_base_url(release)
            ))
        );
    }
}

#[test]
fn manifest_rejects_unsupported_release() {
    let error = validate_manifest(
        "canary",
        &manifest(
            "https://persistent.oaistatic.com/codex-primary-runtime/2026.06.17/runtime.tar.xz",
        ),
    )
    .expect_err("release should be rejected");

    assert_eq!(error, invalid_params("unsupported runtime release: canary"));
}

fn manifest(archive_url: &str) -> RuntimeInstallManifestParams {
    RuntimeInstallManifestParams {
        archive_name: None,
        archive_sha256: SHA256.to_string(),
        archive_size_bytes: None,
        archive_url: archive_url.to_string(),
        bundle_format_version: Some(2),
        bundle_version: Some("2026.06.17".to_string()),
        format: Some("tar.xz".to_string()),
        runtime_root_directory_name: None,
    }
}

fn approved_base_url(release: &str) -> &'static str {
    match release {
        "latest" => "https://persistent.oaistatic.com/codex-primary-runtime",
        "latest-alpha" => {
            "https://oaisidekickupdates.blob.core.windows.net/owl/codex-primary-runtime/alpha"
        }
        _ => panic!("unsupported test release"),
    }
}

use std::path::Path;

use super::is_wsl_windows_drive_path;
use super::should_use_legacy_local_curated_discovery_filter;
use crate::OPENAI_BUNDLED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;

#[test]
fn legacy_local_curated_filter_matches_wsl_windows_backed_curated_checkout() {
    let marketplace_path =
        Path::new("/mnt/c/Users/user/.codex/.tmp/plugins/.agents/plugins/marketplace.json");

    assert!(should_use_legacy_local_curated_discovery_filter(
        OPENAI_CURATED_MARKETPLACE_NAME,
        marketplace_path,
    ));
}

#[test]
fn legacy_local_curated_filter_does_not_match_native_wsl_curated_checkout() {
    let marketplace_path =
        Path::new("/home/user/.codex/.tmp/plugins/.agents/plugins/marketplace.json");

    assert!(!should_use_legacy_local_curated_discovery_filter(
        OPENAI_CURATED_MARKETPLACE_NAME,
        marketplace_path,
    ));
}

#[test]
fn legacy_local_curated_filter_does_not_match_other_wsl_marketplaces() {
    let other_marketplace_path = Path::new(
        "/mnt/c/Users/user/.codex/.tmp/marketplaces/other/.agents/plugins/marketplace.json",
    );
    let local_curated_marketplace_path =
        Path::new("/mnt/c/Users/user/.codex/.tmp/plugins/.agents/plugins/marketplace.json");

    assert!(!should_use_legacy_local_curated_discovery_filter(
        OPENAI_CURATED_MARKETPLACE_NAME,
        other_marketplace_path,
    ));
    assert!(!should_use_legacy_local_curated_discovery_filter(
        OPENAI_BUNDLED_MARKETPLACE_NAME,
        local_curated_marketplace_path,
    ));
}

#[test]
fn wsl_windows_drive_path_matches_only_mnt_drive_paths() {
    assert!(is_wsl_windows_drive_path(Path::new(
        "/mnt/c/Users/user/.codex/.tmp/plugins",
    )));
    assert!(is_wsl_windows_drive_path(Path::new("/mnt/Z/tmp")));
    assert!(!is_wsl_windows_drive_path(Path::new("/home/user/.codex")));
    assert!(!is_wsl_windows_drive_path(Path::new(
        "/mnt/codex/Users/user/.codex",
    )));
    assert!(!is_wsl_windows_drive_path(Path::new(
        "/media/c/Users/user/.codex",
    )));
}

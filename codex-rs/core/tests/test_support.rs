#![allow(clippy::expect_used)]

// Helpers shared by the integration tests.  These are located inside the
// `tests/` tree on purpose so they never become part of the public API surface
// of the `codex-core` crate.

use tempfile::TempDir;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;

/// Returns a default `Config` whose on-disk state is confined to the provided
/// temporary directory. Using a per-test directory keeps tests hermetic and
/// avoids clobbering a developer’s real `~/.codex`.
pub fn load_default_config_for_test(codex_home: &TempDir) -> Config {
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )
    .expect("defaults for test should always succeed")
}

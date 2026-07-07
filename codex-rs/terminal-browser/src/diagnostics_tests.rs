use pretty_assertions::assert_eq;

use super::first_nonempty_line;
use super::has_supported_version;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use super::inspect_installation;

#[test]
fn version_probe_uses_the_first_nonempty_line() {
    assert_eq!(
        first_nonempty_line(b"\n  Carbonyl 0.0.3-codex.1  \nextra\n"),
        Some("Carbonyl 0.0.3-codex.1".to_string())
    );
}

#[test]
fn version_probe_requires_the_exact_pinned_version() {
    assert!(has_supported_version("Carbonyl 0.0.3-codex.1"));
    assert!(!has_supported_version("Carbonyl 0.0.3"));
    assert!(!has_supported_version("Carbonyl 0.0.30"));
    assert!(!has_supported_version("Carbonyl 1.0.0"));
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test]
#[ignore = "opt-in smoke test requiring CODEX_CARBONYL_BINARY and a host sandbox"]
async fn configured_real_carbonyl_passes_the_sandboxed_version_probe() {
    let binary = std::env::var_os("CODEX_CARBONYL_BINARY")
        .map(std::path::PathBuf::from)
        .expect("set CODEX_CARBONYL_BINARY to the real Carbonyl executable");

    let installation = inspect_installation(&binary, &Default::default())
        .await
        .expect("supported installation");

    assert_eq!(installation.version, "Carbonyl 0.0.3-codex.1");
}

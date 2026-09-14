//! Preserve explicit Windows mode priority without persisting feature fallbacks.

use super::*;
use codex_config::ConfigRequirements;
use pretty_assertions::assert_eq;

#[test]
fn configured_mode_takes_priority_without_persisting_feature_fallback() -> std::io::Result<()> {
    for (configured_mode, expected) in [
        (
            None,
            PreparedWindowsSandboxConfig {
                mode: None,
                level: WindowsSandboxLevel::Elevated,
            },
        ),
        (
            Some(WindowsSandboxModeToml::Unelevated),
            PreparedWindowsSandboxConfig {
                mode: Some(WindowsSandboxModeToml::Unelevated),
                level: WindowsSandboxLevel::RestrictedToken,
            },
        ),
    ] {
        let actual = prepare_windows_sandbox_config(
            configured_mode,
            WindowsSandboxLevel::Elevated,
            &mut ConfigRequirements::default().windows_sandbox_mode,
            &mut Vec::new(),
        )?;
        assert_eq!(actual, expected);
    }
    Ok(())
}

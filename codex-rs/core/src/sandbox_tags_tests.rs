use super::sandbox_tag;
use crate::exec::SandboxType;
use crate::protocol::SandboxPolicy;
use crate::safety::get_platform_sandbox;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::NetworkAccess;
use pretty_assertions::assert_eq;

#[test]
fn danger_full_access_is_untagged_even_when_linux_sandbox_defaults_apply() {
    let actual = sandbox_tag(
        &SandboxPolicy::DangerFullAccess,
        WindowsSandboxLevel::Disabled,
    );
    assert_eq!(actual, "none");
}

#[test]
fn external_sandbox_keeps_external_tag_when_linux_sandbox_defaults_apply() {
    let actual = sandbox_tag(
        &SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        },
        WindowsSandboxLevel::Disabled,
    );
    assert_eq!(actual, "external");
}

#[test]
fn default_linux_sandbox_uses_platform_sandbox_tag() {
    let actual = sandbox_tag(
        &SandboxPolicy::new_read_only_policy(),
        WindowsSandboxLevel::Disabled,
    );
    let expected = get_platform_sandbox(false)
        .map(SandboxType::as_metric_tag)
        .unwrap_or("none");
    assert_eq!(actual, expected);
}

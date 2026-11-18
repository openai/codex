#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn legacy_landlock_flag_is_included_when_requested() {
    let command = vec!["/bin/true".to_string()];
    let command_cwd = Path::new("/tmp/link");
    let cwd = Path::new("/tmp");
    let permission_profile = PermissionProfile::read_only();

    let default_bwrap = create_linux_sandbox_command_args_for_permission_profile(
        command.clone(),
        command_cwd,
        &permission_profile,
        cwd,
        LinuxSandboxMode::resolve(
            /*use_legacy_landlock*/ false, /*managed_network*/ None,
        ),
    );
    assert_eq!(
        default_bwrap.contains(&"--use-legacy-landlock".to_string()),
        false
    );

    let legacy_landlock = create_linux_sandbox_command_args_for_permission_profile(
        command,
        command_cwd,
        &permission_profile,
        cwd,
        LinuxSandboxMode::resolve(
            /*use_legacy_landlock*/ true, /*managed_network*/ None,
        ),
    );
    assert_eq!(
        legacy_landlock.contains(&"--use-legacy-landlock".to_string()),
        true
    );
}

#[test]
fn managed_context_takes_precedence_over_legacy_landlock() {
    let command = vec!["/bin/true".to_string()];
    let command_cwd = Path::new("/tmp/link");
    let cwd = Path::new("/tmp");
    let permission_profile = PermissionProfile::read_only();

    let route = ManagedNetworkSandboxContext {
        loopback_ports: vec![8123],
        ..Default::default()
    };
    let args = create_linux_sandbox_command_args_for_permission_profile(
        command,
        command_cwd,
        &permission_profile,
        cwd,
        LinuxSandboxMode::resolve(/*use_legacy_landlock*/ true, Some(&route)),
    );
    let payload = args
        .windows(2)
        .find(|pair| pair[0] == "--managed-network")
        .expect("managed network argument");
    assert_eq!(
        serde_json::from_str::<ManagedNetworkSandboxContext>(&payload[1]).expect("network context"),
        route
    );
    assert_eq!(args.contains(&"--use-legacy-landlock".to_string()), false);
}

#[test]
fn permission_profile_flag_is_included() {
    let command = vec!["/bin/true".to_string()];
    let command_cwd = Path::new("/tmp/link");
    let cwd = Path::new("/tmp");
    let permission_profile = PermissionProfile::read_only();

    let args = create_linux_sandbox_command_args_for_permission_profile(
        command,
        command_cwd,
        &permission_profile,
        cwd,
        LinuxSandboxMode::LegacyLandlock,
    );

    assert_eq!(
        args.windows(2)
            .any(|window| { window[0] == "--permission-profile" && !window[1].is_empty() }),
        true
    );
    assert_eq!(
        args.windows(2)
            .any(|window| window[0] == "--command-cwd" && window[1] == "/tmp/link"),
        true
    );
}

#[cfg(target_os = "linux")]
#[test]
fn wsl1_rejects_linux_bubblewrap_path() {
    let read_only = PermissionProfile::read_only();
    let no_proxy = LinuxSandboxMode::Bubblewrap {
        managed_network: None,
    };
    assert!(matches!(
        no_proxy.ensure_supported_on_wsl1(&read_only),
        Err(SandboxTransformError::Wsl1UnsupportedForBubblewrap)
    ));

    let route = ManagedNetworkSandboxContext::default();
    for use_legacy_landlock in [false, true] {
        let mode = LinuxSandboxMode::resolve(use_legacy_landlock, Some(&route));
        assert!(matches!(
            mode.ensure_supported_on_wsl1(&PermissionProfile::Disabled),
            Err(SandboxTransformError::Wsl1UnsupportedForBubblewrap)
        ));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn wsl1_allows_non_bubblewrap_linux_paths() {
    let no_proxy = LinuxSandboxMode::Bubblewrap {
        managed_network: None,
    };
    assert!(
        no_proxy
            .ensure_supported_on_wsl1(&PermissionProfile::Disabled)
            .is_ok()
    );
    assert!(
        LinuxSandboxMode::LegacyLandlock
            .ensure_supported_on_wsl1(&PermissionProfile::read_only())
            .is_ok()
    );
}

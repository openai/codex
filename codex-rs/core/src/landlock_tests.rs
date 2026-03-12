use super::*;
use pretty_assertions::assert_eq;

#[test]
fn legacy_landlock_flag_is_included_when_requested() {
    let command = vec!["/bin/true".to_string()];
    let cwd = Path::new("/tmp");

    let default_bwrap = create_linux_sandbox_command_args(command.clone(), cwd, false, false);
    assert_eq!(
        default_bwrap.contains(&"--use-legacy-landlock".to_string()),
        false
    );

    let legacy_landlock = create_linux_sandbox_command_args(command, cwd, true, false);
    assert_eq!(
        legacy_landlock.contains(&"--use-legacy-landlock".to_string()),
        true
    );
}

#[test]
fn proxy_flag_is_included_when_requested() {
    let command = vec!["/bin/true".to_string()];
    let cwd = Path::new("/tmp");

    let args = create_linux_sandbox_command_args(command, cwd, true, true);
    assert_eq!(
        args.contains(&"--allow-network-for-proxy".to_string()),
        true
    );
}

#[test]
fn split_policy_flags_are_included() {
    let command = vec!["/bin/true".to_string()];
    let cwd = Path::new("/tmp");
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let file_system_sandbox_policy = FileSystemSandboxPolicy::from(&sandbox_policy);
    let network_sandbox_policy = NetworkSandboxPolicy::from(&sandbox_policy);

    let args = create_linux_sandbox_command_args_for_policies(
        command,
        &sandbox_policy,
        &file_system_sandbox_policy,
        network_sandbox_policy,
        cwd,
        true,
        false,
    );

    assert_eq!(
        args.windows(2)
            .any(|window| { window[0] == "--file-system-sandbox-policy" && !window[1].is_empty() }),
        true
    );
    assert_eq!(
        args.windows(2)
            .any(|window| window[0] == "--network-sandbox-policy" && window[1] == "\"restricted\""),
        true
    );
}

#[test]
fn proxy_network_requires_managed_requirements() {
    assert_eq!(allow_network_for_proxy(false), false);
    assert_eq!(allow_network_for_proxy(true), true);
}

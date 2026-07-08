use super::*;
use pretty_assertions::assert_eq;

#[test]
fn legacy_landlock_flag_is_included_when_requested() {
    let command = vec!["/bin/true".to_string()];
    let command_cwd = Path::new("/tmp/link");
    let cwd = Path::new("/tmp");

    let default_bwrap = create_linux_sandbox_command_args(
        command.clone(),
        command_cwd,
        cwd,
        /*use_legacy_landlock*/ false,
        /*allow_network_for_proxy*/ false,
    );
    assert_eq!(
        default_bwrap.contains(&"--use-legacy-landlock".to_string()),
        false
    );

    let legacy_landlock = create_linux_sandbox_command_args(
        command,
        command_cwd,
        cwd,
        /*use_legacy_landlock*/ true,
        /*allow_network_for_proxy*/ false,
    );
    assert_eq!(
        legacy_landlock.contains(&"--use-legacy-landlock".to_string()),
        true
    );
}

#[test]
fn proxy_flag_takes_precedence_over_legacy_landlock() {
    let command = vec!["/bin/true".to_string()];
    let command_cwd = Path::new("/tmp/link");
    let cwd = Path::new("/tmp");
    let permission_profile = PermissionProfile::read_only();

    let args = create_linux_sandbox_command_args_for_permission_profile(
        command,
        command_cwd,
        &permission_profile,
        cwd,
        /*use_legacy_landlock*/ true,
        /*allow_network_for_proxy*/ true,
    );
    assert_eq!(
        args.contains(&"--allow-network-for-proxy".to_string()),
        true
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
        /*use_legacy_landlock*/ true,
        /*allow_network_for_proxy*/ false,
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

#[test]
fn proxy_network_requires_managed_requirements() {
    assert_eq!(
        allow_network_for_proxy(/*enforce_managed_network*/ false),
        false
    );
    assert_eq!(
        allow_network_for_proxy(/*enforce_managed_network*/ true),
        true
    );
}

#[test]
fn dns_domain_policy_requires_managed_local_binding_and_proxy_ports() {
    for flags in 0_u8..32 {
        let context = ManagedNetworkSandboxContext {
            loopback_ports: (flags & 1 != 0).then_some(43123).into_iter().collect(),
            allow_local_binding: flags & 2 != 0,
            domain_policy: (flags & 4 != 0).then(|| ManagedNetworkDomainPolicy {
                allowed_domains: (flags & 8 != 0)
                    .then(|| "example.com".to_string())
                    .into_iter()
                    .collect(),
                denied_domains: Vec::new(),
            }),
        };
        assert_eq!(
            dns_domain_policy_for_proxy(flags & 16 != 0, Some(&context)).is_some(),
            flags == 31
        );
    }
}

#[test]
fn linux_args_include_dns_domain_policy_immediately_before_separator() {
    let mut args = vec!["--".to_string()];
    insert_dns_policy_args(&mut args, &ManagedNetworkDomainPolicy::default());
    assert_eq!(
        args.join(" "),
        r#"--dns-domain-policy {"allowedDomains":[],"deniedDomains":[]} --"#
    );
}

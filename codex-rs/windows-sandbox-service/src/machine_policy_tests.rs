use anyhow::Result;
use codex_config::ConfigRequirementsToml;
use codex_windows_sandbox::WindowsSandboxProvisioningSettings;
use codex_windows_sandbox::WindowsSandboxProxyListeners;
use pretty_assertions::assert_eq;

use crate::ipc::ProvisioningRequest;

fn validate_requirements(
    settings: &WindowsSandboxProvisioningSettings,
    listeners: WindowsSandboxProxyListeners,
    contents: &str,
) -> Result<()> {
    let requirements: ConfigRequirementsToml = toml::from_str(contents)?;
    super::validate_requirements(settings, &listeners, &requirements)
}

#[test]
fn only_registered_refresh_skips_configuration_loading() {
    for (registered_core, refresh_only) in
        [(true, true), (true, false), (false, false), (false, true)]
    {
        let request = ProvisioningRequest {
            codex_home: std::env::temp_dir(),
            registered_core,
            refresh_only,
            settings: WindowsSandboxProvisioningSettings::default(),
            listeners: WindowsSandboxProxyListeners::default(),
        };
        // An invalid worker token makes any attempt to load config fail closed.
        // Refresh must succeed without starting that runtime; setup must not.
        let result = super::validate_provisioning_request(
            &request.codex_home,
            &request,
            std::ptr::null_mut(),
        );
        assert_eq!(result.is_ok(), registered_core && refresh_only);
        if let Err(error) = result {
            assert!(error.to_string().contains("failed to impersonate"));
        }
    }
}

#[test]
fn unmanaged_policy_allows_default_and_requested_network_settings() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings::default(),
        WindowsSandboxProxyListeners::default(),
        "",
    )?;
    validate_requirements(
        &WindowsSandboxProvisioningSettings {
            proxy_ports: vec![3128, 8080, 9000],
            allow_local_binding: true,
        },
        WindowsSandboxProxyListeners {
            http_ports: vec![3128],
            socks_ports: vec![8080],
        },
        "allowed_sandbox_modes = [\"workspace-write\"]",
    )
}

#[test]
fn elevated_sandbox_is_allowed_when_included() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings::default(),
        WindowsSandboxProxyListeners::default(),
        "[windows]\nallowed_sandbox_implementations = [\"unelevated\", \"elevated\"]",
    )
}

#[test]
fn elevated_sandbox_is_rejected_when_prohibited_or_empty() {
    for implementations in ["[\"unelevated\"]", "[]"] {
        let policy = format!("[windows]\nallowed_sandbox_implementations = {implementations}");
        assert!(
            validate_requirements(
                &WindowsSandboxProvisioningSettings::default(),
                WindowsSandboxProxyListeners::default(),
                &policy,
            )
            .is_err()
        );
    }
}

#[test]
fn forbidden_local_binding_is_rejected() {
    let settings = WindowsSandboxProvisioningSettings {
        proxy_ports: Vec::new(),
        allow_local_binding: true,
    };
    assert!(
        validate_requirements(
            &settings,
            WindowsSandboxProxyListeners::default(),
            "[experimental_network]\nallow_local_binding = false"
        )
        .is_err()
    );
}

#[test]
fn disabled_network_rejects_proxy_ports_and_local_binding() -> Result<()> {
    let policy = "[experimental_network]\nenabled = false";
    validate_requirements(
        &WindowsSandboxProvisioningSettings::default(),
        WindowsSandboxProxyListeners::default(),
        policy,
    )?;
    for settings in [
        WindowsSandboxProvisioningSettings {
            proxy_ports: vec![3128],
            allow_local_binding: false,
        },
        WindowsSandboxProvisioningSettings {
            proxy_ports: Vec::new(),
            allow_local_binding: true,
        },
    ] {
        assert!(
            validate_requirements(&settings, WindowsSandboxProxyListeners::default(), policy)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn enabled_network_allows_unrestricted_proxy_ports() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings {
            proxy_ports: vec![3128, 8080, 8081, 9000],
            allow_local_binding: false,
        },
        WindowsSandboxProxyListeners {
            http_ports: vec![3128, 8080],
            socks_ports: vec![8081],
        },
        "[experimental_network]\nenabled = true",
    )
}

#[test]
fn disabled_local_binding_is_allowed_when_policy_permits_binding() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings::default(),
        WindowsSandboxProxyListeners::default(),
        "[experimental_network]\nallow_local_binding = true",
    )
}

#[test]
fn managed_proxy_ports_allow_only_configured_ports() -> Result<()> {
    let policy = "[experimental_network]\nhttp_port = 3128\nsocks_port = 1080";
    let allowed = WindowsSandboxProvisioningSettings {
        proxy_ports: vec![1080, 3128],
        allow_local_binding: false,
    };
    for listeners in [
        WindowsSandboxProxyListeners {
            http_ports: vec![3128],
            socks_ports: vec![1080],
        },
        WindowsSandboxProxyListeners::default(),
    ] {
        validate_requirements(&allowed, listeners, policy)?;
    }
    let prohibited = WindowsSandboxProvisioningSettings {
        proxy_ports: vec![1080, 3128, 8080],
        allow_local_binding: false,
    };
    for listeners in [
        WindowsSandboxProxyListeners {
            http_ports: vec![3128, 8080],
            socks_ports: vec![1080],
        },
        WindowsSandboxProxyListeners {
            http_ports: vec![3128],
            socks_ports: vec![1080],
        },
        WindowsSandboxProxyListeners::default(),
    ] {
        assert!(validate_requirements(&prohibited, listeners, policy).is_err());
    }
    validate_requirements(
        &WindowsSandboxProvisioningSettings::default(),
        WindowsSandboxProxyListeners::default(),
        policy,
    )
}

#[test]
fn managed_http_port_does_not_restrict_the_unmanaged_socks_listener() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings {
            proxy_ports: vec![3128, 8081],
            allow_local_binding: false,
        },
        WindowsSandboxProxyListeners {
            http_ports: vec![3128],
            socks_ports: vec![8081],
        },
        "[experimental_network]\nhttp_port = 3128",
    )
}

#[test]
fn managed_socks_port_does_not_restrict_the_unmanaged_http_listener() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings {
            proxy_ports: vec![1080, 3128],
            allow_local_binding: false,
        },
        WindowsSandboxProxyListeners {
            http_ports: vec![3128],
            socks_ports: vec![1080],
        },
        "[experimental_network]\nsocks_port = 1080",
    )
}

#[test]
fn managed_proxy_ports_reject_swapped_listener_roles() {
    assert!(
        validate_requirements(
            &WindowsSandboxProvisioningSettings {
                proxy_ports: vec![1080, 3128],
                allow_local_binding: false,
            },
            WindowsSandboxProxyListeners {
                http_ports: vec![1080],
                socks_ports: vec![3128],
            },
            "[experimental_network]\nhttp_port = 3128\nsocks_port = 1080",
        )
        .is_err()
    );
}

#[test]
fn managed_socks_port_rejects_a_mismatched_socks_listener() {
    assert!(
        validate_requirements(
            &WindowsSandboxProvisioningSettings {
                proxy_ports: vec![1080, 3128, 8081],
                allow_local_binding: false,
            },
            WindowsSandboxProxyListeners {
                http_ports: vec![3128],
                socks_ports: vec![1080, 8081],
            },
            "[experimental_network]\nhttp_port = 3128\nsocks_port = 1080",
        )
        .is_err()
    );
}

#[test]
fn managed_socks_port_does_not_require_a_disabled_socks_listener() -> Result<()> {
    validate_requirements(
        &WindowsSandboxProvisioningSettings {
            proxy_ports: vec![3128],
            allow_local_binding: false,
        },
        WindowsSandboxProxyListeners {
            http_ports: vec![3128],
            socks_ports: Vec::new(),
        },
        "[experimental_network]\nhttp_port = 3128\nsocks_port = 1080",
    )
}

#[test]
fn malformed_machine_policy_fails_closed() {
    assert!(
        validate_requirements(
            &WindowsSandboxProvisioningSettings::default(),
            WindowsSandboxProxyListeners::default(),
            "[experimental_network]\nhttp_port = \"invalid\""
        )
        .is_err()
    );
    assert!(
        validate_requirements(
            &WindowsSandboxProvisioningSettings::default(),
            WindowsSandboxProxyListeners::default(),
            "[windows]\nallowed_sandbox_implementations = [\"invalid\"]"
        )
        .is_err()
    );
}

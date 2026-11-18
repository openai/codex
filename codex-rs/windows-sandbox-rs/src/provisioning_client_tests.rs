//! Exercises provisioning client routing and transport failures without contacting the service.

use super::WindowsSandboxProvisioningOutcome;
use super::provision_windows_sandbox_via_service;
use crate::WindowsSandboxProvisioningSettings;
use crate::WindowsSandboxProxyListeners;
use anyhow::Result;
use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
use pretty_assertions::assert_eq;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn workload_identity_selects_helper_fallback_except_for_registered_core() -> Result<()> {
    for variable in [
        OPENAI_FEDERATION_RULE_ID_ENV_VAR,
        OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
    ] {
        for registered_core in ["0", "1"] {
            let output = Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "provisioning_client::tests::workload_identity_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env_remove(OPENAI_FEDERATION_RULE_ID_ENV_VAR)
                .env_remove(OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR)
                .env(variable, "test-only")
                .env("CODEX_WINDOWS_REGISTERED_CORE", registered_core)
                .output()?;
            assert!(
                output.status.success(),
                "{variable}, registered_core={registered_core}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "child process for workload_identity_selects_helper_fallback_except_for_registered_core"]
fn workload_identity_child() -> Result<()> {
    // If the early guard regresses, this fails payload validation before any IPC.
    let invalid_home = PathBuf::from(OsString::from_wide(&[0xd800]));
    let outcome = provision_windows_sandbox_via_service(
        &invalid_home,
        WindowsSandboxProvisioningSettings {
            proxy_ports: Vec::new(),
            allow_local_binding: false,
        },
        WindowsSandboxProxyListeners::default(),
    );
    if crate::registered_core_requested() {
        assert_eq!(
            outcome
                .expect_err("registered Core must not fall back")
                .to_string(),
            "app runtime provisioning service is unavailable; refusing helper fallback"
        );
    } else {
        assert_eq!(outcome?, WindowsSandboxProvisioningOutcome::Unavailable);
    }
    Ok(())
}

#[test]
fn registered_setup_retains_the_transport_cause_for_metrics() {
    use crate::SetupErrorCode;
    use std::io;
    use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;

    for (cause, code) in [
        (
            io::ErrorKind::TimedOut.into(),
            SetupErrorCode::RegisteredServiceTimedOut,
        ),
        (
            io::ErrorKind::UnexpectedEof.into(),
            SetupErrorCode::RegisteredServicePipeClosed,
        ),
        (
            io::Error::from_raw_os_error(ERROR_BROKEN_PIPE as i32),
            SetupErrorCode::RegisteredServicePipeClosed,
        ),
    ] {
        let error = anyhow::Error::new(cause).context("wait for sandbox provisioning response");
        let error = super::service_exchange_error(error, /*registered_core*/ true).unwrap_err();
        assert_eq!(error.to_string(), super::REGISTERED_SERVICE_UNAVAILABLE);
        assert_eq!(crate::extract_setup_failure(&error).unwrap().code, code);
    }
    let timeout = io::Error::from(io::ErrorKind::TimedOut);
    assert!(matches!(
        super::service_exchange_error(timeout.into(), /*registered_core*/ false).unwrap(),
        crate::SandboxProvisioningResponse::Unavailable
    ));
    let denied = io::Error::from(io::ErrorKind::PermissionDenied);
    let error = super::service_exchange_error(denied.into(), /*registered_core*/ true).unwrap_err();
    assert!(crate::extract_setup_failure(&error).is_none());
}

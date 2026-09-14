//! Adapts authenticated service provisioning to the existing setup transaction.
//! The caller owns admission, directory pins, and the setup mutex.

use super::Payload;
use super::SetupMode;
use super::run_payload;
use crate::SETUP_VERSION;
use crate::SetupRuntime;
use crate::logging::log_note;
use crate::sandbox_dir;
use anyhow::Result;
use std::path::Path;

/// Runs the existing ProvisionOnly setup transaction without launching a helper process.
/// The caller authenticates the owner, retains directory protections, and holds the setup mutex
/// on this thread across registration. Firewall setup preserves the caller's COM apartment.
pub fn provision_sandbox_in_process(
    codex_home: &Path,
    real_user: &str,
    settings: crate::WindowsSandboxProvisioningSettings,
    runtime: SetupRuntime,
) -> Result<()> {
    let payload = Payload {
        version: SETUP_VERSION,
        offline_username: crate::setup::OFFLINE_USERNAME.to_string(),
        online_username: crate::setup::ONLINE_USERNAME.to_string(),
        codex_home: codex_home.to_path_buf(),
        command_cwd: codex_home.to_path_buf(),
        read_roots: Vec::new(),
        write_roots: Vec::new(),
        deny_read_paths: Vec::new(),
        deny_write_paths: Vec::new(),
        proxy_ports: settings.proxy_ports,
        allow_local_binding: settings.allow_local_binding,
        otel: codex_otel::global_statsig_metrics_settings(),
        real_user: real_user.to_string(),
        mode: SetupMode::ProvisionOnly,
        runtime,
        refresh_only: false,
    };
    run_payload(&payload)?;
    if let Err(err) = crate::setup_error::clear_setup_error_report(codex_home) {
        log_note(
            &format!("setup orchestrator: failed to clear setup_error.json after success: {err}"),
            Some(&sandbox_dir(codex_home)),
        );
    }
    Ok(())
}

//! Validates provisioning requests against the standard managed configuration layers.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_cloud_config::cloud_config_bundle_loader_for_storage_without_cache;
use codex_config::ConfigLoadOptions;
use codex_config::ConfigRequirementsToml;
use codex_config::WindowsSandboxImplementationToml;
use codex_core::config::bootstrap_auth_config;
use codex_core::config::load_config_toml_with_layer_stack;
use codex_windows_sandbox::WindowsSandboxProvisioningSettings;
use codex_windows_sandbox::WindowsSandboxProxyListeners;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Notify;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::ImpersonateLoggedOnUser;

pub(crate) fn validate_provisioning_settings(
    codex_home: &Path,
    settings: &WindowsSandboxProvisioningSettings,
    listeners: &WindowsSandboxProxyListeners,
    impersonation_token: HANDLE,
) -> Result<()> {
    let impersonation_failure = Arc::new(Notify::new());
    let worker_impersonation_failure = Arc::clone(&impersonation_failure);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .on_thread_start(move || {
            if unsafe { ImpersonateLoggedOnUser(impersonation_token) } == 0 {
                let error = std::io::Error::last_os_error();
                worker_impersonation_failure.notify_one();
                panic!(
                    "failed to impersonate provisioning client on configuration runtime thread: {error}"
                );
            }
        })
        .build()
        .context("start managed configuration runtime")?;
    let requirements = runtime.block_on(async {
        tokio::select! {
            biased;
            () = impersonation_failure.notified() => {
                Err(anyhow::anyhow!("configuration runtime worker failed to impersonate the provisioning client"))
            },
            result = async {
                let mut bootstrap_config = load_config_toml_with_layer_stack(
                    codex_home,
                    /*cwd*/ None,
                    Vec::new(),
                    ConfigLoadOptions::default(),
                )
                .await
                .context("load bootstrap configuration")?;
                // Use the default cloud-policy endpoint unless managed requirements override it.
                bootstrap_config.config_toml.chatgpt_base_url = None;
                let cloud_config_bundle = cloud_config_bundle_loader_for_storage_without_cache(
                    bootstrap_auth_config(codex_home, &bootstrap_config)
                        .context("resolve cloud configuration authentication")?,
                    /*enable_codex_api_key_env*/ false,
                )
                .await
                .context("initialize cloud configuration authentication")?;
                let config = load_config_toml_with_layer_stack(
                    codex_home,
                    /*cwd*/ None,
                    Vec::new(),
                    ConfigLoadOptions {
                        cloud_config_bundle,
                        ..Default::default()
                    },
                )
                .await
                .context("load managed configuration")?;

                Ok::<_, anyhow::Error>(config.config_layer_stack.requirements_toml().clone())
            } => result,
        }
    })?;

    validate_requirements(settings, listeners, &requirements)
        .context("enforce managed provisioning requirements")
}

fn validate_requirements(
    settings: &WindowsSandboxProvisioningSettings,
    listeners: &WindowsSandboxProxyListeners,
    requirements: &ConfigRequirementsToml,
) -> Result<()> {
    if requirements
        .windows
        .as_ref()
        .and_then(|windows| windows.allowed_sandbox_implementations.as_ref())
        .is_some_and(|allowed| !allowed.contains(&WindowsSandboxImplementationToml::Elevated))
    {
        bail!("managed policy does not permit the elevated Windows sandbox");
    }

    let Some(network) = requirements.network.as_ref() else {
        return Ok(());
    };
    if network.enabled == Some(false)
        && (settings.allow_local_binding || !settings.proxy_ports.is_empty())
    {
        bail!("managed policy disables sandbox network access");
    }
    if settings.allow_local_binding && network.allow_local_binding == Some(false) {
        bail!("managed policy does not permit local network binding");
    }
    if let Some(required) = network.http_port
        && let Some(actual) = listeners.http_ports.iter().find(|port| **port != required)
    {
        bail!("managed policy does not permit HTTP proxy port {actual}");
    }
    if let Some(required) = network.socks_port
        && let Some(actual) = listeners.socks_ports.iter().find(|port| **port != required)
    {
        bail!("managed policy does not permit SOCKS proxy port {actual}");
    }
    // Omitting listener identities must not bypass managed port restrictions.
    if network.http_port.is_some() || network.socks_port.is_some() {
        for port in &settings.proxy_ports {
            if !listeners.http_ports.contains(port)
                && !listeners.socks_ports.contains(port)
                && network.http_port != Some(*port)
                && network.socks_port != Some(*port)
            {
                bail!("managed policy does not permit unclassified proxy port {port}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "machine_policy_tests.rs"]
mod tests;

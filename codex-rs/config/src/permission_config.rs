//! Selects legacy or named permissions without changing the source config layers.
//! Legacy settings are merged before translation; managed constraints apply afterward.

use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use crate::config_toml::ConfigToml;
use crate::config_toml::effective_sandbox_mode;
use crate::types::SandboxWorkspaceWrite;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_path_uri::Platform;

#[derive(serde::Deserialize)]
struct PermissionSelectors {
    sandbox_mode: Option<SandboxMode>,
    default_permissions: Option<String>,
}

/// A config-selected permission request, before managed selection and constraints.
/// `None` from the resolver means trust/platform-dependent defaults still apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PermissionConfig<'a> {
    Legacy {
        mode: SandboxMode,
        workspace_write: Option<&'a SandboxWorkspaceWrite>,
    },
    Named {
        id: &'a str,
    },
}

/// Resolves competing selectors at their original layer precedence. Field origins
/// remain available from `layers.origins()`; no synthetic config is inserted.
/// `config` must be the effective merged view of `layers`.
pub fn resolve_permission_config<'a>(
    layers: &ConfigLayerStack,
    config: &'a ConfigToml,
    sandbox_mode_override: Option<SandboxMode>,
) -> Option<PermissionConfig<'a>> {
    let legacy = |mode| PermissionConfig::Legacy {
        mode,
        workspace_write: config.sandbox_workspace_write.as_ref(),
    };
    if let Some(mode) = sandbox_mode_override {
        return Some(legacy(mode));
    }
    let named = config
        .default_permissions
        .as_deref()
        .map(|id| PermissionConfig::Named { id });
    // Explicit session profile selection retains the existing override exception.
    if layers
        .layers_high_to_low()
        .find(|layer| matches!(layer.name, ConfigLayerSource::SessionFlags))
        .and_then(|layer| layer.config.clone().try_into::<PermissionSelectors>().ok())
        .is_some_and(|selectors| selectors.default_permissions.is_some())
    {
        return named;
    }
    let mut selection = None;
    for layer in layers.layers_low_to_high() {
        let Ok(selectors) = layer.config.clone().try_into::<PermissionSelectors>() else {
            continue;
        };
        if selectors.sandbox_mode.is_some() {
            selection = config.sandbox_mode.map(legacy);
        }
        if selectors.default_permissions.is_some() {
            selection = named;
        }
    }
    selection.or_else(|| named.or_else(|| config.sandbox_mode.map(legacy)))
}

/// Translates a merged legacy request using the executor's sandbox availability.
/// This preserves custom roots, networking and temporary-directory exclusions;
/// it does not assign a named-profile identity or apply managed constraints.
pub fn translate_legacy_permissions(
    mode: SandboxMode,
    workspace_write: Option<&SandboxWorkspaceWrite>,
    platform: Platform,
    windows_sandbox_level: WindowsSandboxLevel,
) -> PermissionProfile {
    match effective_sandbox_mode(mode, platform, windows_sandbox_level) {
        SandboxMode::ReadOnly => PermissionProfile::read_only(),
        SandboxMode::WorkspaceWrite => match workspace_write {
            Some(settings) => PermissionProfile::workspace_write_with(
                &settings.writable_roots,
                if settings.network_access {
                    NetworkSandboxPolicy::Enabled
                } else {
                    NetworkSandboxPolicy::Restricted
                },
                settings.exclude_tmpdir_env_var,
                settings.exclude_slash_tmp,
            ),
            None => PermissionProfile::workspace_write(),
        },
        SandboxMode::DangerFullAccess => PermissionProfile::Disabled,
    }
}

#[cfg(test)]
#[path = "permission_config_tests.rs"]
mod tests;

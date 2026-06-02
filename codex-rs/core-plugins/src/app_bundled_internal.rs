use std::str::FromStr;

use codex_plugin::PluginHookSource;
use codex_plugin::PluginHookSourceKind;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

use crate::OPENAI_BUNDLED_MARKETPLACE_NAME;
use crate::loader::load_plugin_hooks;
use crate::manifest::load_plugin_manifest;

const COMPUTER_USE_PLUGIN_NAME: &str = "computer-use";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBundledInternalPlugin {
    pub plugin_id: PluginId,
    pub plugin_root: AbsolutePathBuf,
}

impl FromStr for AppBundledInternalPlugin {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((plugin_id, plugin_root)) = value.split_once('=') else {
            return Err(
                "expected app-bundled internal plugin as <plugin>@<marketplace>=<absolute-path>"
                    .to_string(),
            );
        };
        let plugin_id = PluginId::parse(plugin_id).map_err(|err| err.to_string())?;
        let plugin_root = AbsolutePathBuf::from_absolute_path(plugin_root)
            .map_err(|err| format!("invalid packaged plugin root `{plugin_root}`: {err}"))?;
        Ok(Self {
            plugin_id,
            plugin_root,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBundledInternalHookDiagnosticCode {
    PluginNotAllowlisted,
    PackagedRootUnavailable,
    PackagedManifestUnavailable,
    CachedDeclarationLoadFailed,
    PackagedDeclarationLoadFailed,
    DeclarationMismatch,
}

impl AppBundledInternalHookDiagnosticCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::PluginNotAllowlisted => "plugin_not_allowlisted",
            Self::PackagedRootUnavailable => "packaged_root_unavailable",
            Self::PackagedManifestUnavailable => "packaged_manifest_unavailable",
            Self::CachedDeclarationLoadFailed => "cached_declaration_load_failed",
            Self::PackagedDeclarationLoadFailed => "packaged_declaration_load_failed",
            Self::DeclarationMismatch => "declaration_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBundledInternalHookDiagnostic {
    pub plugin_id: PluginId,
    pub installed_plugin_root: AbsolutePathBuf,
    pub packaged_plugin_root: AbsolutePathBuf,
    pub code: AppBundledInternalHookDiagnosticCode,
    pub details: String,
}

impl AppBundledInternalHookDiagnostic {
    fn warning_message(&self) -> String {
        format!(
            "app-bundled internal plugin hook verification failed for {}: {} ({})",
            self.plugin_id.as_key(),
            self.details,
            self.code.as_str()
        )
    }

    fn record(&self) {
        warn!(
            plugin_id = %self.plugin_id.as_key(),
            installed_plugin_root = %self.installed_plugin_root.display(),
            packaged_plugin_root = %self.packaged_plugin_root.display(),
            code = self.code.as_str(),
            details = %self.details,
            "app-bundled internal plugin hook verification failed"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalPluginHookSource {
    source_relative_path: String,
    hooks: codex_config::HookEventsToml,
}

pub(crate) fn apply_app_bundled_internal_hook_authority(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    plugin_data_root: &AbsolutePathBuf,
    hook_sources: Vec<PluginHookSource>,
    hook_load_warnings: Vec<String>,
    authorities: &[AppBundledInternalPlugin],
) -> (Vec<PluginHookSource>, Vec<String>) {
    let Some(authority) = authorities
        .iter()
        .find(|authority| authority.plugin_id == *plugin_id)
    else {
        return (hook_sources, hook_load_warnings);
    };

    match verify_app_bundled_internal_hooks(
        plugin_id,
        installed_plugin_root,
        plugin_data_root,
        hook_sources,
        hook_load_warnings,
        authority,
    ) {
        Ok(hook_sources) => (hook_sources, Vec::new()),
        Err(diagnostic) => {
            diagnostic.record();
            (Vec::new(), vec![diagnostic.warning_message()])
        }
    }
}

fn verify_app_bundled_internal_hooks(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    plugin_data_root: &AbsolutePathBuf,
    hook_sources: Vec<PluginHookSource>,
    hook_load_warnings: Vec<String>,
    authority: &AppBundledInternalPlugin,
) -> Result<Vec<PluginHookSource>, AppBundledInternalHookDiagnostic> {
    if !is_app_bundled_internal_plugin_allowlisted(plugin_id) {
        return Err(diagnostic(
            plugin_id,
            installed_plugin_root,
            authority,
            AppBundledInternalHookDiagnosticCode::PluginNotAllowlisted,
            "plugin id is not allowlisted for app-bundled internal hooks".to_string(),
        ));
    }

    if !authority.plugin_root.as_path().is_dir() {
        return Err(diagnostic(
            plugin_id,
            installed_plugin_root,
            authority,
            AppBundledInternalHookDiagnosticCode::PackagedRootUnavailable,
            "packaged plugin root does not exist or is not a directory".to_string(),
        ));
    }

    let Some(packaged_manifest) = load_plugin_manifest(authority.plugin_root.as_path()) else {
        return Err(diagnostic(
            plugin_id,
            installed_plugin_root,
            authority,
            AppBundledInternalHookDiagnosticCode::PackagedManifestUnavailable,
            "packaged plugin root is missing a valid plugin manifest".to_string(),
        ));
    };

    if !hook_load_warnings.is_empty() {
        return Err(diagnostic(
            plugin_id,
            installed_plugin_root,
            authority,
            AppBundledInternalHookDiagnosticCode::CachedDeclarationLoadFailed,
            hook_load_warnings.join("; "),
        ));
    }

    let (packaged_sources, packaged_warnings) = load_plugin_hooks(
        &authority.plugin_root,
        plugin_id,
        plugin_data_root,
        &packaged_manifest.paths,
    );
    if !packaged_warnings.is_empty() {
        return Err(diagnostic(
            plugin_id,
            installed_plugin_root,
            authority,
            AppBundledInternalHookDiagnosticCode::PackagedDeclarationLoadFailed,
            packaged_warnings.join("; "),
        ));
    }

    if canonical_hook_sources(&hook_sources) != canonical_hook_sources(&packaged_sources) {
        return Err(diagnostic(
            plugin_id,
            installed_plugin_root,
            authority,
            AppBundledInternalHookDiagnosticCode::DeclarationMismatch,
            "cached plugin hook declarations do not match the app-packaged declarations"
                .to_string(),
        ));
    }

    Ok(hook_sources
        .into_iter()
        .map(|mut source| {
            source.kind = PluginHookSourceKind::AppBundledInternal;
            source
        })
        .collect())
}

fn is_app_bundled_internal_plugin_allowlisted(plugin_id: &PluginId) -> bool {
    plugin_id.plugin_name == COMPUTER_USE_PLUGIN_NAME
        && plugin_id.marketplace_name == OPENAI_BUNDLED_MARKETPLACE_NAME
}

fn canonical_hook_sources(sources: &[PluginHookSource]) -> Vec<CanonicalPluginHookSource> {
    sources
        .iter()
        .map(|source| CanonicalPluginHookSource {
            source_relative_path: source.source_relative_path.clone(),
            hooks: source.hooks.clone(),
        })
        .collect()
}

fn diagnostic(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    authority: &AppBundledInternalPlugin,
    code: AppBundledInternalHookDiagnosticCode,
    details: String,
) -> AppBundledInternalHookDiagnostic {
    AppBundledInternalHookDiagnostic {
        plugin_id: plugin_id.clone(),
        installed_plugin_root: installed_plugin_root.clone(),
        packaged_plugin_root: authority.plugin_root.clone(),
        code,
        details,
    }
}

#[cfg(test)]
#[path = "app_bundled_internal_tests.rs"]
mod tests;

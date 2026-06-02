use std::fs;
use std::io;
use std::io::Write;
use std::str::FromStr;

use codex_plugin::PluginHookSource;
use codex_plugin::PluginHookSourceKind;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use tracing::warn;
use walkdir::WalkDir;

use crate::loader::load_plugin_hooks;
use crate::manifest::PluginManifestPaths;
use crate::manifest::load_plugin_manifest;

pub(crate) const APP_BUNDLED_INTERNAL_HOOK_RECEIPT_PATH: &str =
    ".codex-plugin/app-bundled-internal-hooks.json";
const APP_BUNDLED_INTERNAL_HOOK_RECEIPT_SCHEMA_VERSION: u32 = 1;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AppBundledInternalHookReceipt {
    schema_version: u32,
    plugin_id: String,
    status: AppBundledInternalHookReceiptStatus,
    plugin_tree_sha256: String,
    hook_sources: Vec<CanonicalPluginHookSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AppBundledInternalHookReceiptStatus {
    Verified,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    if let Some(authority) = authority_for_plugin(plugin_id, authorities) {
        return match verify_packaged_hook_declarations(
            plugin_id,
            installed_plugin_root,
            plugin_data_root,
            &hook_sources,
            &hook_load_warnings,
            authority,
        ) {
            Ok(plugin_tree_sha256) => {
                if let Err(err) = write_verified_receipt(
                    plugin_id,
                    installed_plugin_root,
                    &plugin_tree_sha256,
                    &hook_sources,
                ) {
                    write_rejected_receipt_best_effort(plugin_id, installed_plugin_root);
                    warn!(
                        plugin_id = %plugin_id.as_key(),
                        installed_plugin_root = %installed_plugin_root.display(),
                        error = %err,
                        "failed to persist app-bundled internal hook receipt"
                    );
                    return (Vec::new(), Vec::new());
                }
                (mark_internal(hook_sources), Vec::new())
            }
            Err(err) => {
                write_rejected_receipt_best_effort(plugin_id, installed_plugin_root);
                warn!(
                    plugin_id = %plugin_id.as_key(),
                    installed_plugin_root = %installed_plugin_root.display(),
                    packaged_plugin_root = %authority.plugin_root.display(),
                    error = %err,
                    "app-bundled internal plugin hook verification failed"
                );
                (Vec::new(), Vec::new())
            }
        };
    }

    match verify_receipt(plugin_id, installed_plugin_root, &hook_sources) {
        Ok(ReceiptVerification::Missing) => (hook_sources, hook_load_warnings),
        Ok(ReceiptVerification::Valid) => (mark_internal(hook_sources), Vec::new()),
        Err(err) => {
            warn!(
                plugin_id = %plugin_id.as_key(),
                installed_plugin_root = %installed_plugin_root.display(),
                error = %err,
                "app-bundled internal hook receipt verification failed"
            );
            (Vec::new(), Vec::new())
        }
    }
}

pub(crate) fn refresh_app_bundled_internal_hook_receipt(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    plugin_data_root: &AbsolutePathBuf,
    authorities: &[AppBundledInternalPlugin],
) -> Result<(), String> {
    let Some(authority) = authority_for_plugin(plugin_id, authorities) else {
        return Ok(());
    };
    let Some(manifest) = load_plugin_manifest(installed_plugin_root.as_path()) else {
        let err = "cannot persist app-bundled internal hook receipt without an installed manifest"
            .to_string();
        write_rejected_receipt_best_effort(plugin_id, installed_plugin_root);
        return Err(err);
    };
    let (hook_sources, hook_load_warnings) = load_plugin_hooks(
        installed_plugin_root,
        plugin_id,
        plugin_data_root,
        &manifest.paths,
    );
    let result = verify_packaged_hook_declarations(
        plugin_id,
        installed_plugin_root,
        plugin_data_root,
        &hook_sources,
        &hook_load_warnings,
        authority,
    )
    .and_then(|plugin_tree_sha256| {
        write_verified_receipt(
            plugin_id,
            installed_plugin_root,
            &plugin_tree_sha256,
            &hook_sources,
        )
        .map_err(|err| err.to_string())
    });
    if let Err(err) = &result {
        write_rejected_receipt_best_effort(plugin_id, installed_plugin_root);
        warn!(
            plugin_id = %plugin_id.as_key(),
            installed_plugin_root = %installed_plugin_root.display(),
            packaged_plugin_root = %authority.plugin_root.display(),
            error = %err,
            "failed to persist app-bundled internal hook receipt after plugin install"
        );
    }
    result
}

pub(crate) fn should_hide_app_bundled_internal_hook_declarations(
    plugin_id: &PluginId,
    plugin_root: &AbsolutePathBuf,
    plugin_data_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
    authorities: &[AppBundledInternalPlugin],
) -> bool {
    if authority_for_plugin(plugin_id, authorities).is_some() {
        return true;
    }

    let (hook_sources, hook_load_warnings) =
        load_plugin_hooks(plugin_root, plugin_id, plugin_data_root, manifest_paths);
    hook_load_warnings.is_empty()
        && matches!(
            verify_receipt(plugin_id, plugin_root, &hook_sources),
            Ok(ReceiptVerification::Valid)
        )
}

pub(crate) fn remove_app_bundled_internal_hook_receipt(
    installed_plugin_root: &AbsolutePathBuf,
) -> io::Result<()> {
    let receipt_path = installed_plugin_root.join(APP_BUNDLED_INTERNAL_HOOK_RECEIPT_PATH);
    match fs::remove_file(receipt_path.as_path()) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn authority_for_plugin<'a>(
    plugin_id: &PluginId,
    authorities: &'a [AppBundledInternalPlugin],
) -> Option<&'a AppBundledInternalPlugin> {
    authorities
        .iter()
        .find(|authority| authority.plugin_id == *plugin_id)
}

fn verify_packaged_hook_declarations(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    plugin_data_root: &AbsolutePathBuf,
    installed_sources: &[PluginHookSource],
    installed_warnings: &[String],
    authority: &AppBundledInternalPlugin,
) -> Result<String, String> {
    if !authority.plugin_root.as_path().is_dir() {
        return Err("packaged plugin root does not exist or is not a directory".to_string());
    }
    let Some(packaged_manifest) = load_plugin_manifest(authority.plugin_root.as_path()) else {
        return Err("packaged plugin root is missing a valid plugin manifest".to_string());
    };
    if packaged_manifest.name != plugin_id.plugin_name {
        return Err(format!(
            "packaged plugin manifest name `{}` does not match `{}`",
            packaged_manifest.name, plugin_id.plugin_name
        ));
    }
    if !installed_warnings.is_empty() {
        return Err(format!(
            "installed declarations failed to load: {}",
            installed_warnings.join("; ")
        ));
    }

    let (packaged_sources, packaged_warnings) = load_plugin_hooks(
        &authority.plugin_root,
        plugin_id,
        plugin_data_root,
        &packaged_manifest.paths,
    );
    if !packaged_warnings.is_empty() {
        return Err(format!(
            "packaged declarations failed to load: {}",
            packaged_warnings.join("; ")
        ));
    }
    if canonical_hook_sources(installed_sources) != canonical_hook_sources(&packaged_sources) {
        return Err(format!(
            "installed hook declarations in {} do not match the app-packaged declarations",
            installed_plugin_root.display()
        ));
    }
    let installed_tree_sha256 = plugin_tree_sha256(installed_plugin_root)?;
    let packaged_tree_sha256 = plugin_tree_sha256(&authority.plugin_root)?;
    if installed_tree_sha256 != packaged_tree_sha256 {
        return Err(format!(
            "installed plugin tree in {} does not match the app-packaged plugin tree",
            installed_plugin_root.display()
        ));
    }
    Ok(installed_tree_sha256)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiptVerification {
    Missing,
    Valid,
}

fn verify_receipt(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    hook_sources: &[PluginHookSource],
) -> Result<ReceiptVerification, String> {
    let receipt_path = installed_plugin_root.join(APP_BUNDLED_INTERNAL_HOOK_RECEIPT_PATH);
    let contents = match fs::read_to_string(receipt_path.as_path()) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(ReceiptVerification::Missing);
        }
        Err(err) => return Err(format!("failed to read receipt: {err}")),
    };
    let receipt: AppBundledInternalHookReceipt =
        serde_json::from_str(&contents).map_err(|err| format!("failed to parse receipt: {err}"))?;
    if receipt.schema_version != APP_BUNDLED_INTERNAL_HOOK_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported receipt schema version {}",
            receipt.schema_version
        ));
    }
    if receipt.plugin_id != plugin_id.as_key() {
        return Err(format!(
            "receipt plugin id `{}` does not match `{}`",
            receipt.plugin_id,
            plugin_id.as_key()
        ));
    }
    if receipt.status == AppBundledInternalHookReceiptStatus::Rejected {
        return Err("app-bundled internal hook receipt records a rejected install".to_string());
    }
    let plugin_tree_sha256 = plugin_tree_sha256(installed_plugin_root)?;
    if receipt.plugin_tree_sha256 != plugin_tree_sha256 {
        return Err("installed plugin tree no longer matches the receipt".to_string());
    }
    if receipt.hook_sources != canonical_hook_sources(hook_sources) {
        return Err("installed hook declarations no longer match the receipt".to_string());
    }
    Ok(ReceiptVerification::Valid)
}

fn write_verified_receipt(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
    plugin_tree_sha256: &str,
    hook_sources: &[PluginHookSource],
) -> io::Result<()> {
    write_receipt(
        installed_plugin_root,
        &AppBundledInternalHookReceipt {
            schema_version: APP_BUNDLED_INTERNAL_HOOK_RECEIPT_SCHEMA_VERSION,
            plugin_id: plugin_id.as_key(),
            status: AppBundledInternalHookReceiptStatus::Verified,
            plugin_tree_sha256: plugin_tree_sha256.to_string(),
            hook_sources: canonical_hook_sources(hook_sources),
        },
    )
}

fn write_rejected_receipt_best_effort(
    plugin_id: &PluginId,
    installed_plugin_root: &AbsolutePathBuf,
) {
    let receipt = AppBundledInternalHookReceipt {
        schema_version: APP_BUNDLED_INTERNAL_HOOK_RECEIPT_SCHEMA_VERSION,
        plugin_id: plugin_id.as_key(),
        status: AppBundledInternalHookReceiptStatus::Rejected,
        plugin_tree_sha256: String::new(),
        hook_sources: Vec::new(),
    };
    if let Err(err) = write_receipt(installed_plugin_root, &receipt) {
        warn!(
            plugin_id = %plugin_id.as_key(),
            installed_plugin_root = %installed_plugin_root.display(),
            error = %err,
            "failed to persist rejected app-bundled internal hook receipt"
        );
    }
}

fn write_receipt(
    installed_plugin_root: &AbsolutePathBuf,
    receipt: &AppBundledInternalHookReceipt,
) -> io::Result<()> {
    let mut contents = serde_json::to_string_pretty(&receipt).map_err(io::Error::other)?;
    contents.push('\n');
    let receipt_path = installed_plugin_root.join(APP_BUNDLED_INTERNAL_HOOK_RECEIPT_PATH);
    let parent = receipt_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("receipt path {} has no parent", receipt_path.display()),
        )
    })?;
    fs::create_dir_all(&parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&parent)?;
    temporary.write_all(contents.as_bytes())?;
    temporary
        .persist(receipt_path.as_path())
        .map_err(|err| err.error)?;
    Ok(())
}

fn mark_internal(mut hook_sources: Vec<PluginHookSource>) -> Vec<PluginHookSource> {
    for source in &mut hook_sources {
        source.kind = PluginHookSourceKind::AppBundledInternal;
    }
    hook_sources
}

fn plugin_tree_sha256(plugin_root: &AbsolutePathBuf) -> Result<String, String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(plugin_root.as_path()) {
        let entry = entry.map_err(|err| format!("failed to walk plugin tree: {err}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative_path = entry
            .path()
            .strip_prefix(plugin_root.as_path())
            .map_err(|err| format!("failed to relativize plugin path: {err}"))?;
        if relative_path == std::path::Path::new(APP_BUNDLED_INTERNAL_HOOK_RECEIPT_PATH) {
            continue;
        }
        files.push(relative_path.to_path_buf());
    }
    files.sort();

    let mut hasher = Sha256::new();
    for relative_path in files {
        let relative_path_string = relative_path
            .to_str()
            .ok_or_else(|| {
                format!(
                    "app-bundled internal plugin path is not UTF-8: {}",
                    relative_path.display()
                )
            })?
            .replace('\\', "/");
        hasher.update((relative_path_string.len() as u64).to_be_bytes());
        hasher.update(relative_path_string.as_bytes());
        let path = plugin_root.join(&relative_path);
        let contents = fs::read(path.as_path())
            .map_err(|err| format!("failed to read plugin file `{relative_path_string}`: {err}"))?;
        hasher.update((contents.len() as u64).to_be_bytes());
        hasher.update(contents);
    }
    let digest = hasher.finalize();
    Ok(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
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

#[cfg(test)]
#[path = "app_bundled_internal_tests.rs"]
mod tests;

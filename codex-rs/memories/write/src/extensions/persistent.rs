use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const MANIFEST_VERSION: u32 = 1;
const MANIFEST_FILENAME: &str = "manifest.json";
const INSTRUCTIONS_FILENAME: &str = "instructions.md";
const RESOURCES_SUBDIR: &str = "resources";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistentMemoryExtensionResource {
    /// Stable source identity used to detect updates and removals.
    pub source_id: String,
    /// Flat Markdown filename below the extension's `resources/` directory.
    pub target_file_name: String,
    /// Hash of the original source content, before any metadata envelope is added.
    pub content_sha256: String,
    /// Complete staged resource content, including any source metadata envelope.
    pub content: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersistentMemoryExtensionSyncOutcome {
    pub written: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
    pub unchanged: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
struct PersistentExtensionManifest {
    version: u32,
    resources: BTreeMap<String, PersistentExtensionManifestEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistentExtensionManifestEntry {
    target_file_name: String,
    content_sha256: String,
}

pub async fn persistent_extension_needs_sync(
    memory_root: &Path,
    extension_name: &str,
    instructions: &str,
    resources: &[PersistentMemoryExtensionResource],
) -> io::Result<bool> {
    validate_inputs(extension_name, resources)?;
    let extension_root = extension_root(memory_root, extension_name);
    if !tokio::fs::try_exists(&extension_root).await? {
        return Ok(!resources.is_empty());
    }

    if read_optional_string(&extension_root.join(INSTRUCTIONS_FILENAME)).await?
        != Some(instructions.to_string())
    {
        return Ok(true);
    }

    let manifest = match read_manifest(&extension_root).await {
        Ok(Some(manifest)) if manifest.version == MANIFEST_VERSION => manifest,
        Ok(_) => return Ok(true),
        Err(err) if err.kind() == io::ErrorKind::InvalidData => return Ok(true),
        Err(err) => return Err(err),
    };
    let desired_manifest = desired_manifest(resources);
    if manifest != desired_manifest {
        return Ok(true);
    }

    let resources_root = extension_root.join(RESOURCES_SUBDIR);
    for resource in resources {
        if read_optional_string(&resources_root.join(&resource.target_file_name)).await?
            != Some(resource.content.clone())
        {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Synchronizes durable, non-expiring resources for one memory extension.
///
/// The caller must hold the Phase 2 workspace-mutation claim before invoking this function. Only
/// files listed in this extension's prior manifest are eligible for removal; unrelated resources
/// are preserved.
pub async fn sync_persistent_extension_resources(
    memory_root: &Path,
    extension_name: &str,
    instructions: &str,
    resources: &[PersistentMemoryExtensionResource],
) -> io::Result<PersistentMemoryExtensionSyncOutcome> {
    validate_inputs(extension_name, resources)?;
    let extension_root = extension_root(memory_root, extension_name);
    let resources_root = extension_root.join(RESOURCES_SUBDIR);
    tokio::fs::create_dir_all(&resources_root).await?;

    write_if_changed(
        &extension_root.join(INSTRUCTIONS_FILENAME),
        instructions.as_bytes(),
    )
    .await?;

    let previous_manifest = match read_manifest(&extension_root).await {
        Ok(manifest) => manifest.unwrap_or_default(),
        Err(err) if err.kind() == io::ErrorKind::InvalidData => {
            PersistentExtensionManifest::default()
        }
        Err(err) => return Err(err),
    };
    let desired_manifest = desired_manifest(resources);
    let desired_target_names = resources
        .iter()
        .map(|resource| resource.target_file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut outcome = PersistentMemoryExtensionSyncOutcome::default();

    for resource in resources {
        let target_path = resources_root.join(&resource.target_file_name);
        if write_if_changed(&target_path, resource.content.as_bytes()).await? {
            outcome.written.push(target_path);
        } else {
            outcome.unchanged = outcome.unchanged.saturating_add(1);
        }
    }

    for previous_entry in previous_manifest.resources.values() {
        if desired_target_names.contains(previous_entry.target_file_name.as_str()) {
            continue;
        }
        let target_path = resources_root.join(&previous_entry.target_file_name);
        match tokio::fs::remove_file(&target_path).await {
            Ok(()) => outcome.removed.push(target_path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }

    let mut rendered_manifest = serde_json::to_string_pretty(&desired_manifest)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    rendered_manifest.push('\n');
    write_if_changed(
        &extension_root.join(MANIFEST_FILENAME),
        rendered_manifest.as_bytes(),
    )
    .await?;

    Ok(outcome)
}

fn validate_inputs(
    extension_name: &str,
    resources: &[PersistentMemoryExtensionResource],
) -> io::Result<()> {
    validate_flat_name(extension_name, "extension name")?;
    let mut source_ids = BTreeSet::new();
    let mut target_file_names = BTreeSet::new();
    for resource in resources {
        if resource.source_id.is_empty() {
            return Err(invalid_data("resource source_id cannot be empty"));
        }
        validate_flat_name(&resource.target_file_name, "resource target filename")?;
        if !resource.target_file_name.ends_with(".md") {
            return Err(invalid_data("resource target filename must end with .md"));
        }
        if !source_ids.insert(resource.source_id.as_str()) {
            return Err(invalid_data("resource source_id values must be unique"));
        }
        if !target_file_names.insert(resource.target_file_name.as_str()) {
            return Err(invalid_data(
                "resource target filename values must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_flat_name(value: &str, label: &str) -> io::Result<()> {
    let path = Path::new(value);
    if value.is_empty() || path.file_name().and_then(|name| name.to_str()) != Some(value) {
        return Err(invalid_data(format!("{label} must be one path component")));
    }
    Ok(())
}

fn extension_root(memory_root: &Path, extension_name: &str) -> PathBuf {
    crate::memory_extensions_root(memory_root).join(extension_name)
}

fn desired_manifest(
    resources: &[PersistentMemoryExtensionResource],
) -> PersistentExtensionManifest {
    PersistentExtensionManifest {
        version: MANIFEST_VERSION,
        resources: resources
            .iter()
            .map(|resource| {
                (
                    resource.source_id.clone(),
                    PersistentExtensionManifestEntry {
                        target_file_name: resource.target_file_name.clone(),
                        content_sha256: resource.content_sha256.clone(),
                    },
                )
            })
            .collect(),
    }
}

async fn read_manifest(extension_root: &Path) -> io::Result<Option<PersistentExtensionManifest>> {
    let Some(raw) = read_optional_string(&extension_root.join(MANIFEST_FILENAME)).await? else {
        return Ok(None);
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

async fn read_optional_string(path: &Path) -> io::Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

async fn write_if_changed(path: &Path, content: &[u8]) -> io::Result<bool> {
    match tokio::fs::read(path).await {
        Ok(existing) if existing == content => return Ok(false),
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    tokio::fs::write(path, content).await?;
    Ok(true)
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "persistent_tests.rs"]
mod tests;

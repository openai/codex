use crate::store::PluginStoreError;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;
pub(crate) const INSTALL_PROVENANCE_FILE: &str = ".codex-plugin-install.json";
const INSTALL_PROVENANCE_SCHEMA_VERSION: u8 = 1;
#[derive(Debug, Deserialize, Serialize)]
struct PluginInstallProvenance {
    schema_version: u8,
    content_sha256: String,
}
enum TreeEntry {
    Directory,
    File,
    VirtualFile(Vec<u8>),
}
pub(crate) fn fingerprint_plugin_tree(
    root: &Path,
    manifest_override: Option<&str>,
) -> Result<String, PluginStoreError> {
    let traversal_root = fs::canonicalize(root)
        .map_err(|error| PluginStoreError::io("failed to resolve plugin source", error))?;
    let mut entries = BTreeMap::new();
    for entry in WalkDir::new(&traversal_root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| entry.depth() != 1 || entry.file_name() != ".git")
        .skip(1)
    {
        let entry = entry.map_err(|error| {
            PluginStoreError::io(
                "failed to enumerate plugin source",
                std::io::Error::other(error),
            )
        })?;
        let relative = entry
            .path()
            .strip_prefix(&traversal_root)
            .map_err(|error| {
                PluginStoreError::Invalid(format!("failed to fingerprint plugin path: {error}"))
            })?;
        let tree_entry = if entry.file_type().is_dir() {
            TreeEntry::Directory
        } else if entry.file_type().is_file() {
            TreeEntry::File
        } else {
            continue;
        };
        entries.insert(relative.to_path_buf(), tree_entry);
    }
    if let Some(manifest) = manifest_override {
        entries
            .entry(PathBuf::from(".codex-plugin"))
            .or_insert(TreeEntry::Directory);
        entries.insert(
            PathBuf::from(".codex-plugin").join("plugin.json"),
            TreeEntry::VirtualFile(manifest.as_bytes().to_vec()),
        );
    }
    let mut hasher = Sha256::new();
    for (path, entry) in entries {
        hasher.update([if matches!(entry, TreeEntry::Directory) {
            b'd'
        } else {
            b'f'
        }]);
        hash_contents(&mut hasher, path.as_os_str().as_encoded_bytes());
        match entry {
            TreeEntry::Directory => {}
            TreeEntry::File => hash_file(&mut hasher, &traversal_root.join(path))?,
            TreeEntry::VirtualFile(contents) => hash_contents(&mut hasher, &contents),
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub(crate) fn read_install_fingerprint(
    plugin_base_root: &Path,
) -> Result<Option<String>, PluginStoreError> {
    let path = plugin_base_root.join(INSTALL_PROVENANCE_FILE);
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(PluginStoreError::io(
                "failed to read plugin install provenance",
                error,
            ));
        }
    };
    let Ok(provenance) = serde_json::from_str::<PluginInstallProvenance>(&contents) else {
        return Ok(None);
    };
    let fingerprint = provenance.content_sha256;
    Ok(
        (provenance.schema_version == INSTALL_PROVENANCE_SCHEMA_VERSION
            && fingerprint.len() == 64
            && fingerprint
                .chars()
                .all(|character| character.is_ascii_hexdigit()))
        .then(|| fingerprint.to_ascii_lowercase()),
    )
}
pub(crate) fn write_install_fingerprint(
    plugin_base_root: &Path,
    content_sha256: &str,
) -> Result<(), PluginStoreError> {
    let provenance = PluginInstallProvenance {
        schema_version: INSTALL_PROVENANCE_SCHEMA_VERSION,
        content_sha256: content_sha256.to_string(),
    };
    let mut temporary = tempfile::NamedTempFile::new_in(plugin_base_root).map_err(|error| {
        PluginStoreError::io(
            "failed to create temporary plugin install provenance",
            error,
        )
    })?;
    serde_json::to_writer_pretty(&mut temporary, &provenance).map_err(|error| {
        PluginStoreError::Invalid(format!("failed to serialize plugin provenance: {error}"))
    })?;
    io::Write::write_all(&mut temporary, b"\n")
        .map_err(|error| PluginStoreError::io("failed to write plugin provenance", error))?;
    io::Write::flush(temporary.as_file_mut())
        .map_err(|error| PluginStoreError::io("failed to flush plugin provenance", error))?;
    temporary
        .persist(plugin_base_root.join(INSTALL_PROVENANCE_FILE))
        .map_err(|error| {
            PluginStoreError::io("failed to persist plugin install provenance", error.error)
        })?;
    Ok(())
}
fn hash_file(hasher: &mut Sha256, path: &Path) -> Result<(), PluginStoreError> {
    let mut file = fs::File::open(path)
        .map_err(|error| PluginStoreError::io("failed to open plugin file", error))?;
    let length = file
        .metadata()
        .map_err(|error| PluginStoreError::io("failed to inspect plugin file", error))?
        .len();
    hasher.update(length.to_le_bytes());
    io::copy(&mut file, hasher)
        .map_err(|error| PluginStoreError::io("failed to read plugin file", error))?;
    Ok(())
}
fn hash_contents(hasher: &mut Sha256, contents: &[u8]) {
    hasher.update((contents.len() as u64).to_le_bytes());
    hasher.update(contents);
}

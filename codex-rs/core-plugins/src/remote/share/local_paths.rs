use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

const PLUGIN_SHARE_LOCAL_PATHS_FILE: &str = ".tmp/plugin-share-local-paths-v1.json";
static PLUGIN_SHARE_LOCAL_PATHS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginShareLocalPaths {
    #[serde(default)]
    local_plugin_paths_by_remote_plugin_id: BTreeMap<String, AbsolutePathBuf>,
}

pub(crate) fn load_plugin_share_local_paths(
    codex_home: &Path,
) -> io::Result<BTreeMap<String, AbsolutePathBuf>> {
    let _guard = lock_plugin_share_local_paths()?;
    read_plugin_share_local_paths(codex_home)
}

pub(crate) fn record_plugin_share_local_path(
    codex_home: &Path,
    remote_plugin_id: &str,
    plugin_path: AbsolutePathBuf,
) -> io::Result<()> {
    let _guard = lock_plugin_share_local_paths()?;
    let mut mapping = read_plugin_share_local_paths_for_update(codex_home)?;
    mapping.insert(remote_plugin_id.to_string(), plugin_path);
    write_plugin_share_local_paths(codex_home, mapping)
}

pub(crate) fn remove_plugin_share_local_path(
    codex_home: &Path,
    remote_plugin_id: &str,
) -> io::Result<()> {
    let _guard = lock_plugin_share_local_paths()?;
    let mut mapping = read_plugin_share_local_paths_for_update(codex_home)?;
    mapping.remove(remote_plugin_id);
    write_plugin_share_local_paths(codex_home, mapping)
}

fn lock_plugin_share_local_paths() -> io::Result<std::sync::MutexGuard<'static, ()>> {
    PLUGIN_SHARE_LOCAL_PATHS_LOCK
        .lock()
        .map_err(|err| io::Error::other(format!("plugin share local path lock poisoned: {err}")))
}

fn read_plugin_share_local_paths(
    codex_home: &Path,
) -> io::Result<BTreeMap<String, AbsolutePathBuf>> {
    let path = plugin_share_local_paths_path(codex_home);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(err),
    };

    let mapping = serde_json::from_str::<PluginShareLocalPaths>(&contents).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "failed to parse plugin share local path mapping {}: {err}",
                path.display()
            ),
        )
    })?;
    Ok(mapping.local_plugin_paths_by_remote_plugin_id)
}

fn read_plugin_share_local_paths_for_update(
    codex_home: &Path,
) -> io::Result<BTreeMap<String, AbsolutePathBuf>> {
    match read_plugin_share_local_paths(codex_home) {
        Ok(mapping) => Ok(mapping),
        // This is a best-effort cache under .tmp, so malformed state should not
        // permanently block future share saves or deletes.
        Err(err) if err.kind() == io::ErrorKind::InvalidData => Ok(BTreeMap::new()),
        Err(err) => Err(err),
    }
}

fn write_plugin_share_local_paths(
    codex_home: &Path,
    mapping: BTreeMap<String, AbsolutePathBuf>,
) -> io::Result<()> {
    let path = plugin_share_local_paths_path(codex_home);
    if mapping.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        }
    }

    let contents = serde_json::to_string_pretty(&PluginShareLocalPaths {
        local_plugin_paths_by_remote_plugin_id: mapping,
    })
    .map_err(io::Error::other)?;
    write_atomically(&path, &format!("{contents}\n"))
}

fn write_atomically(write_path: &Path, contents: &str) -> io::Result<()> {
    let parent = write_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path {} has no parent directory", write_path.display()),
        )
    })?;
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(contents.as_bytes())?;
    tmp.persist(write_path).map_err(|err| err.error)?;
    Ok(())
}

fn plugin_share_local_paths_path(codex_home: &Path) -> std::path::PathBuf {
    codex_home.join(PLUGIN_SHARE_LOCAL_PATHS_FILE)
}

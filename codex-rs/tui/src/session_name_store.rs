use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use tracing::warn;

const SESSION_NAMES_FILE: &str = "session-names.json";

fn store_path(codex_home: &Path) -> PathBuf {
    codex_home.join(SESSION_NAMES_FILE)
}

fn read_raw_map(path: &Path) -> io::Result<BTreeMap<String, String>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                return Ok(BTreeMap::new());
            }
            match serde_json::from_str::<BTreeMap<String, String>>(&contents) {
                Ok(raw) => Ok(raw),
                Err(err) => {
                    warn!("failed to parse session names file {:?}: {}", path, err);
                    Ok(BTreeMap::new())
                }
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(err) => Err(err),
    }
}

fn write_raw_map(path: &Path, map: &BTreeMap<String, String>) -> io::Result<()> {
    if map.is_empty() {
        if let Err(err) = std::fs::remove_file(path) {
            if err.kind() != io::ErrorKind::NotFound {
                return Err(err);
            }
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let contents = serde_json::to_string_pretty(map)?;
    let mut options = OpenOptions::new();
    options.write(true).truncate(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    use std::io::Write as _;
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub(crate) fn load_all(codex_home: &Path) -> HashMap<PathBuf, String> {
    let path = store_path(codex_home);
    let raw = match read_raw_map(&path) {
        Ok(map) => map,
        Err(err) => {
            warn!("failed to read session names store {:?}: {}", path, err);
            return HashMap::new();
        }
    };

    raw.into_iter()
        .filter_map(|(key, value)| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return None;
            }
            let path = PathBuf::from(key);
            if path.as_os_str().is_empty() {
                return None;
            }
            Some((path, trimmed))
        })
        .collect()
}

pub(crate) fn read(codex_home: &Path, rollout_path: &Path) -> Option<String> {
    let key = key_for(rollout_path);
    let path = store_path(codex_home);
    read_raw_map(&path)
        .ok()
        .and_then(|map| map.get(&key).cloned())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn write(codex_home: &Path, rollout_path: &Path, name: Option<&str>) -> io::Result<()> {
    let path = store_path(codex_home);
    let mut map = read_raw_map(&path)?;
    let key = key_for(rollout_path);
    match name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(value) => {
            map.insert(key, value.to_string());
        }
        None => {
            map.remove(&key);
        }
    }
    write_raw_map(&path, &map)
}

fn key_for(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

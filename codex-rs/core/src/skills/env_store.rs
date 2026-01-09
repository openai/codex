use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use tracing::warn;

const KEYRING_SERVICE: &str = "Codex Env Vars";
const ENV_VARS_FILE: &str = "env_vars.json";

/// Env var persistence uses keyring first, then falls back to a local file.
/// Keys are the raw env var names, without skill-level namespacing.
/// The file lives at `codex_home/env_vars.json` with 0600 permissions on Unix.
pub(crate) fn load_env_var(codex_home: &Path, name: &str) -> io::Result<Option<String>> {
    let keyring_store = DefaultKeyringStore;
    match keyring_store.load(KEYRING_SERVICE, name) {
        Ok(Some(value)) => return Ok(Some(value)),
        Ok(None) => {}
        Err(error) => {
            warn!("failed to read env var from keyring: {}", error.message());
        }
    }

    load_env_var_from_file(codex_home, name)
}

pub(crate) fn save_env_var(codex_home: &Path, name: &str, value: &str) -> io::Result<()> {
    let keyring_store = DefaultKeyringStore;
    match keyring_store.save(KEYRING_SERVICE, name, value) {
        Ok(()) => {
            let _ = delete_env_var_from_file(codex_home, name);
            return Ok(());
        }
        Err(error) => {
            warn!("failed to write env var to keyring: {}", error.message());
        }
    }

    save_env_var_to_file(codex_home, name, value)
}

fn env_vars_file_path(codex_home: &Path) -> PathBuf {
    codex_home.join(ENV_VARS_FILE)
}

fn load_env_var_from_file(codex_home: &Path, name: &str) -> io::Result<Option<String>> {
    let env_file = env_vars_file_path(codex_home);
    let contents = match fs::read_to_string(&env_file) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let env_map: HashMap<String, String> = serde_json::from_str(&contents)?;
    Ok(env_map.get(name).cloned())
}

fn save_env_var_to_file(codex_home: &Path, name: &str, value: &str) -> io::Result<()> {
    let env_file = env_vars_file_path(codex_home);
    if let Some(parent) = env_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut env_map: HashMap<String, String> = match fs::read_to_string(&env_file) {
        Ok(contents) => serde_json::from_str(&contents)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(err) => return Err(err),
    };
    env_map.insert(name.to_string(), value.to_string());
    let json_data = serde_json::to_string_pretty(&env_map)?;
    let mut options = fs::OpenOptions::new();
    options.truncate(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&env_file)?;
    file.write_all(json_data.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn delete_env_var_from_file(codex_home: &Path, name: &str) -> io::Result<bool> {
    let env_file = env_vars_file_path(codex_home);
    let contents = match fs::read_to_string(&env_file) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    let mut env_map: HashMap<String, String> = serde_json::from_str(&contents)?;
    let removed = env_map.remove(name).is_some();
    if !removed {
        return Ok(false);
    }
    if env_map.is_empty() {
        fs::remove_file(env_file)?;
        return Ok(true);
    }
    let json_data = serde_json::to_string_pretty(&env_map)?;
    let mut options = fs::OpenOptions::new();
    options.truncate(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&env_file)?;
    file.write_all(json_data.as_bytes())?;
    file.flush()?;
    Ok(true)
}

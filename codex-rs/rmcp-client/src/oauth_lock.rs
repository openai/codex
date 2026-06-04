use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use codex_utils_home_dir::find_codex_home;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;

use crate::oauth::sha_256_prefix;

pub(super) struct OAuthFileLock {
    _file: fs::File,
    _in_process_guard: Option<OwnedMutexGuard<()>>,
}

pub(super) struct FallbackStoreLock {
    _in_process_guard: std::sync::MutexGuard<'static, ()>,
    _file: fs::File,
}

pub(super) fn acquire_oauth_server_lock(server_name: &str, url: &str) -> Result<OAuthFileLock> {
    let file = lock_oauth_file(oauth_server_lock_path(server_name, url)?)?;
    Ok(OAuthFileLock {
        _file: file,
        _in_process_guard: None,
    })
}

pub(super) async fn acquire_oauth_server_lock_async(
    server_name: &str,
    url: &str,
) -> Result<OAuthFileLock> {
    let in_process_lock = oauth_server_lock_for(server_name, url);
    let in_process_guard = in_process_lock.lock_owned().await;
    let path = oauth_server_lock_path(server_name, url)?;
    let file_lock = tokio::task::spawn_blocking(move || lock_oauth_file(path))
        .await
        .context("OAuth credential lock task failed")??;
    Ok(OAuthFileLock {
        _file: file_lock,
        _in_process_guard: Some(in_process_guard),
    })
}

pub(super) fn acquire_fallback_store_lock() -> Result<FallbackStoreLock> {
    static FALLBACK_STORE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    let in_process_guard = FALLBACK_STORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let file = lock_oauth_file(oauth_lock_dir()?.join("fallback-store.lock"))?;
    Ok(FallbackStoreLock {
        _in_process_guard: in_process_guard,
        _file: file,
    })
}

fn oauth_server_lock_for(server_name: &str, url: &str) -> Arc<Mutex<()>> {
    static OAUTH_SERVER_LOCKS: OnceLock<std::sync::Mutex<BTreeMap<String, Arc<Mutex<()>>>>> =
        OnceLock::new();

    let mut locks = OAUTH_SERVER_LOCKS
        .get_or_init(std::sync::Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks
        .entry(format!("{server_name}\n{url}"))
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn lock_oauth_file(path: PathBuf) -> Result<fs::File> {
    let file = open_oauth_lock_file(path)?;
    file.lock()?;
    Ok(file)
}

fn open_oauth_lock_file(path: PathBuf) -> Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?)
}

fn oauth_server_lock_path(server_name: &str, url: &str) -> Result<PathBuf> {
    let digest = sha_256_prefix(&Value::String(format!("{server_name}\n{url}")))?;
    Ok(oauth_lock_dir()?.join(format!("server-{digest}.lock")))
}

fn oauth_lock_dir() -> Result<PathBuf> {
    Ok(find_codex_home()?.join(".mcp-oauth-locks").to_path_buf())
}

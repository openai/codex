mod macos;

use std::env;
use std::future::Future;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::task;
use toml::Value as TomlValue;

#[cfg(test)]
mod test_support {
    /// Test-only override plumbing so integration tests can bypass the
    /// environment variable and supply a managed config path directly.
    use super::*;
    use std::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;
    use std::panic::resume_unwind;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    static MANAGED_CONFIG_PATH_OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    static MANAGED_CONFIG_PATH_SERIALIZER: OnceLock<Mutex<()>> = OnceLock::new();

    fn managed_config_path_override_storage() -> &'static Mutex<Option<PathBuf>> {
        MANAGED_CONFIG_PATH_OVERRIDE.get_or_init(|| Mutex::new(None))
    }

    fn managed_config_path_serializer() -> &'static Mutex<()> {
        MANAGED_CONFIG_PATH_SERIALIZER.get_or_init(|| Mutex::new(()))
    }

    fn replace_managed_config_path_override(value: Option<PathBuf>) -> Option<PathBuf> {
        let mut guard = match managed_config_path_override_storage().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::mem::replace(&mut *guard, value)
    }

    pub(super) fn current_managed_config_path_override() -> Option<PathBuf> {
        match managed_config_path_override_storage().lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(super) fn with_managed_config_path_override<R>(
        path: Option<&Path>,
        f: impl FnOnce() -> R + std::panic::UnwindSafe,
    ) -> R {
        let serializer_guard = match managed_config_path_serializer().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let previous = replace_managed_config_path_override(path.map(std::path::Path::to_path_buf));
        let result = catch_unwind(AssertUnwindSafe(f));
        replace_managed_config_path_override(previous);
        drop(serializer_guard);
        match result {
            Ok(output) => output,
            Err(payload) => resume_unwind(payload),
        }
    }
}

#[cfg(test)]
use test_support::current_managed_config_path_override;
#[cfg(test)]
pub(super) use test_support::with_managed_config_path_override;

const CONFIG_TOML_FILE: &str = "config.toml";
const MANAGED_CONFIG_PATH_ENV_VAR: &str = "CODEX_MANAGED_CONFIG_PATH";
#[cfg(unix)]
const MANAGED_CONFIG_SYSTEM_PATH: &str = "/etc/codex/managed_config.toml";

#[derive(Debug)]
pub(crate) struct LoadedConfigLayers {
    pub base: TomlValue,
    pub managed_config: Option<TomlValue>,
    pub managed_preferences: Option<TomlValue>,
}

// Configuration layering pipeline (top overrides bottom):
//
//        +-------------------------+
//        | Managed preferences (*) |
//        +-------------------------+
//                    ^
//                    |
//        +-------------------------+
//        |  managed_config.toml   |
//        +-------------------------+
//                    ^
//                    |
//        +-------------------------+
//        |    config.toml (base)   |
//        +-------------------------+
//
// (*) Only available on macOS via managed device profiles.

pub async fn load_config_as_toml_async(codex_home: PathBuf) -> io::Result<TomlValue> {
    let LoadedConfigLayers {
        mut base,
        managed_config,
        managed_preferences,
    } = load_config_layers_async(codex_home).await?;

    for overlay in [managed_config, managed_preferences].into_iter().flatten() {
        merge_toml_values(&mut base, &overlay);
    }

    Ok(base)
}

pub fn load_config_as_toml(codex_home: &Path) -> io::Result<TomlValue> {
    block_on_config_future(load_config_as_toml_async(codex_home.to_path_buf()))
}

pub(crate) async fn load_config_layers_async(
    codex_home: PathBuf,
) -> io::Result<LoadedConfigLayers> {
    let user_config = read_config_from_path(codex_home.join(CONFIG_TOML_FILE), true).await?;
    let managed_config = read_config_from_path(managed_config_path(&codex_home), false).await?;
    let managed_preferences = load_managed_admin_config_layer_async().await?;

    Ok(LoadedConfigLayers {
        base: user_config.unwrap_or_else(default_empty_table),
        managed_config,
        managed_preferences,
    })
}

pub(crate) fn load_config_layers(codex_home: &Path) -> io::Result<LoadedConfigLayers> {
    block_on_config_future(load_config_layers_async(codex_home.to_path_buf()))
}

fn default_empty_table() -> TomlValue {
    TomlValue::Table(Default::default())
}

async fn read_config_from_path(
    path: PathBuf,
    log_missing_as_info: bool,
) -> io::Result<Option<TomlValue>> {
    match fs::read_to_string(&path).await {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                tracing::error!("Failed to parse {}: {err}", path.display());
                Err(io::Error::new(io::ErrorKind::InvalidData, err))
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if log_missing_as_info {
                tracing::info!("{} not found, using defaults", path.display());
            } else {
                tracing::debug!("{} not found", path.display());
            }
            Ok(None)
        }
        Err(err) => {
            tracing::error!("Failed to read {}: {err}", path.display());
            Err(err)
        }
    }
}

#[cfg(target_os = "macos")]
async fn load_managed_admin_config_layer_async() -> io::Result<Option<TomlValue>> {
    const LOAD_ERROR: &str = "Failed to load managed preferences configuration";

    match task::spawn_blocking(load_managed_admin_config).await {
        Ok(result) => result,
        Err(join_err) => {
            if join_err.is_cancelled() {
                tracing::error!("Managed preferences load task was cancelled");
            } else {
                tracing::error!("Managed preferences load task failed: {join_err}");
            }
            Err(io::Error::other(LOAD_ERROR))
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn load_managed_admin_config_layer_async() -> io::Result<Option<TomlValue>> {
    Ok(None)
}

/// Execute the given config-loading future, regardless of whether a Tokio
/// runtime is already running on the current thread.
///
/// Synchronous call sites can reuse the async implementation without
/// duplicating file I/O by delegating through this helper.
fn block_on_config_future<F, T>(future: F) -> io::Result<T>
where
    F: Future<Output = io::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        task::block_in_place(|| handle.block_on(future))
    } else {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to create runtime for managed preferences loading: {err}"
                ))
            })?;
        runtime.block_on(future)
    }
}

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub(crate) fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        for (key, value) in overlay_table {
            if let Some(existing) = base_table.get_mut(key) {
                merge_toml_values(existing, value);
            } else {
                base_table.insert(key.clone(), value.clone());
            }
        }
    } else {
        *base = overlay.clone();
    }
}

#[cfg(target_os = "macos")]
fn load_managed_admin_config() -> io::Result<Option<TomlValue>> {
    macos::load_managed_admin_config()
}

/// Determine the managed-config layer location. Tests and custom installs can
/// override the default system path with `CODEX_MANAGED_CONFIG_PATH`.
fn managed_config_path(codex_home: &Path) -> PathBuf {
    #[cfg(test)]
    {
        if let Some(path) = current_managed_config_path_override() {
            return path;
        }
    }

    if let Some(path) = env::var(MANAGED_CONFIG_PATH_ENV_VAR)
        .ok()
        .filter(|path| !path.is_empty())
    {
        return PathBuf::from(path);
    }

    #[cfg(unix)]
    {
        let _ = codex_home;
        PathBuf::from(MANAGED_CONFIG_SYSTEM_PATH)
    }

    #[cfg(not(unix))]
    {
        codex_home.join("managed_config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn merges_managed_config_layer_on_top() {
        #[cfg(target_os = "macos")]
        super::macos::with_cleared_test_managed_preferences(run_merges_managed_config_layer_on_top);

        #[cfg(not(target_os = "macos"))]
        run_merges_managed_config_layer_on_top();
    }

    #[test]
    fn returns_empty_when_all_layers_missing() {
        #[cfg(target_os = "macos")]
        super::macos::with_cleared_test_managed_preferences(run_returns_empty_when_layers_missing);

        #[cfg(not(target_os = "macos"))]
        run_returns_empty_when_layers_missing();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn managed_preferences_take_highest_precedence() {
        use base64::Engine;

        let managed_payload = r#"
[nested]
value = "managed"
flag = false
"#;
        let encoded = base64::prelude::BASE64_STANDARD.encode(managed_payload.as_bytes());
        super::macos::with_encoded_test_managed_preferences(
            &encoded,
            run_managed_preferences_take_highest_precedence,
        );
    }

    fn run_merges_managed_config_layer_on_top() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

        super::with_managed_config_path_override(Some(&managed_path), || {
            std::fs::write(
                tmp.path().join(CONFIG_TOML_FILE),
                r#"foo = 1

[nested]
value = "base"
"#,
            )
            .expect("write base");
            std::fs::write(
                &managed_path,
                r#"foo = 2

[nested]
value = "managed_config"
extra = true
"#,
            )
            .expect("write managed config");

            let loaded = load_config_as_toml(tmp.path()).expect("load config");
            let table = loaded.as_table().expect("top-level table expected");

            assert_eq!(table.get("foo"), Some(&TomlValue::Integer(2)));
            let nested = table
                .get("nested")
                .and_then(|v| v.as_table())
                .expect("nested");
            assert_eq!(
                nested.get("value"),
                Some(&TomlValue::String("managed_config".to_string()))
            );
            assert_eq!(nested.get("extra"), Some(&TomlValue::Boolean(true)));
        });
    }

    fn run_returns_empty_when_layers_missing() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

        super::with_managed_config_path_override(Some(&managed_path), || {
            let layers = load_config_layers(tmp.path()).expect("load layers");
            let base_table = layers.base.as_table().expect("base table expected");
            assert!(
                base_table.is_empty(),
                "expected empty base layer when configs missing"
            );
            assert!(
                layers.managed_config.is_none(),
                "managed config layer should be absent when file missing"
            );

            #[cfg(not(target_os = "macos"))]
            {
                let loaded = load_config_as_toml(tmp.path()).expect("load config");
                let table = loaded.as_table().expect("top-level table expected");
                assert!(
                    table.is_empty(),
                    "expected empty table when configs missing"
                );
            }
        });
    }

    #[cfg(target_os = "macos")]
    fn run_managed_preferences_take_highest_precedence() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

        super::with_managed_config_path_override(Some(&managed_path), || {
            std::fs::write(
                tmp.path().join(CONFIG_TOML_FILE),
                r#"[nested]
value = "base"
"#,
            )
            .expect("write base");
            std::fs::write(
                &managed_path,
                r#"[nested]
value = "managed_config"
flag = true
"#,
            )
            .expect("write managed config");

            let loaded = load_config_as_toml(tmp.path()).expect("load config");
            let nested = loaded
                .get("nested")
                .and_then(|v| v.as_table())
                .expect("nested table");
            assert_eq!(
                nested.get("value"),
                Some(&TomlValue::String("managed".to_string()))
            );
            assert_eq!(nested.get("flag"), Some(&TomlValue::Boolean(false)));
        });
    }
}

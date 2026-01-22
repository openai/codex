mod macos;

use crate::config::CONFIG_TOML_FILE;
use macos::load_managed_admin_config_layer;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use toml::Value as TomlValue;

#[cfg(unix)]
const CODEX_MANAGED_CONFIG_SYSTEM_PATH: &str = "/etc/codex/managed_config.toml";

#[derive(Debug, Clone)]
pub struct LoadedConfigLayers {
    pub base: TomlValue,
    pub managed_config: Option<TomlValue>,
    pub managed_preferences: Option<TomlValue>,
}

#[derive(Debug, Default, Clone)]
pub struct LoaderOverrides {
    pub managed_config_path: Option<PathBuf>,
    #[cfg(target_os = "macos")]
    pub managed_preferences_base64: Option<String>,
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

pub async fn load_config_as_toml(codex_home: &Path) -> io::Result<TomlValue> {
    load_config_as_toml_with_overrides(codex_home, LoaderOverrides::default()).await
}

pub async fn load_config_layers(codex_home: &Path) -> io::Result<LoadedConfigLayers> {
    load_config_layers_with_overrides(codex_home, LoaderOverrides::default()).await
}

fn default_empty_table() -> TomlValue {
    TomlValue::Table(Default::default())
}

pub async fn load_config_layers_with_overrides(
    codex_home: &Path,
    overrides: LoaderOverrides,
) -> io::Result<LoadedConfigLayers> {
    load_config_layers_internal(codex_home, overrides).await
}

async fn load_config_as_toml_with_overrides(
    codex_home: &Path,
    overrides: LoaderOverrides,
) -> io::Result<TomlValue> {
    let layers = load_config_layers_internal(codex_home, overrides).await?;
    Ok(apply_managed_layers(layers))
}

async fn load_config_layers_internal(
    codex_home: &Path,
    overrides: LoaderOverrides,
) -> io::Result<LoadedConfigLayers> {
    #[cfg(target_os = "macos")]
    let LoaderOverrides {
        managed_config_path,
        managed_preferences_base64,
    } = overrides;

    #[cfg(not(target_os = "macos"))]
    let LoaderOverrides {
        managed_config_path,
    } = overrides;

    let managed_config_path =
        managed_config_path.unwrap_or_else(|| managed_config_default_path(codex_home));

    let user_config_path = codex_home.join(CONFIG_TOML_FILE);
    let user_config = read_config_from_path(&user_config_path, true).await?;
    let managed_config = read_config_from_path(&managed_config_path, false).await?;

    #[cfg(target_os = "macos")]
    let managed_preferences =
        load_managed_admin_config_layer(managed_preferences_base64.as_deref()).await?;

    #[cfg(not(target_os = "macos"))]
    let managed_preferences = load_managed_admin_config_layer(None).await?;

    Ok(LoadedConfigLayers {
        base: user_config.unwrap_or_else(default_empty_table),
        managed_config,
        managed_preferences,
    })
}

async fn read_config_from_path(
    path: &Path,
    log_missing_as_info: bool,
) -> io::Result<Option<TomlValue>> {
    match fs::read_to_string(path).await {
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

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
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

fn managed_config_default_path(codex_home: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("CODEX_MANAGED_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(unix)]
    {
        let _ = codex_home;
        PathBuf::from(CODEX_MANAGED_CONFIG_SYSTEM_PATH)
    }

    #[cfg(not(unix))]
    {
        codex_home.join("managed_config.toml")
    }
}

fn apply_managed_layers(layers: LoadedConfigLayers) -> TomlValue {
    let LoadedConfigLayers {
        mut base,
        managed_config,
        managed_preferences,
    } = layers;

    for overlay in [managed_config, managed_preferences].into_iter().flatten() {
        merge_toml_values(&mut base, &overlay);
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn merges_managed_config_layer_on_top() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

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

        let overrides = LoaderOverrides {
            managed_config_path: Some(managed_path),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides)
            .await
            .expect("load config");
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
    }

    #[tokio::test]
    async fn returns_empty_when_all_layers_missing() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");
        let overrides = LoaderOverrides {
            managed_config_path: Some(managed_path),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let layers = load_config_layers_with_overrides(tmp.path(), overrides)
            .await
            .expect("load layers");
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
            let loaded = load_config_as_toml(tmp.path()).await.expect("load config");
            let table = loaded.as_table().expect("top-level table expected");
            assert!(
                table.is_empty(),
                "expected empty table when configs missing"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn managed_preferences_take_highest_precedence() {
        use base64::Engine;

        let managed_payload = r#"
[nested]
value = "managed"
flag = false
"#;
        let encoded = base64::prelude::BASE64_STANDARD.encode(managed_payload.as_bytes());
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

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

        let overrides = LoaderOverrides {
            managed_config_path: Some(managed_path),
            managed_preferences_base64: Some(encoded),
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides)
            .await
            .expect("load config");
        let nested = loaded
            .get("nested")
            .and_then(|v| v.as_table())
            .expect("nested table");
        assert_eq!(
            nested.get("value"),
            Some(&TomlValue::String("managed".to_string()))
        );
        assert_eq!(nested.get("flag"), Some(&TomlValue::Boolean(false)));
    }
}

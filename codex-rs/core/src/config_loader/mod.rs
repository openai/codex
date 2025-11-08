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

#[derive(Debug)]
pub(crate) struct LoadedConfigLayers {
    pub base: TomlValue,
    pub project_config: Option<TomlValue>,
    pub managed_config: Option<TomlValue>,
    pub managed_preferences: Option<TomlValue>,
}

#[derive(Debug, Default)]
pub(crate) struct LoaderOverrides {
    pub managed_config_path: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
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
//        |  managed_config.toml    |
//        +-------------------------+
//                    ^
//                    |
//        +-------------------------+
//        | .codex/config.toml (**) |
//        +-------------------------+
//                    ^
//                    |
//        +-------------------------+
//        |    config.toml (base)   |
//        +-------------------------+
//
// (*) Only available on macOS via managed device profiles.
// (**) Project-level config found by walking up from cwd.

pub async fn load_config_as_toml(codex_home: &Path) -> io::Result<TomlValue> {
    load_config_as_toml_with_overrides(codex_home, LoaderOverrides::default()).await
}

fn default_empty_table() -> TomlValue {
    TomlValue::Table(Default::default())
}

pub(crate) async fn load_config_layers_with_overrides(
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
        cwd,
        managed_preferences_base64,
    } = overrides;

    #[cfg(not(target_os = "macos"))]
    let LoaderOverrides {
        managed_config_path,
        cwd,
    } = overrides;

    let managed_config_path =
        managed_config_path.unwrap_or_else(|| managed_config_default_path(codex_home));

    // Resolve cwd to find project config
    let resolved_cwd = match cwd {
        Some(p) if p.is_absolute() => p,
        Some(p) => std::env::current_dir()?.join(p),
        None => std::env::current_dir()?,
    };

    let user_config_path = codex_home.join(CONFIG_TOML_FILE);
    let user_config = read_config_from_path(&user_config_path, true).await?;

    // Load project config only if it exists AND is allowed by trust settings
    let project_config = if let Some(project_config_path) = find_project_config_path(&resolved_cwd) {
        // Check if this project is allowed to use project configs
        let empty_table = default_empty_table();
        let user_config_value = user_config.as_ref().unwrap_or(&empty_table);
        if is_project_config_allowed(user_config_value, &resolved_cwd) {
            tracing::info!(
                "Loading project config from {} (project is trusted)",
                project_config_path.display()
            );
            read_config_from_path(&project_config_path, false).await?
        } else {
            tracing::warn!(
                "Found project config at {} but project is not trusted. \
                 Set trust_level = \"trusted\" in ~/.codex/config.toml to enable project configs.",
                project_config_path.display()
            );
            None
        }
    } else {
        None
    };

    let managed_config = read_config_from_path(&managed_config_path, false).await?;

    #[cfg(target_os = "macos")]
    let managed_preferences =
        load_managed_admin_config_layer(managed_preferences_base64.as_deref()).await?;

    #[cfg(not(target_os = "macos"))]
    let managed_preferences = load_managed_admin_config_layer(None).await?;

    Ok(LoadedConfigLayers {
        base: user_config.unwrap_or_else(default_empty_table),
        project_config,
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

/// Find a project-level config file by walking up from the given directory.
/// Looks for `.codex/config.toml` in the current directory and all parent directories.
/// Returns the path to the first config file found, or None if no project config exists.
fn find_project_config_path(cwd: &Path) -> Option<PathBuf> {
    let mut current = cwd;

    loop {
        let candidate = current.join(".codex").join(CONFIG_TOML_FILE);
        if candidate.exists() && candidate.is_file() {
            tracing::debug!("Found project config at {}", candidate.display());
            return Some(candidate);
        }

        // Move up to parent directory, or stop if we've reached the filesystem root
        current = current.parent()?;
    }
}

/// Check if a project is allowed to use project-level configs based on the user config.
/// This parses the projects section of the user config to determine trust settings.
fn is_project_config_allowed(user_config: &TomlValue, cwd: &Path) -> bool {
    use crate::git_info::get_git_repo_root;

    let Some(projects_table) = user_config.get("projects").and_then(|v| v.as_table()) else {
        // No projects configured means no trust, so project configs not allowed
        tracing::debug!("No projects configured, project config not allowed");
        return false;
    };

    // First, try exact match with cwd
    if let Some(project_config) = projects_table.get(cwd.to_string_lossy().as_ref()) {
        return check_project_config_allows(project_config);
    }

    // If in a git repo, try matching the git root
    if let Some(git_root) = get_git_repo_root(cwd) {
        if let Some(project_config) = projects_table.get(git_root.to_string_lossy().as_ref()) {
            return check_project_config_allows(project_config);
        }
    }

    // No matching project found, default to not allowed
    tracing::debug!("No matching trusted project found for {}", cwd.display());
    false
}

/// Check if a project config TOML value allows project configs.
fn check_project_config_allows(project_toml: &TomlValue) -> bool {
    // Check explicit allow_project_config setting
    if let Some(allow) = project_toml
        .get("allow_project_config")
        .and_then(|v| v.as_bool())
    {
        return allow;
    }

    // Fall back to checking trust_level (trusted projects allow by default)
    let is_trusted = project_toml
        .get("trust_level")
        .and_then(|v| v.as_str())
        .map(|s| s == "trusted")
        .unwrap_or(false);

    is_trusted
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

fn managed_config_default_path(codex_home: &Path) -> PathBuf {
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
        project_config,
        managed_config,
        managed_preferences,
    } = layers;

    // Apply layers in order of precedence (later layers override earlier ones)
    for overlay in [project_config, managed_config, managed_preferences]
        .into_iter()
        .flatten()
    {
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
            cwd: None,
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
            cwd: None,
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
            cwd: None,
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

    #[tokio::test]
    async fn loads_project_config_for_trusted_project() {
        let tmp = tempdir().expect("tempdir");
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".codex")).expect("create project .codex dir");

        // Write user config with trusted project
        std::fs::write(
            tmp.path().join(CONFIG_TOML_FILE),
            format!(
                r#"foo = 1

[projects."{}"]
trust_level = "trusted"
"#,
                project_dir.display()
            ),
        )
        .expect("write base config");

        // Write project config
        std::fs::write(
            project_dir.join(".codex").join(CONFIG_TOML_FILE),
            r#"foo = 2
bar = "from_project"
"#,
        )
        .expect("write project config");

        let overrides = LoaderOverrides {
            cwd: Some(project_dir.clone()),
            managed_config_path: Some(tmp.path().join("nonexistent_managed.toml")),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides)
            .await
            .expect("load config");
        let table = loaded.as_table().expect("top-level table expected");

        // Project config should override user config
        assert_eq!(table.get("foo"), Some(&TomlValue::Integer(2)));
        assert_eq!(
            table.get("bar"),
            Some(&TomlValue::String("from_project".to_string()))
        );
    }

    #[tokio::test]
    async fn ignores_project_config_for_untrusted_project() {
        let tmp = tempdir().expect("tempdir");
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".codex")).expect("create project .codex dir");

        // Write user config WITHOUT trusting the project
        std::fs::write(
            tmp.path().join(CONFIG_TOML_FILE),
            r#"foo = 1
"#,
        )
        .expect("write base config");

        // Write project config (should be ignored)
        std::fs::write(
            project_dir.join(".codex").join(CONFIG_TOML_FILE),
            r#"foo = 2
bar = "from_project"
"#,
        )
        .expect("write project config");

        let overrides = LoaderOverrides {
            cwd: Some(project_dir.clone()),
            managed_config_path: Some(tmp.path().join("nonexistent_managed.toml")),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides)
            .await
            .expect("load config");
        let table = loaded.as_table().expect("top-level table expected");

        // Project config should be ignored, only user config applies
        assert_eq!(table.get("foo"), Some(&TomlValue::Integer(1)));
        assert_eq!(table.get("bar"), None);
    }

    #[tokio::test]
    async fn project_config_respects_allow_project_config_setting() {
        let tmp = tempdir().expect("tempdir");
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".codex")).expect("create project .codex dir");

        // Write user config with trusted project but allow_project_config = false
        std::fs::write(
            tmp.path().join(CONFIG_TOML_FILE),
            format!(
                r#"foo = 1

[projects."{}"]
trust_level = "trusted"
allow_project_config = false
"#,
                project_dir.display()
            ),
        )
        .expect("write base config");

        // Write project config (should be ignored despite trust)
        std::fs::write(
            project_dir.join(".codex").join(CONFIG_TOML_FILE),
            r#"foo = 2
bar = "from_project"
"#,
        )
        .expect("write project config");

        let overrides = LoaderOverrides {
            cwd: Some(project_dir.clone()),
            managed_config_path: Some(tmp.path().join("nonexistent_managed.toml")),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides)
            .await
            .expect("load config");
        let table = loaded.as_table().expect("top-level table expected");

        // Project config should be ignored due to allow_project_config = false
        assert_eq!(table.get("foo"), Some(&TomlValue::Integer(1)));
        assert_eq!(table.get("bar"), None);
    }
}

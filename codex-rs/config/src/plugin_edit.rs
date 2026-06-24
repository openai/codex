use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use codex_utils_path::resolve_symlink_write_paths;
use codex_utils_path::write_atomically;
use tokio::task;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::value;

use crate::CONFIG_TOML_FILE;
use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use crate::ConfigLayerStackOrdering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginConfigEdit {
    SetEnabled { plugin_key: String, enabled: bool },
    Clear { plugin_key: String },
}

pub async fn set_user_plugin_enabled(
    codex_home: &Path,
    plugin_key: String,
    enabled: bool,
) -> std::io::Result<()> {
    apply_user_plugin_config_edits(
        codex_home,
        vec![PluginConfigEdit::SetEnabled {
            plugin_key,
            enabled,
        }],
    )
    .await
}

pub async fn clear_user_plugin(codex_home: &Path, plugin_key: String) -> std::io::Result<()> {
    apply_user_plugin_config_edits(codex_home, vec![PluginConfigEdit::Clear { plugin_key }]).await
}

pub async fn reconcile_user_plugin_registrations(
    codex_home: &Path,
    config_layer_stack: Option<&ConfigLayerStack>,
    plugin_key: String,
    plugin_name: String,
    enable_canonical: bool,
) -> std::io::Result<()> {
    let target_path = codex_home.join(CONFIG_TOML_FILE);
    let mut paths = BTreeSet::new();
    if let Some(config_layer_stack) = config_layer_stack {
        for layer in config_layer_stack.get_user_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        ) {
            if let ConfigLayerSource::User { file, .. } = &layer.name {
                paths.insert(file.as_path().to_path_buf());
            }
        }
    }
    paths.remove(&target_path);
    task::spawn_blocking(move || {
        for path in paths {
            reconcile_plugin_config_path(&path, &plugin_key, &plugin_name, /*enable*/ false)?;
        }
        reconcile_plugin_config_path(&target_path, &plugin_key, &plugin_name, enable_canonical)
    })
    .await
    .map_err(|err| std::io::Error::other(format!("config persistence task panicked: {err}")))?
}

pub async fn apply_user_plugin_config_edits(
    codex_home: &Path,
    edits: Vec<PluginConfigEdit>,
) -> std::io::Result<()> {
    let codex_home = codex_home.to_path_buf();
    task::spawn_blocking(move || apply_user_plugin_config_edits_blocking(&codex_home, edits))
        .await
        .map_err(|err| std::io::Error::other(format!("config persistence task panicked: {err}")))?
}

fn apply_user_plugin_config_edits_blocking(
    codex_home: &Path,
    edits: Vec<PluginConfigEdit>,
) -> std::io::Result<()> {
    if edits.is_empty() {
        return Ok(());
    }

    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let write_paths = resolve_symlink_write_paths(&config_path)?;
    let mut doc = read_or_create_document(write_paths.read_path.as_deref())?;
    let mut mutated = false;
    for edit in edits {
        mutated |= match edit {
            PluginConfigEdit::SetEnabled {
                plugin_key,
                enabled,
            } => set_plugin_enabled(&mut doc, &plugin_key, enabled),
            PluginConfigEdit::Clear { plugin_key } => clear_plugin(&mut doc, &plugin_key),
        };
    }
    if !mutated {
        return Ok(());
    }
    write_atomically(&write_paths.write_path, &doc.to_string())
}

fn reconcile_plugin_config_path(
    config_path: &Path,
    plugin_key: &str,
    plugin_name: &str,
    enable: bool,
) -> std::io::Result<()> {
    let write_paths = resolve_symlink_write_paths(config_path)?;
    let mut doc = read_or_create_document(write_paths.read_path.as_deref())?;
    let mutated = if enable {
        set_plugin_enabled_exclusively(&mut doc, plugin_key, plugin_name)
    } else {
        remove_other_plugin_registrations(&mut doc, plugin_key, plugin_name)
    };
    if mutated {
        write_atomically(&write_paths.write_path, &doc.to_string())?;
    }
    Ok(())
}

fn read_or_create_document(config_path: Option<&Path>) -> std::io::Result<DocumentMut> {
    let Some(config_path) = config_path else {
        return Ok(DocumentMut::new());
    };
    match fs::read_to_string(config_path) {
        Ok(raw) => raw
            .parse::<DocumentMut>()
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(err) => Err(err),
    }
}

fn set_plugin_enabled(doc: &mut DocumentMut, plugin_key: &str, enabled: bool) -> bool {
    let Some(plugins) = ensure_plugins_table(doc) else {
        return false;
    };
    let Some(plugin) = ensure_table_for_write(&mut plugins[plugin_key]) else {
        return false;
    };
    let mut replacement = value(enabled);
    if let Some(existing) = plugin.get("enabled") {
        preserve_decor(existing, &mut replacement);
    }
    plugin["enabled"] = replacement;
    true
}

fn set_plugin_enabled_exclusively(
    doc: &mut DocumentMut,
    plugin_key: &str,
    plugin_name: &str,
) -> bool {
    let removed_duplicate = remove_other_plugin_registrations(doc, plugin_key, plugin_name);
    set_plugin_enabled(doc, plugin_key, /*enabled*/ true) || removed_duplicate
}

fn remove_other_plugin_registrations(
    doc: &mut DocumentMut,
    plugin_key: &str,
    plugin_name: &str,
) -> bool {
    let Some(plugins) = doc
        .as_table_mut()
        .get_mut("plugins")
        .and_then(ensure_table_for_read)
    else {
        return false;
    };
    let duplicate_keys = plugins
        .iter()
        .map(|(key, _)| key)
        .filter(|key| {
            *key != plugin_key
                && key.rsplit_once('@').is_some_and(|(name, marketplace)| {
                    name == plugin_name && !marketplace.is_empty()
                })
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut removed = false;
    for key in duplicate_keys {
        removed |= plugins.remove(&key).is_some();
    }
    removed
}

fn clear_plugin(doc: &mut DocumentMut, plugin_key: &str) -> bool {
    let root = doc.as_table_mut();
    let Some(plugins_item) = root.get_mut("plugins") else {
        return false;
    };
    let Some(plugins) = ensure_table_for_read(plugins_item) else {
        return false;
    };
    plugins.remove(plugin_key).is_some()
}

fn ensure_plugins_table(doc: &mut DocumentMut) -> Option<&mut TomlTable> {
    let root = doc.as_table_mut();
    if !root.contains_key("plugins") {
        root.insert("plugins", TomlItem::Table(new_implicit_table()));
    }
    ensure_table_for_write(root.get_mut("plugins")?)
}

fn ensure_table_for_write(item: &mut TomlItem) -> Option<&mut TomlTable> {
    match item {
        TomlItem::Table(table) => Some(table),
        TomlItem::Value(value) => {
            let table = value
                .as_inline_table()
                .map_or_else(new_implicit_table, table_from_inline);
            *item = TomlItem::Table(table);
            item.as_table_mut()
        }
        TomlItem::None => {
            *item = TomlItem::Table(new_implicit_table());
            item.as_table_mut()
        }
        _ => None,
    }
}

fn ensure_table_for_read(item: &mut TomlItem) -> Option<&mut TomlTable> {
    match item {
        TomlItem::Table(_) => {}
        TomlItem::Value(value) => {
            let inline = value.as_inline_table()?.clone();
            *item = TomlItem::Table(table_from_inline(&inline));
        }
        _ => return None,
    }
    item.as_table_mut()
}

fn table_from_inline(inline: &toml_edit::InlineTable) -> TomlTable {
    let mut table = new_implicit_table();
    for (key, value) in inline.iter() {
        let mut value = value.clone();
        value.decor_mut().set_suffix("");
        table.insert(key, TomlItem::Value(value));
    }
    table
}

fn new_implicit_table() -> TomlTable {
    let mut table = TomlTable::new();
    table.set_implicit(true);
    table
}

fn preserve_decor(existing: &TomlItem, replacement: &mut TomlItem) {
    if let (TomlItem::Value(existing_value), TomlItem::Value(replacement_value)) =
        (existing, replacement)
    {
        replacement_value
            .decor_mut()
            .clone_from(existing_value.decor());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigLayerEntry;
    use crate::config_requirements::ConfigRequirements;
    use crate::config_requirements::ConfigRequirementsToml;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn set_user_plugin_enabled_writes_plugin_entry() {
        let codex_home = TempDir::new().unwrap();

        set_user_plugin_enabled(
            codex_home.path(),
            "demo@market".to_string(),
            /*enabled*/ true,
        )
        .await
        .unwrap();

        let config = read_config(codex_home.path());
        let expected: toml::Value = toml::from_str(
            r#"
[plugins."demo@market"]
enabled = true
        "#,
        )
        .unwrap();
        assert_eq!(config, expected);
    }

    #[tokio::test]
    async fn set_user_plugin_enabled_preserves_existing_plugin_fields() {
        let codex_home = TempDir::new().unwrap();
        fs::write(
            codex_home.path().join(CONFIG_TOML_FILE),
            r#"
[plugins."demo@market"]
enabled = false
source = "/tmp/plugin"
"#,
        )
        .unwrap();

        set_user_plugin_enabled(
            codex_home.path(),
            "demo@market".to_string(),
            /*enabled*/ true,
        )
        .await
        .unwrap();

        let config = read_config(codex_home.path());
        let expected: toml::Value = toml::from_str(
            r#"
[plugins."demo@market"]
enabled = true
source = "/tmp/plugin"
        "#,
        )
        .unwrap();
        assert_eq!(config, expected);
    }

    #[tokio::test]
    async fn exclusive_registration_reconciles_active_profile_layers() {
        let codex_home = TempDir::new().unwrap();
        let base_path = codex_home.path().join(CONFIG_TOML_FILE);
        let profile_path = codex_home.path().join("work.config.toml");
        let base_config = "[plugins.\"structure-viewer@openai-internal-testing\"]\nenabled = true\n[plugins.\"sequence-viewer@openai-internal-testing\"]\nenabled = true\n";
        let profile_config = "[plugins.\"structure-viewer@debug\"]\nenabled = true\n";
        fs::write(&base_path, base_config).unwrap();
        fs::write(&profile_path, profile_config).unwrap();
        let layer = |file: &Path, profile: Option<&str>, config: &str| {
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: AbsolutePathBuf::try_from(file.to_path_buf()).unwrap(),
                    profile: profile.map(str::to_string),
                },
                toml::from_str(config).unwrap(),
            )
        };
        let stack = ConfigLayerStack::new(
            vec![
                layer(&base_path, None, base_config),
                layer(&profile_path, Some("work"), profile_config),
            ],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .unwrap();
        reconcile_user_plugin_registrations(
            codex_home.path(),
            Some(&stack),
            "structure-viewer@openai-monorepo".to_string(),
            "structure-viewer".to_string(),
            /*enable_canonical*/ true,
        )
        .await
        .unwrap();
        let parse = |path: &Path| {
            toml::from_str::<toml::Value>(&fs::read_to_string(path).unwrap()).unwrap()
        };
        assert_eq!(
            parse(&base_path),
            toml::from_str::<toml::Value>(
                "[plugins.\"sequence-viewer@openai-internal-testing\"]\nenabled = true\n[plugins.\"structure-viewer@openai-monorepo\"]\nenabled = true\n"
            )
            .unwrap(),
        );
        assert_eq!(
            parse(&profile_path),
            toml::from_str::<toml::Value>("").unwrap(),
        );
    }

    #[tokio::test]
    async fn clear_user_plugin_removes_empty_plugins_table() {
        let codex_home = TempDir::new().unwrap();
        fs::write(
            codex_home.path().join(CONFIG_TOML_FILE),
            r#"
[plugins."demo@market"]
enabled = true
"#,
        )
        .unwrap();

        clear_user_plugin(codex_home.path(), "demo@market".to_string())
            .await
            .unwrap();

        assert_eq!(
            fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap(),
            ""
        );
    }

    #[tokio::test]
    async fn clear_user_plugin_missing_entry_does_not_create_config() {
        let codex_home = TempDir::new().unwrap();

        clear_user_plugin(codex_home.path(), "demo@market".to_string())
            .await
            .unwrap();

        assert!(!codex_home.path().join(CONFIG_TOML_FILE).exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn set_user_plugin_enabled_follows_config_symlink() {
        use std::os::unix::fs::symlink;

        let codex_home = TempDir::new().unwrap();
        let target_path = codex_home.path().join("target_config.toml");
        symlink(&target_path, codex_home.path().join(CONFIG_TOML_FILE)).unwrap();

        set_user_plugin_enabled(
            codex_home.path(),
            "demo@market".to_string(),
            /*enabled*/ true,
        )
        .await
        .unwrap();

        let config =
            toml::from_str::<toml::Value>(&fs::read_to_string(target_path).unwrap()).unwrap();
        let expected: toml::Value = toml::from_str(
            r#"
[plugins."demo@market"]
enabled = true
        "#,
        )
        .unwrap();
        assert_eq!(config, expected);
    }

    fn read_config(codex_home: &Path) -> toml::Value {
        toml::from_str(&fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).unwrap()).unwrap()
    }
}

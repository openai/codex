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

const CONFIG_WRITE_MAX_RETRIES: usize = 4;

/// Upserts user-level trusted hashes for hooks in a single config write.
///
/// Existing hook state fields, such as `enabled`, are preserved. If the same
/// hook key appears more than once, the last hash in the input wins.
pub async fn upsert_user_hook_trusted_hashes(
    codex_home: &Path,
    trusted_hashes: Vec<(String, String)>,
) -> std::io::Result<()> {
    upsert_hook_trusted_hashes(&codex_home.join(CONFIG_TOML_FILE), trusted_hashes).await
}

/// Upserts trusted hashes in the selected user config file.
pub async fn upsert_hook_trusted_hashes(
    config_path: &Path,
    trusted_hashes: Vec<(String, String)>,
) -> std::io::Result<()> {
    if trusted_hashes.is_empty() {
        return Ok(());
    }

    let config_path = config_path.to_path_buf();
    task::spawn_blocking(move || upsert_hook_trusted_hashes_blocking(&config_path, trusted_hashes))
        .await
        .map_err(|err| std::io::Error::other(format!("config persistence task panicked: {err}")))?
}

fn upsert_hook_trusted_hashes_blocking(
    config_path: &Path,
    trusted_hashes: Vec<(String, String)>,
) -> std::io::Result<()> {
    for _ in 0..CONFIG_WRITE_MAX_RETRIES {
        let write_paths = resolve_symlink_write_paths(config_path)?;
        let original = read_optional_string(write_paths.read_path.as_deref())?;
        let mut doc = parse_or_create_document(original.as_deref())?;
        let mut mutated = false;
        for (hook_key, trusted_hash) in &trusted_hashes {
            mutated |= set_hook_trusted_hash(&mut doc, hook_key, trusted_hash);
        }
        if !mutated {
            return Ok(());
        }
        if read_optional_string(write_paths.read_path.as_deref())? != original {
            continue;
        }
        return write_atomically(&write_paths.write_path, &doc.to_string());
    }
    Err(std::io::Error::new(
        ErrorKind::WouldBlock,
        "config changed repeatedly while persisting automatic hook trust",
    ))
}

fn read_optional_string(config_path: Option<&Path>) -> std::io::Result<Option<String>> {
    let Some(config_path) = config_path else {
        return Ok(None);
    };
    match fs::read_to_string(config_path) {
        Ok(raw) => Ok(Some(raw)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn parse_or_create_document(raw: Option<&str>) -> std::io::Result<DocumentMut> {
    match raw {
        Some(raw) => raw
            .parse::<DocumentMut>()
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err)),
        None => Ok(DocumentMut::new()),
    }
}

fn set_hook_trusted_hash(doc: &mut DocumentMut, hook_key: &str, trusted_hash: &str) -> bool {
    let Some(hooks) = ensure_child_table(doc.as_table_mut(), "hooks") else {
        return false;
    };
    let Some(state) = ensure_child_table(hooks, "state") else {
        return false;
    };
    let Some(hook_state) = ensure_table_for_write(&mut state[hook_key]) else {
        return false;
    };

    if hook_state.get("trusted_hash").and_then(TomlItem::as_str) == Some(trusted_hash) {
        return false;
    }

    let mut replacement = value(trusted_hash);
    if let Some(existing) = hook_state.get("trusted_hash") {
        preserve_decor(existing, &mut replacement);
    }
    hook_state["trusted_hash"] = replacement;
    true
}

fn ensure_child_table<'a>(table: &'a mut TomlTable, key: &str) -> Option<&'a mut TomlTable> {
    if !table.contains_key(key) {
        table.insert(key, TomlItem::Table(new_implicit_table()));
    }
    ensure_table_for_write(table.get_mut(key)?)
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
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn upserts_multiple_hook_trusted_hashes() {
        let codex_home = TempDir::new().unwrap();

        upsert_user_hook_trusted_hashes(
            codex_home.path(),
            vec![
                (
                    "plugin:pre_tool_use:0".to_string(),
                    "sha256:first".to_string(),
                ),
                ("plugin:stop:0".to_string(), "sha256:second".to_string()),
            ],
        )
        .await
        .unwrap();

        let config = read_config(codex_home.path());
        let expected: toml::Value = toml::from_str(
            r#"
[hooks.state."plugin:pre_tool_use:0"]
trusted_hash = "sha256:first"

[hooks.state."plugin:stop:0"]
trusted_hash = "sha256:second"
"#,
        )
        .unwrap();
        assert_eq!(config, expected);
    }

    #[tokio::test]
    async fn preserves_existing_hook_state_fields_and_formatting() {
        let codex_home = TempDir::new().unwrap();
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        fs::write(
            &config_path,
            r#"# leading comment
model = "o3"

[hooks.state."plugin:stop:0"]
enabled = false # keep disabled
trusted_hash = "sha256:old" # trust hash
custom = "preserved"
"#,
        )
        .unwrap();

        upsert_user_hook_trusted_hashes(
            codex_home.path(),
            vec![("plugin:stop:0".to_string(), "sha256:new".to_string())],
        )
        .await
        .unwrap();

        assert_eq!(
            fs::read_to_string(config_path).unwrap(),
            r#"# leading comment
model = "o3"

[hooks.state."plugin:stop:0"]
enabled = false # keep disabled
trusted_hash = "sha256:new" # trust hash
custom = "preserved"
"#
        );
    }

    #[tokio::test]
    async fn supports_existing_inline_hook_state() {
        let codex_home = TempDir::new().unwrap();
        fs::write(
            codex_home.path().join(CONFIG_TOML_FILE),
            r#"hooks = { state = { "plugin:stop:0" = { enabled = false } } }
"#,
        )
        .unwrap();

        upsert_user_hook_trusted_hashes(
            codex_home.path(),
            vec![("plugin:stop:0".to_string(), "sha256:new".to_string())],
        )
        .await
        .unwrap();

        let config = read_config(codex_home.path());
        let expected: toml::Value = toml::from_str(
            r#"
[hooks.state."plugin:stop:0"]
enabled = false
trusted_hash = "sha256:new"
"#,
        )
        .unwrap();
        assert_eq!(config, expected);
    }

    #[tokio::test]
    async fn empty_batch_does_not_create_or_read_config() {
        let codex_home = TempDir::new().unwrap();
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        fs::write(&config_path, "not valid toml = [").unwrap();

        upsert_user_hook_trusted_hashes(codex_home.path(), Vec::new())
            .await
            .unwrap();

        assert_eq!(
            fs::read_to_string(config_path).unwrap(),
            "not valid toml = ["
        );
    }

    #[tokio::test]
    async fn matching_hash_does_not_rewrite_config() {
        let codex_home = TempDir::new().unwrap();
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let original = r#"[hooks.state."plugin:stop:0"]
trusted_hash = "sha256:same"
"#;
        fs::write(&config_path, original).unwrap();

        upsert_user_hook_trusted_hashes(
            codex_home.path(),
            vec![("plugin:stop:0".to_string(), "sha256:same".to_string())],
        )
        .await
        .unwrap();

        assert_eq!(fs::read_to_string(config_path).unwrap(), original);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn follows_config_symlink() {
        use std::os::unix::fs::symlink;

        let codex_home = TempDir::new().unwrap();
        let target_path = codex_home.path().join("target_config.toml");
        symlink(&target_path, codex_home.path().join(CONFIG_TOML_FILE)).unwrap();

        upsert_user_hook_trusted_hashes(
            codex_home.path(),
            vec![("plugin:stop:0".to_string(), "sha256:new".to_string())],
        )
        .await
        .unwrap();

        assert!(codex_home.path().join(CONFIG_TOML_FILE).is_symlink());
        let config =
            toml::from_str::<toml::Value>(&fs::read_to_string(target_path).unwrap()).unwrap();
        let expected: toml::Value = toml::from_str(
            r#"
[hooks.state."plugin:stop:0"]
trusted_hash = "sha256:new"
"#,
        )
        .unwrap();
        assert_eq!(config, expected);
    }

    fn read_config(codex_home: &Path) -> toml::Value {
        toml::from_str(&fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).unwrap()).unwrap()
    }
}

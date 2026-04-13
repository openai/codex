use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::Value as TomlValue;
use toml_edit::value;

use crate::CONFIG_TOML_FILE;

pub struct MarketplaceConfigUpdate<'a> {
    pub last_updated: &'a str,
    pub source_type: &'a str,
    pub source: &'a str,
    pub ref_name: Option<&'a str>,
    pub sparse_paths: &'a [String],
}

pub fn record_user_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
    update: &MarketplaceConfigUpdate<'_>,
) -> std::io::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let mut doc = read_or_create_document(&config_path)?;
    upsert_marketplace(&mut doc, marketplace_name, update);
    fs::create_dir_all(codex_home)?;
    fs::write(config_path, doc.to_string())
}

pub fn remove_user_marketplace(codex_home: &Path, marketplace_name: &str) -> std::io::Result<bool> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let mut doc = match fs::read_to_string(&config_path) {
        Ok(raw) => raw
            .parse::<DocumentMut>()
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err))?,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    let removed = remove_marketplace(&mut doc, marketplace_name);
    if !removed {
        return Ok(false);
    }

    fs::create_dir_all(codex_home)?;
    fs::write(config_path, doc.to_string())?;
    Ok(true)
}

fn read_or_create_document(config_path: &Path) -> std::io::Result<DocumentMut> {
    match fs::read_to_string(config_path) {
        Ok(raw) => raw
            .parse::<DocumentMut>()
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(err) => Err(err),
    }
}

fn upsert_marketplace(
    doc: &mut DocumentMut,
    marketplace_name: &str,
    update: &MarketplaceConfigUpdate<'_>,
) {
    let root = doc.as_table_mut();
    if !root.contains_key("marketplaces") {
        root.insert("marketplaces", TomlItem::Table(new_implicit_table()));
    }

    let Some(marketplaces_item) = root.get_mut("marketplaces") else {
        return;
    };
    if !marketplaces_item.is_table() {
        *marketplaces_item = TomlItem::Table(new_implicit_table());
    }

    let Some(marketplaces) = marketplaces_item.as_table_mut() else {
        return;
    };
    let mut entry = TomlTable::new();
    entry.set_implicit(false);
    entry["last_updated"] = value(update.last_updated.to_string());
    entry["source_type"] = value(update.source_type.to_string());
    entry["source"] = value(update.source.to_string());
    if let Some(ref_name) = update.ref_name {
        entry["ref"] = value(ref_name.to_string());
    }
    if !update.sparse_paths.is_empty() {
        entry["sparse_paths"] = TomlItem::Value(TomlValue::Array(
            update.sparse_paths.iter().map(String::as_str).collect(),
        ));
    }
    marketplaces.insert(marketplace_name, TomlItem::Table(entry));
}

fn remove_marketplace(doc: &mut DocumentMut, marketplace_name: &str) -> bool {
    let root = doc.as_table_mut();
    let Some(marketplaces_item) = root.get_mut("marketplaces") else {
        return false;
    };
    let Some(marketplaces) = marketplaces_item.as_table_mut() else {
        return false;
    };
    if marketplaces.remove(marketplace_name).is_none() {
        return false;
    }
    if marketplaces.is_empty() {
        root.remove("marketplaces");
    }
    true
}

fn new_implicit_table() -> TomlTable {
    let mut table = TomlTable::new();
    table.set_implicit(true);
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn remove_user_marketplace_removes_requested_entry() {
        let codex_home = TempDir::new().unwrap();
        let update = MarketplaceConfigUpdate {
            last_updated: "2026-04-13T00:00:00Z",
            source_type: "git",
            source: "https://github.com/owner/repo.git",
            ref_name: Some("main"),
            sparse_paths: &[],
        };
        record_user_marketplace(codex_home.path(), "debug", &update).unwrap();
        record_user_marketplace(codex_home.path(), "other", &update).unwrap();

        let removed = remove_user_marketplace(codex_home.path(), "debug").unwrap();

        assert!(removed);
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap())
                .unwrap();
        let marketplaces = config
            .get("marketplaces")
            .and_then(toml::Value::as_table)
            .unwrap();
        assert_eq!(marketplaces.len(), 1);
        assert!(marketplaces.contains_key("other"));
    }

    #[test]
    fn remove_user_marketplace_returns_false_when_missing() {
        let codex_home = TempDir::new().unwrap();

        let removed = remove_user_marketplace(codex_home.path(), "debug").unwrap();

        assert!(!removed);
    }
}

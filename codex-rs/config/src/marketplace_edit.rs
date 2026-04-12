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

fn new_implicit_table() -> TomlTable {
    let mut table = TomlTable::new();
    table.set_implicit(true);
    table
}

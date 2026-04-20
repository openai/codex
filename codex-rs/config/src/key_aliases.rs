use toml::Value as TomlValue;
use toml::map::Map as TomlMap;

#[derive(Debug, Clone, Copy)]
struct ConfigKeyAlias {
    table_path: &'static [&'static str],
    legacy_key: &'static str,
    canonical_key: &'static str,
}

const CONFIG_KEY_ALIASES: &[ConfigKeyAlias] = &[ConfigKeyAlias {
    table_path: &["memories"],
    legacy_key: "no_memories_if_mcp_or_web_search",
    canonical_key: "disable_on_external_context",
}];

pub(crate) fn normalize_key_aliases(path: &[String], table: &mut TomlMap<String, TomlValue>) {
    for alias in CONFIG_KEY_ALIASES {
        if path
            .iter()
            .map(String::as_str)
            .eq(alias.table_path.iter().copied())
            && let Some(value) = table.remove(alias.legacy_key)
        {
            table
                .entry(alias.canonical_key.to_string())
                .or_insert(value);
        }
    }
}

pub(crate) fn normalized_with_key_aliases(value: &TomlValue, path: &[String]) -> TomlValue {
    match value {
        TomlValue::Table(table) => {
            let mut normalized = TomlMap::new();
            for (key, child) in table {
                let mut child_path = path.to_vec();
                child_path.push(key.clone());
                normalized.insert(key.clone(), normalized_with_key_aliases(child, &child_path));
            }
            normalize_key_aliases(path, &mut normalized);
            TomlValue::Table(normalized)
        }
        TomlValue::Array(items) => TomlValue::Array(
            items
                .iter()
                .map(|item| normalized_with_key_aliases(item, path))
                .collect(),
        ),
        _ => value.clone(),
    }
}

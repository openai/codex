use toml::Value as TomlValue;

pub(crate) fn default_empty_table() -> TomlValue {
    TomlValue::Table(Default::default())
}

pub fn build_cli_overrides_layer(cli_overrides: &[(String, TomlValue)]) -> TomlValue {
    let mut root = default_empty_table();
    for (path, value) in cli_overrides {
        apply_toml_override(&mut root, path, value.clone());
    }
    root
}

/// Apply a single dotted-path override onto a TOML value.
fn apply_toml_override(root: &mut TomlValue, path: &str, value: TomlValue) {
    use toml::value::Table;

    let mut current = root;
    let mut segments_iter = path.split('.').peekable();

    while let Some(segment) = segments_iter.next() {
        let is_last = segments_iter.peek().is_none();

        if is_last {
            match current {
                TomlValue::Table(table) => {
                    table.insert(segment.to_string(), value);
                }
                _ => {
                    let mut table = Table::new();
                    table.insert(segment.to_string(), value);
                    *current = TomlValue::Table(table);
                }
            }
            return;
        }

        match current {
            TomlValue::Table(table) => {
                current = table
                    .entry(segment.to_string())
                    .or_insert_with(|| TomlValue::Table(Table::new()));
            }
            _ => {
                *current = TomlValue::Table(Table::new());
                if let TomlValue::Table(tbl) = current {
                    current = tbl
                        .entry(segment.to_string())
                        .or_insert_with(|| TomlValue::Table(Table::new()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::merge_toml_values;
    use pretty_assertions::assert_eq;
    use toml::toml;

    #[test]
    fn build_cli_overrides_layer_preserves_empty_table_values() {
        let cli_overrides = vec![(
            "mcp_servers".to_string(),
            TomlValue::Table(toml::map::Map::new()),
        )];

        let layer = build_cli_overrides_layer(&cli_overrides);

        assert_eq!(layer, TomlValue::Table(toml! { mcp_servers = {} }));
    }

    #[test]
    fn empty_table_cli_override_clears_existing_map() {
        let cli_overrides = vec![(
            "mcp_servers".to_string(),
            TomlValue::Table(toml::map::Map::new()),
        )];
        let cli_layer = build_cli_overrides_layer(&cli_overrides);
        let mut merged = TomlValue::Table(toml! {
            [mcp_servers.docs]
            command = "uvx"
        });

        merge_toml_values(&mut merged, &cli_layer);

        assert_eq!(merged, TomlValue::Table(toml! { mcp_servers = {} }));
    }
}

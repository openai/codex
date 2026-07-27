use crate::merge::is_multi_agent_v2_feature_path;
use crate::merge::merge_toml_values;
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
    let mut traversed_segments = Vec::new();

    while let Some(segment) = segments_iter.next() {
        traversed_segments.push(segment);
        let is_last = segments_iter.peek().is_none();

        if is_last {
            match current {
                TomlValue::Table(table) => {
                    if is_multi_agent_v2_feature_path(&traversed_segments)
                        && let Some(existing) = table.get_mut(segment)
                    {
                        match (&mut *existing, &value) {
                            (TomlValue::Table(feature), TomlValue::Boolean(enabled)) => {
                                feature.insert("enabled".to_string(), TomlValue::Boolean(*enabled));
                                return;
                            }
                            (TomlValue::Boolean(enabled), TomlValue::Table(_)) => {
                                *existing = TomlValue::Table(Table::from_iter([(
                                    "enabled".to_string(),
                                    TomlValue::Boolean(*enabled),
                                )]));
                                merge_toml_values(existing, &value);
                                return;
                            }
                            (TomlValue::Table(_), TomlValue::Table(_)) => {
                                merge_toml_values(existing, &value);
                                return;
                            }
                            (
                                TomlValue::String(_)
                                | TomlValue::Integer(_)
                                | TomlValue::Float(_)
                                | TomlValue::Boolean(_)
                                | TomlValue::Datetime(_)
                                | TomlValue::Array(_)
                                | TomlValue::Table(_),
                                _,
                            ) => {}
                        }
                    }
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
                if is_multi_agent_v2_feature_path(&traversed_segments)
                    && let TomlValue::Boolean(enabled) = current
                {
                    *current = TomlValue::Table(Table::from_iter([(
                        "enabled".to_string(),
                        TomlValue::Boolean(*enabled),
                    )]));
                }
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

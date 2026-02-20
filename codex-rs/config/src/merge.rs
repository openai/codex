use toml::Value as TomlValue;

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        if overlay_table.is_empty() {
            base_table.clear();
            return;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use toml::toml;

    #[test]
    fn empty_overlay_table_replaces_existing_table() {
        let mut base = TomlValue::Table(toml! {
            [mcp_servers.docs]
            command = "uvx"

            [mcp_servers.logs]
            command = "tail"
        });
        let overlay = TomlValue::Table(toml! {
            mcp_servers = {}
        });

        merge_toml_values(&mut base, &overlay);

        assert_eq!(base, TomlValue::Table(toml! { mcp_servers = {} }));
    }

    #[test]
    fn empty_overlay_table_only_clears_targeted_nested_table() {
        let mut base = TomlValue::Table(toml! {
            [model_providers.default]
            name = "provider-a"

            [model_providers.default.extra]
            endpoint = "https://example-a"

            [model_providers.other]
            name = "provider-b"
        });
        let overlay = TomlValue::Table(toml! {
            model_providers = { default = {} }
        });

        merge_toml_values(&mut base, &overlay);

        assert_eq!(
            base,
            TomlValue::Table(toml! {
                model_providers = { default = {}, other = { name = "provider-b" } }
            })
        );
    }

    #[test]
    fn non_empty_overlay_table_keeps_recursive_merge_behavior() {
        let mut base = TomlValue::Table(toml! {
            [mcp_servers.docs]
            command = "uvx"
            disabled = false
        });
        let overlay = TomlValue::Table(toml! {
            mcp_servers = { docs = { disabled = true } }
        });

        merge_toml_values(&mut base, &overlay);

        assert_eq!(
            base,
            TomlValue::Table(toml! {
                mcp_servers = { docs = { command = "uvx", disabled = true } }
            })
        );
    }
}

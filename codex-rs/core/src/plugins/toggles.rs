use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

pub fn collect_plugin_enabled_candidates<'a>(
    edits: impl Iterator<Item = (&'a String, &'a JsonValue)>,
) -> BTreeMap<String, bool> {
    let mut pending_changes = BTreeMap::new();
    for (key_path, value) in edits {
        let segments = key_path
            .split('.')
            .map(str::to_string)
            .collect::<Vec<String>>();
        match segments.as_slice() {
            [plugins, plugin_id, enabled]
                if plugins == "plugins" && enabled == "enabled" && value.is_boolean() =>
            {
                if let Some(enabled) = value.as_bool() {
                    pending_changes.insert(plugin_id.clone(), enabled);
                }
            }
            [plugins, plugin_id] if plugins == "plugins" => {
                if let Some(enabled) = value.get("enabled").and_then(JsonValue::as_bool) {
                    pending_changes.insert(plugin_id.clone(), enabled);
                }
            }
            [plugins] if plugins == "plugins" => {
                let Some(entries) = value.as_object() else {
                    continue;
                };
                for (plugin_id, plugin_value) in entries {
                    let Some(enabled) = plugin_value.get("enabled").and_then(JsonValue::as_bool)
                    else {
                        continue;
                    };
                    pending_changes.insert(plugin_id.clone(), enabled);
                }
            }
            _ => {}
        }
    }

    pending_changes
}

#[cfg(test)]
mod tests {
    use super::collect_plugin_enabled_candidates;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn collect_plugin_enabled_candidates_tracks_direct_and_table_writes() {
        let candidates = collect_plugin_enabled_candidates(
            [
                (&"plugins.sample@test.enabled".to_string(), &json!(true)),
                (
                    &"plugins.other@test".to_string(),
                    &json!({ "enabled": false, "ignored": true }),
                ),
                (
                    &"plugins".to_string(),
                    &json!({
                        "nested@test": { "enabled": true },
                        "skip@test": { "name": "skip" },
                    }),
                ),
            ]
            .into_iter(),
        );

        assert_eq!(
            candidates,
            BTreeMap::from([
                ("nested@test".to_string(), true),
                ("other@test".to_string(), false),
                ("sample@test".to_string(), true),
            ])
        );
    }

    #[test]
    fn collect_plugin_enabled_candidates_uses_last_write_for_same_plugin() {
        let candidates = collect_plugin_enabled_candidates(
            [
                (&"plugins.sample@test.enabled".to_string(), &json!(true)),
                (
                    &"plugins.sample@test".to_string(),
                    &json!({ "enabled": false }),
                ),
            ]
            .into_iter(),
        );

        assert_eq!(
            candidates,
            BTreeMap::from([("sample@test".to_string(), false)])
        );
    }
}

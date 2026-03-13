use crate::config::ConfigToml;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::Path;

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

pub fn read_plugin_enabled_states(
    config_path: &Path,
    pending_changes: &BTreeMap<String, bool>,
) -> BTreeMap<String, Option<bool>> {
    let parsed_config = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|contents| toml::from_str::<ConfigToml>(&contents).ok());

    pending_changes
        .keys()
        .map(|plugin_id| {
            let enabled = parsed_config
                .as_ref()
                .and_then(|config| config.plugins.get(plugin_id))
                .map(|plugin| plugin.enabled);
            (plugin_id.clone(), enabled)
        })
        .collect()
}

pub fn plugin_toggle_events_to_emit(
    previous_states: &BTreeMap<String, Option<bool>>,
    updated_states: &BTreeMap<String, Option<bool>>,
    pending_changes: BTreeMap<String, bool>,
) -> Vec<(String, bool)> {
    pending_changes
        .into_iter()
        .filter_map(|(plugin_id, enabled)| {
            let previous_enabled = previous_states.get(&plugin_id).copied().flatten();
            let updated_enabled = updated_states.get(&plugin_id).copied().flatten();
            if previous_enabled == updated_enabled || updated_enabled != Some(enabled) {
                None
            } else {
                Some((plugin_id, enabled))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::collect_plugin_enabled_candidates;
    use super::plugin_toggle_events_to_emit;
    use super::read_plugin_enabled_states;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

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

    #[test]
    fn read_plugin_enabled_states_reads_plugin_table_values() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config_path = temp_dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            "[plugins.\"sample@test\"]\nenabled = true\n\n[plugins.\"other@test\"]\nenabled = false\n",
        )
        .expect("write config");

        let states = read_plugin_enabled_states(
            &config_path,
            &BTreeMap::from([
                ("missing@test".to_string(), true),
                ("other@test".to_string(), false),
                ("sample@test".to_string(), true),
            ]),
        );

        assert_eq!(
            states,
            BTreeMap::from([
                ("missing@test".to_string(), None),
                ("other@test".to_string(), Some(false)),
                ("sample@test".to_string(), Some(true)),
            ])
        );
    }

    #[test]
    fn plugin_toggle_events_to_emit_only_reports_real_state_transitions() {
        let previous_states = BTreeMap::from([
            ("disabled@test".to_string(), Some(false)),
            ("enabled@test".to_string(), Some(true)),
            ("missing@test".to_string(), None),
            ("stays_enabled@test".to_string(), Some(true)),
        ]);
        let updated_states = BTreeMap::from([
            ("disabled@test".to_string(), Some(true)),
            ("enabled@test".to_string(), Some(false)),
            ("missing@test".to_string(), None),
            ("stays_enabled@test".to_string(), Some(true)),
        ]);
        let pending_changes = BTreeMap::from([
            ("disabled@test".to_string(), true),
            ("enabled@test".to_string(), false),
            ("missing@test".to_string(), true),
            ("stays_enabled@test".to_string(), true),
        ]);

        let events =
            plugin_toggle_events_to_emit(&previous_states, &updated_states, pending_changes);

        assert_eq!(
            events,
            vec![
                ("disabled@test".to_string(), true),
                ("enabled@test".to_string(), false),
            ]
        );
    }
}

//! Tests source-aware plugin telemetry selection and complete-inventory bounds.

use crate::session::tests::make_session_and_context;
use codex_extension_api::SelectedPluginIdentity;
use codex_extension_api::SelectedPluginSnapshot;
use codex_utils_plugins::PluginIdentity;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn active_plugin_ids_select_and_validate_by_source() {
    let (_session, mut turn_context) = make_session_and_context().await;
    for (name, identities, selected_ids, expected) in [
        (
            "remote wins even when the package fallback is invalid",
            vec![
                ("box@local", Some("plugins~Plugin_box")),
                ("selected-root", Some("plugin_box-1")),
            ],
            vec![],
            Some(vec!["plugin_box-1", "plugins~Plugin_box"]),
        ),
        (
            "package fallback",
            vec![("my.tool@123", None)],
            vec!["executor-demo@1"],
            Some(vec!["executor-demo@1", "my.tool@123"]),
        ),
        (
            "blank present remote",
            vec![("box@local", Some(""))],
            vec![],
            None,
        ),
        (
            "remote is not trimmed",
            vec![("box@local", Some(" plugin_box "))],
            vec![],
            None,
        ),
        (
            "package key is not a remote ID",
            vec![("box@local", Some("box@local"))],
            vec![],
            None,
        ),
        (
            "remote-looking selected root is still a fallback",
            vec![],
            vec!["plugins~Plugin_box"],
            None,
        ),
        (
            "one invalid identity makes the inventory unknown",
            vec![("box@local", Some("plugin_box")), ("bad..name@1", None)],
            vec![],
            None,
        ),
        (
            "exact deduplication retains aliases and distinct remote objects",
            vec![
                ("box@local", Some("plugins~Plugin_b")),
                ("box@local", None),
                ("box@local", Some("plugins~Plugin_a")),
                ("box@other", Some("plugins~Plugin_a")),
            ],
            vec!["box@local", "box@local"],
            Some(vec!["box@local", "plugins~Plugin_a", "plugins~Plugin_b"]),
        ),
        ("empty observation", vec![], vec![], Some(vec![])),
    ] {
        let identities = identities
            .into_iter()
            .map(|(plugin_id, remote_plugin_id)| PluginIdentity {
                plugin_id: plugin_id.to_string(),
                remote_plugin_id: remote_plugin_id.map(str::to_string),
            })
            .collect();
        let selected_plugins = selected_ids
            .into_iter()
            .map(|plugin_id| SelectedPluginIdentity {
                selected_root_id: Some("capability-root".to_string()),
                plugin_id: plugin_id.to_string(),
            })
            .collect::<Vec<_>>();
        turn_context.active_host_plugin_identities = Some(identities);
        turn_context.extension_data.insert(SelectedPluginSnapshot {
            plugins: selected_plugins,
            ..Default::default()
        });
        assert_eq!(
            turn_context.active_plugin_ids_for_telemetry(),
            expected.map(|ids| ids.into_iter().map(str::to_string).collect::<Vec<_>>()),
            "{name}"
        );
    }
}

#[tokio::test]
async fn active_plugin_ids_enforce_bounds_after_selection_and_deduplication() {
    let (_session, mut turn_context) = make_session_and_context().await;
    for length in [128, 129] {
        for remote in [false, true] {
            let id = if remote {
                "r".repeat(length)
            } else {
                format!("{}@local", "p".repeat(length - 6))
            };
            // A remote ID also wins over an overlong package key.
            let plugin_id = if remote {
                format!("{}@local", "p".repeat(/*n*/ 128))
            } else {
                id.clone()
            };
            let identities = vec![PluginIdentity {
                plugin_id,
                remote_plugin_id: remote.then_some(id.clone()),
            }];
            turn_context.active_host_plugin_identities = Some(identities);
            turn_context
                .extension_data
                .insert(SelectedPluginSnapshot::default());
            assert_eq!(
                turn_context.active_plugin_ids_for_telemetry(),
                (length == 128).then_some(vec![id]),
                "length={length}, remote={remote}"
            );
        }
        let id = format!("{}@local", "p".repeat(length - 6));
        let selected_plugin = SelectedPluginIdentity {
            selected_root_id: Some("capability-root".to_string()),
            plugin_id: id.clone(),
        };
        turn_context.active_host_plugin_identities = Some(Vec::new());
        turn_context.extension_data.insert(SelectedPluginSnapshot {
            plugins: vec![selected_plugin],
            ..Default::default()
        });
        assert_eq!(
            turn_context.active_plugin_ids_for_telemetry(),
            (length == 128).then_some(vec![id]),
            "selected length={length}"
        );
    }

    for count in [512, 513] {
        let ids = (0..count)
            .map(|index| format!("plugin_{index:03}@local"))
            .collect::<Vec<_>>();
        let identities = ids[..count / 2]
            .iter()
            .map(|id| PluginIdentity {
                plugin_id: id.clone(),
                remote_plugin_id: None,
            })
            .collect();
        let selected_plugins = ids[count / 2 - 1..]
            .iter()
            .map(|id| SelectedPluginIdentity {
                selected_root_id: Some("capability-root".to_string()),
                plugin_id: id.clone(),
            })
            .collect::<Vec<_>>();
        turn_context.active_host_plugin_identities = Some(identities);
        turn_context.extension_data.insert(SelectedPluginSnapshot {
            plugins: selected_plugins,
            ..Default::default()
        });
        assert_eq!(
            turn_context.active_plugin_ids_for_telemetry(),
            (count == 512).then_some(ids),
            "{count} distinct IDs across host and selected inputs, with one overlap"
        );
    }
}

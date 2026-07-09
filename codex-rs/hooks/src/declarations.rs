use codex_plugin::PluginHookSource;
use codex_protocol::protocol::HookEventName;

/// Minimal declaration metadata for one bundled plugin hook handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHookDeclaration {
    pub key: String,
    pub event_name: HookEventName,
}

/// Persisted trust identity for one supported executable plugin command hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHookTrustEntry {
    pub key: String,
    pub current_hash: String,
}

/// Return the hook handlers declared by plugin bundles without projecting live runtime state.
pub fn plugin_hook_declarations(hook_sources: &[PluginHookSource]) -> Vec<PluginHookDeclaration> {
    let mut declarations = Vec::new();

    for source in hook_sources {
        let key_source = plugin_hook_key_source(
            source.plugin_id.as_key().as_str(),
            source.source_relative_path.as_str(),
        );
        for (event_name, groups) in source.hooks.clone().into_matcher_groups() {
            for (group_index, group) in groups.iter().enumerate() {
                for (handler_index, _) in group.hooks.iter().enumerate() {
                    declarations.push(PluginHookDeclaration {
                        key: crate::hook_key(&key_source, event_name, group_index, handler_index),
                        event_name,
                    });
                }
            }
        }
    }

    declarations
}

/// Return trust identities for supported executable plugin command hooks.
///
/// This projects runtime discovery results so filtering, platform command
/// selection, normalization, and hashing stay identical to hook execution.
pub fn plugin_hook_trust_entries(hook_sources: &[PluginHookSource]) -> Vec<PluginHookTrustEntry> {
    crate::engine::discovery::discover_handlers(
        /*config_layer_stack*/ None,
        hook_sources.to_vec(),
        /*plugin_hook_load_warnings*/ Vec::new(),
        /*bypass_hook_trust*/ false,
    )
    .hook_entries
    .into_iter()
    .map(|entry| PluginHookTrustEntry {
        key: entry.key,
        current_hash: entry.current_hash,
    })
    .collect()
}

pub(crate) fn plugin_hook_key_source(plugin_id: &str, source_relative_path: &str) -> String {
    format!("{plugin_id}:{source_relative_path}")
}

#[cfg(test)]
mod tests {
    use codex_config::HookEventsToml;
    use codex_config::HookHandlerConfig;
    use codex_config::MatcherGroup;
    use codex_plugin::PluginId;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn lists_declared_plugin_handlers_with_persisted_hook_keys() {
        let plugin_root = test_path_buf("/tmp/plugin").abs();
        let source_path = plugin_root.join("hooks/hooks.json");
        let declarations = plugin_hook_declarations(&[PluginHookSource {
            plugin_id: PluginId::parse("demo@test").expect("plugin id"),
            plugin_root: plugin_root.clone(),
            plugin_data_root: plugin_root.join("data"),
            source_path,
            source_relative_path: "hooks/hooks.json".to_string(),
            hooks: HookEventsToml {
                pre_tool_use: vec![MatcherGroup {
                    matcher: None,
                    hooks: vec![
                        HookHandlerConfig::Prompt {},
                        HookHandlerConfig::Command {
                            command: "echo hi".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: false,
                            status_message: None,
                        },
                    ],
                }],
                session_start: vec![MatcherGroup {
                    matcher: None,
                    hooks: vec![HookHandlerConfig::Agent {}],
                }],
                ..Default::default()
            },
        }]);

        assert_eq!(
            declarations,
            vec![
                PluginHookDeclaration {
                    key: "demo@test:hooks/hooks.json:pre_tool_use:0:0".to_string(),
                    event_name: HookEventName::PreToolUse,
                },
                PluginHookDeclaration {
                    key: "demo@test:hooks/hooks.json:pre_tool_use:0:1".to_string(),
                    event_name: HookEventName::PreToolUse,
                },
                PluginHookDeclaration {
                    key: "demo@test:hooks/hooks.json:session_start:0:0".to_string(),
                    event_name: HookEventName::SessionStart,
                },
            ]
        );
    }

    #[test]
    fn trust_entries_include_only_supported_executable_command_hooks() {
        let plugin_root = test_path_buf("/tmp/plugin").abs();
        let source_path = plugin_root.join("hooks/hooks.json");
        let entries = plugin_hook_trust_entries(&[PluginHookSource {
            plugin_id: PluginId::parse("demo@test").expect("plugin id"),
            plugin_root: plugin_root.clone(),
            plugin_data_root: plugin_root.join("data"),
            source_path,
            source_relative_path: "hooks/hooks.json".to_string(),
            hooks: HookEventsToml {
                pre_tool_use: vec![MatcherGroup {
                    matcher: Some("^Bash$".to_string()),
                    hooks: vec![
                        HookHandlerConfig::Command {
                            command: "echo supported".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: false,
                            status_message: None,
                        },
                        HookHandlerConfig::Command {
                            command: "echo async".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: true,
                            status_message: None,
                        },
                        HookHandlerConfig::Command {
                            command: "   ".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: false,
                            status_message: None,
                        },
                        HookHandlerConfig::Prompt {},
                        HookHandlerConfig::Agent {},
                    ],
                }],
                ..Default::default()
            },
        }]);

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].key,
            "demo@test:hooks/hooks.json:pre_tool_use:0:0"
        );
        assert!(!entries[0].current_hash.is_empty());
    }
}

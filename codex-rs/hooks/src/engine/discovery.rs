use std::fs;
use std::path::Path;

use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::HookEventsToml;
use codex_config::HookHandlerConfig;
use codex_config::HooksFile;
use codex_config::ManagedHooksRequirementsToml;
use codex_config::MatcherGroup;
use codex_config::RequirementSource;
use codex_plugin::PluginHookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;

use super::ConfiguredHandler;
use super::HookListEntry;
use crate::config_rules::disabled_hook_keys_from_stack;
use crate::events::common::matcher_pattern_for_event;
use crate::events::common::validate_matcher_pattern;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookSource;

pub(crate) struct DiscoveryResult {
    pub handlers: Vec<ConfiguredHandler>,
    pub hook_entries: Vec<HookListEntry>,
    pub warnings: Vec<String>,
}

struct HookHandlerSource<'a> {
    path: &'a AbsolutePathBuf,
    key_source: String,
    source: HookSource,
    disabled_hook_keys: &'a HashSet<String>,
    env: HashMap<String, String>,
    plugin_id: Option<String>,
}

pub(crate) fn discover_handlers(
    config_layer_stack: Option<&ConfigLayerStack>,
    plugin_hook_sources: Vec<PluginHookSource>,
    plugin_hook_load_warnings: Vec<String>,
) -> DiscoveryResult {
    let mut handlers = Vec::new();
    let mut hook_entries = Vec::new();
    let mut warnings = plugin_hook_load_warnings;
    let mut display_order = 0_i64;
    let disabled_hook_keys = disabled_hook_keys_from_stack(config_layer_stack);

    if let Some(config_layer_stack) = config_layer_stack {
        append_managed_requirement_handlers(
            &mut handlers,
            &mut hook_entries,
            &mut warnings,
            &mut display_order,
            config_layer_stack,
            &disabled_hook_keys,
        );

        for layer in config_layer_stack.get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        ) {
            let hook_source = hook_source_for_config_layer_source(&layer.name);
            let json_hooks = load_hooks_json(layer.config_folder().as_deref(), &mut warnings);
            let toml_hooks = load_toml_hooks_from_layer(layer, &mut warnings);

            if let (Some((json_source_path, json_events)), Some((toml_source_path, toml_events))) =
                (&json_hooks, &toml_hooks)
                && !json_events.is_empty()
                && !toml_events.is_empty()
            {
                warnings.push(format!(
                    "loading hooks from both {} and {}; prefer a single representation for this layer",
                    json_source_path.display(),
                    toml_source_path.display()
                ));
            }

            for (source_path, hook_events) in [json_hooks, toml_hooks].into_iter().flatten() {
                append_hook_events(
                    &mut handlers,
                    &mut hook_entries,
                    &mut warnings,
                    &mut display_order,
                    HookHandlerSource {
                        path: &source_path,
                        key_source: source_path.display().to_string(),
                        source: hook_source,
                        disabled_hook_keys: &disabled_hook_keys,
                        env: HashMap::new(),
                        plugin_id: None,
                    },
                    hook_events,
                );
            }
        }
    }

    append_plugin_hook_sources(
        &mut handlers,
        &mut hook_entries,
        &mut warnings,
        &mut display_order,
        plugin_hook_sources,
        &disabled_hook_keys,
    );

    DiscoveryResult {
        handlers,
        hook_entries,
        warnings,
    }
}

fn append_managed_requirement_handlers(
    handlers: &mut Vec<ConfiguredHandler>,
    hook_entries: &mut Vec<HookListEntry>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    config_layer_stack: &ConfigLayerStack,
    disabled_hook_keys: &HashSet<String>,
) {
    let Some(managed_hooks) = config_layer_stack.requirements().managed_hooks.as_ref() else {
        return;
    };
    let Some(source_path) =
        managed_hooks_source_path(managed_hooks.get(), managed_hooks.source.as_ref(), warnings)
    else {
        return;
    };
    append_hook_events(
        handlers,
        hook_entries,
        warnings,
        display_order,
        HookHandlerSource {
            path: &source_path,
            key_source: source_path.display().to_string(),
            source: hook_source_for_requirement_source(managed_hooks.source.as_ref()),
            disabled_hook_keys,
            env: HashMap::new(),
            plugin_id: None,
        },
        managed_hooks.get().hooks.clone(),
    );
}

fn append_plugin_hook_sources(
    handlers: &mut Vec<ConfiguredHandler>,
    hook_entries: &mut Vec<HookListEntry>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    plugin_hook_sources: Vec<PluginHookSource>,
    disabled_hook_keys: &HashSet<String>,
) {
    // TODO(abhinav): check enabled/trusted state here before plugin hooks become runnable.
    for source in plugin_hook_sources {
        let PluginHookSource {
            plugin_root,
            plugin_id,
            plugin_data_root,
            source_path,
            source_relative_path,
            hooks,
        } = source;
        let mut env = HashMap::new();
        let plugin_root_value = plugin_root.display().to_string();
        let plugin_data_root_value = plugin_data_root.display().to_string();
        env.insert("PLUGIN_ROOT".to_string(), plugin_root_value.clone());
        // For OOTB compat with existing plugins that use this env var.
        env.insert("CLAUDE_PLUGIN_ROOT".to_string(), plugin_root_value);
        env.insert("PLUGIN_DATA".to_string(), plugin_data_root_value.clone());
        // For OOTB compat with existing plugins that use this env var.
        env.insert("CLAUDE_PLUGIN_DATA".to_string(), plugin_data_root_value);
        let plugin_id = plugin_id.as_key();
        append_hook_events(
            handlers,
            hook_entries,
            warnings,
            display_order,
            HookHandlerSource {
                path: &source_path,
                key_source: format!("{plugin_id}:{source_relative_path}"),
                source: HookSource::Plugin,
                disabled_hook_keys,
                env,
                plugin_id: Some(plugin_id),
            },
            hooks,
        );
    }
}

fn managed_hooks_source_path(
    managed_hooks: &ManagedHooksRequirementsToml,
    requirement_source: Option<&RequirementSource>,
    warnings: &mut Vec<String>,
) -> Option<AbsolutePathBuf> {
    let source = requirement_source
        .map(ToString::to_string)
        .unwrap_or_else(|| "managed requirements".to_string());
    let Some(source_path) = managed_hooks.managed_dir_for_current_platform() else {
        warnings.push(format!(
            "skipping managed hooks from {source}: no managed hook directory is configured for this platform"
        ));
        return None;
    };

    if !source_path.is_absolute() {
        warnings.push(format!(
            "skipping managed hooks from {source}: managed hook directory {} is not absolute",
            source_path.display()
        ));
        None
    } else if !source_path.exists() {
        warnings.push(format!(
            "skipping managed hooks from {source}: managed hook directory {} does not exist",
            source_path.display()
        ));
        None
    } else if !source_path.is_dir() {
        warnings.push(format!(
            "skipping managed hooks from {source}: managed hook directory {} is not a directory",
            source_path.display()
        ));
        None
    } else {
        AbsolutePathBuf::from_absolute_path(source_path)
            .inspect_err(|err| {
                warnings.push(format!(
                    "skipping managed hooks from {source}: could not normalize managed hook directory {}: {err}",
                    source_path.display()
                ));
            })
            .ok()
    }
}

fn load_hooks_json(
    config_folder: Option<&Path>,
    warnings: &mut Vec<String>,
) -> Option<(AbsolutePathBuf, HookEventsToml)> {
    let source_path = config_folder?.join("hooks.json");
    if !source_path.as_path().is_file() {
        return None;
    }

    let contents = match fs::read_to_string(source_path.as_path()) {
        Ok(contents) => contents,
        Err(err) => {
            warnings.push(format!(
                "failed to read hooks config {}: {err}",
                source_path.display()
            ));
            return None;
        }
    };

    let parsed: HooksFile = match serde_json::from_str(&contents) {
        Ok(parsed) => parsed,
        Err(err) => {
            warnings.push(format!(
                "failed to parse hooks config {}: {err}",
                source_path.display()
            ));
            return None;
        }
    };

    let source_path = AbsolutePathBuf::from_absolute_path(&source_path)
        .inspect_err(|err| {
            warnings.push(format!(
                "failed to normalize hooks config path {}: {err}",
                source_path.display()
            ));
        })
        .ok()?;

    (!parsed.hooks.is_empty()).then_some((source_path, parsed.hooks))
}

fn load_toml_hooks_from_layer(
    layer: &ConfigLayerEntry,
    warnings: &mut Vec<String>,
) -> Option<(AbsolutePathBuf, HookEventsToml)> {
    let source_path = config_toml_source_path(layer);
    let hook_value = layer.config.get("hooks")?.clone();
    let parsed = match HookEventsToml::deserialize(hook_value) {
        Ok(parsed) => parsed,
        Err(err) => {
            warnings.push(format!(
                "failed to parse TOML hooks in {}: {err}",
                source_path.display()
            ));
            return None;
        }
    };

    (!parsed.is_empty()).then_some((source_path, parsed))
}

fn config_toml_source_path(layer: &ConfigLayerEntry) -> AbsolutePathBuf {
    match &layer.name {
        ConfigLayerSource::System { file }
        | ConfigLayerSource::User { file }
        | ConfigLayerSource::LegacyManagedConfigTomlFromFile { file } => file.clone(),
        ConfigLayerSource::Project { dot_codex_folder } => dot_codex_folder.join(CONFIG_TOML_FILE),
        ConfigLayerSource::Mdm { domain, key } => {
            synthetic_layer_path(&format!("<mdm:{domain}:{key}>/{CONFIG_TOML_FILE}"))
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {
            synthetic_layer_path("<legacy-managed-config.toml-mdm>/managed_config.toml")
        }
        ConfigLayerSource::SessionFlags => synthetic_layer_path("<session-flags>/config.toml"),
    }
}

fn synthetic_layer_path(path: &str) -> AbsolutePathBuf {
    #[cfg(windows)]
    {
        AbsolutePathBuf::resolve_path_against_base(path, r"C:\")
    }

    #[cfg(not(windows))]
    {
        AbsolutePathBuf::resolve_path_against_base(path, "/")
    }
}

fn append_hook_events(
    handlers: &mut Vec<ConfiguredHandler>,
    hook_entries: &mut Vec<HookListEntry>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source: HookHandlerSource<'_>,
    hook_events: HookEventsToml,
) {
    for (event_name, groups) in hook_events.into_matcher_groups() {
        append_matcher_groups(
            handlers,
            hook_entries,
            warnings,
            display_order,
            &source,
            event_name,
            groups,
        );
    }
}

fn append_matcher_groups(
    handlers: &mut Vec<ConfiguredHandler>,
    hook_entries: &mut Vec<HookListEntry>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source: &HookHandlerSource<'_>,
    event_name: codex_protocol::protocol::HookEventName,
    groups: Vec<MatcherGroup>,
) {
    for (group_index, group) in groups.into_iter().enumerate() {
        let matcher = matcher_pattern_for_event(event_name, group.matcher.as_deref());
        if let Some(matcher) = matcher
            && let Err(err) = validate_matcher_pattern(matcher)
        {
            warnings.push(format!(
                "invalid matcher {matcher:?} in {}: {err}",
                source.path.display()
            ));
            continue;
        }
        for (handler_index, handler) in group.hooks.into_iter().enumerate() {
            match handler {
                HookHandlerConfig::Command {
                    command,
                    timeout_sec,
                    r#async,
                    status_message,
                } => {
                    if r#async {
                        warnings.push(format!(
                            "skipping async hook in {}: async hooks are not supported yet",
                            source.path.display()
                        ));
                        continue;
                    }
                    if command.trim().is_empty() {
                        warnings.push(format!(
                            "skipping empty hook command in {}",
                            source.path.display()
                        ));
                        continue;
                    }
                    let command = source.env.iter().fold(command, |command, (key, value)| {
                        command.replace(&format!("${{{key}}}"), value)
                    });
                    let timeout_sec = timeout_sec.unwrap_or(600).max(1);
                    // TODO(abhinav): replace this positional suffix with a durable hook id.
                    let key = format!(
                        "{}:{}:{}:{}",
                        source.key_source,
                        hook_event_key_label(event_name),
                        group_index,
                        handler_index
                    );
                    let enabled =
                        source.source.is_managed() || !source.disabled_hook_keys.contains(&key);
                    hook_entries.push(HookListEntry {
                        key,
                        event_name,
                        handler_type: HookHandlerType::Command,
                        matcher: matcher.map(ToOwned::to_owned),
                        command: Some(command.clone()),
                        timeout_sec,
                        status_message: status_message.clone(),
                        source_path: source.path.clone(),
                        source: source.source,
                        plugin_id: source.plugin_id.clone(),
                        display_order: *display_order,
                        enabled,
                        is_managed: source.source.is_managed(),
                    });
                    if enabled {
                        handlers.push(ConfiguredHandler {
                            event_name,
                            matcher: matcher.map(ToOwned::to_owned),
                            command,
                            timeout_sec,
                            status_message,
                            source_path: source.path.clone(),
                            source: source.source,
                            display_order: *display_order,
                            env: source.env.clone(),
                        });
                    }
                    *display_order += 1;
                }
                HookHandlerConfig::Prompt {} => warnings.push(format!(
                    "skipping prompt hook in {}: prompt hooks are not supported yet",
                    source.path.display()
                )),
                HookHandlerConfig::Agent {} => warnings.push(format!(
                    "skipping agent hook in {}: agent hooks are not supported yet",
                    source.path.display()
                )),
            }
        }
    }
}

fn hook_event_key_label(event_name: codex_protocol::protocol::HookEventName) -> &'static str {
    match event_name {
        codex_protocol::protocol::HookEventName::PreToolUse => "pre_tool_use",
        codex_protocol::protocol::HookEventName::PermissionRequest => "permission_request",
        codex_protocol::protocol::HookEventName::PostToolUse => "post_tool_use",
        codex_protocol::protocol::HookEventName::SessionStart => "session_start",
        codex_protocol::protocol::HookEventName::UserPromptSubmit => "user_prompt_submit",
        codex_protocol::protocol::HookEventName::Stop => "stop",
    }
}

fn hook_source_for_config_layer_source(source: &ConfigLayerSource) -> HookSource {
    match source {
        ConfigLayerSource::System { .. } => HookSource::System,
        ConfigLayerSource::User { .. } => HookSource::User,
        ConfigLayerSource::Project { .. } => HookSource::Project,
        ConfigLayerSource::Mdm { .. } => HookSource::Mdm,
        ConfigLayerSource::SessionFlags => HookSource::SessionFlags,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. } => {
            HookSource::LegacyManagedConfigFile
        }
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm => HookSource::LegacyManagedConfigMdm,
    }
}

fn hook_source_for_requirement_source(source: Option<&RequirementSource>) -> HookSource {
    match source {
        Some(RequirementSource::MdmManagedPreferences { .. }) => HookSource::Mdm,
        Some(RequirementSource::SystemRequirementsToml { .. }) => HookSource::System,
        Some(RequirementSource::LegacyManagedConfigTomlFromFile { .. }) => {
            HookSource::LegacyManagedConfigFile
        }
        Some(RequirementSource::LegacyManagedConfigTomlFromMdm) => {
            HookSource::LegacyManagedConfigMdm
        }
        Some(RequirementSource::CloudRequirements) => HookSource::CloudRequirements,
        Some(RequirementSource::Unknown) | None => HookSource::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use codex_config::ConfigLayerEntry;
    use codex_config::ConfigLayerSource;
    use codex_config::HookEventsToml;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::ConfiguredHandler;
    use super::append_matcher_groups;
    use codex_config::HookHandlerConfig;
    use codex_config::MatcherGroup;
    use codex_config::TomlValue;

    fn source_path() -> AbsolutePathBuf {
        test_path_buf("/tmp/hooks.json").abs()
    }

    fn hook_source() -> HookSource {
        HookSource::User
    }

    fn hook_handler_source<'a>(
        path: &'a AbsolutePathBuf,
        disabled_hook_keys: &'a std::collections::HashSet<String>,
    ) -> super::HookHandlerSource<'a> {
        super::HookHandlerSource {
            path,
            key_source: path.display().to_string(),
            source: hook_source(),
            disabled_hook_keys,
            env: std::collections::HashMap::new(),
            plugin_id: None,
        }
    }

    fn command_group(matcher: Option<&str>) -> MatcherGroup {
        MatcherGroup {
            matcher: matcher.map(str::to_string),
            hooks: vec![HookHandlerConfig::Command {
                command: "echo hello".to_string(),
                timeout_sec: None,
                r#async: false,
                status_message: None,
            }],
        }
    }

    #[test]
    fn user_prompt_submit_ignores_invalid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;
        let source_path = source_path();
        let disabled_hook_keys = std::collections::HashSet::new();

        append_matcher_groups(
            &mut handlers,
            &mut Vec::new(),
            &mut warnings,
            &mut display_order,
            &hook_handler_source(&source_path, &disabled_hook_keys),
            HookEventName::UserPromptSubmit,
            vec![command_group(Some("["))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            handlers,
            vec![ConfiguredHandler {
                event_name: HookEventName::UserPromptSubmit,
                matcher: None,
                command: "echo hello".to_string(),
                timeout_sec: 600,
                status_message: None,
                source_path: source_path.clone(),
                source: hook_source(),
                display_order: 0,
                env: std::collections::HashMap::new(),
            }]
        );
    }

    #[test]
    fn pre_tool_use_keeps_valid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;
        let source_path = source_path();
        let disabled_hook_keys = std::collections::HashSet::new();

        append_matcher_groups(
            &mut handlers,
            &mut Vec::new(),
            &mut warnings,
            &mut display_order,
            &hook_handler_source(&source_path, &disabled_hook_keys),
            HookEventName::PreToolUse,
            vec![command_group(Some("^Bash$"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            handlers,
            vec![ConfiguredHandler {
                event_name: HookEventName::PreToolUse,
                matcher: Some("^Bash$".to_string()),
                command: "echo hello".to_string(),
                timeout_sec: 600,
                status_message: None,
                source_path: source_path.clone(),
                source: hook_source(),
                display_order: 0,
                env: std::collections::HashMap::new(),
            }]
        );
    }

    #[test]
    fn pre_tool_use_treats_star_matcher_as_match_all() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;
        let source_path = source_path();
        let disabled_hook_keys = std::collections::HashSet::new();

        append_matcher_groups(
            &mut handlers,
            &mut Vec::new(),
            &mut warnings,
            &mut display_order,
            &hook_handler_source(&source_path, &disabled_hook_keys),
            HookEventName::PreToolUse,
            vec![command_group(Some("*"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].matcher.as_deref(), Some("*"));
    }

    #[test]
    fn post_tool_use_keeps_valid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;
        let source_path = source_path();
        let disabled_hook_keys = std::collections::HashSet::new();

        append_matcher_groups(
            &mut handlers,
            &mut Vec::new(),
            &mut warnings,
            &mut display_order,
            &hook_handler_source(&source_path, &disabled_hook_keys),
            HookEventName::PostToolUse,
            vec![command_group(Some("Edit|Write"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].event_name, HookEventName::PostToolUse);
        assert_eq!(handlers[0].matcher.as_deref(), Some("Edit|Write"));
    }

    #[test]
    fn toml_hook_discovery_ignores_malformed_state_entries() {
        let layer = ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: test_path_buf("/tmp/config.toml").abs(),
            },
            config_with_malformed_state_and_session_start_hook(),
        );
        let mut warnings = Vec::new();

        let (_, hooks) = super::load_toml_hooks_from_layer(&layer, &mut warnings)
            .expect("valid hook events should still load");

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            hooks,
            HookEventsToml {
                session_start: vec![MatcherGroup {
                    matcher: None,
                    hooks: vec![HookHandlerConfig::Command {
                        command: "echo hello".to_string(),
                        timeout_sec: None,
                        r#async: false,
                        status_message: None,
                    }],
                }],
                ..Default::default()
            }
        );
    }

    fn config_with_malformed_state_and_session_start_hook() -> TomlValue {
        serde_json::from_value(serde_json::json!({
            "hooks": {
                "state": {
                    "some_key": {
                        "enabled": "not a bool",
                    },
                },
                "SessionStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo hello",
                    }],
                }],
            },
        }))
        .expect("config TOML should deserialize")
    }

    #[test]
    fn hook_source_for_config_layer_source_discards_source_details() {
        let config_file = test_path_buf("/tmp/.codex/config.toml").abs();
        let dot_codex_folder = test_path_buf("/tmp/worktree/.codex").abs();

        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::System {
                file: config_file.clone(),
            }),
            HookSource::System,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::User {
                file: config_file.clone(),
            }),
            HookSource::User,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::Project {
                dot_codex_folder
            }),
            HookSource::Project,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::Mdm {
                domain: "com.openai.codex".to_string(),
                key: "config".to_string(),
            }),
            HookSource::Mdm,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(&ConfigLayerSource::SessionFlags),
            HookSource::SessionFlags,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(
                &ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: config_file },
            ),
            HookSource::LegacyManagedConfigFile,
        );
        assert_eq!(
            super::hook_source_for_config_layer_source(
                &ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            ),
            HookSource::LegacyManagedConfigMdm,
        );
    }
}

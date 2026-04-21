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
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;

use super::ConfiguredHandler;
use crate::events::common::matcher_pattern_for_event;
use crate::events::common::validate_matcher_pattern;
use codex_protocol::protocol::HookSource;

pub(crate) struct DiscoveryResult {
    pub handlers: Vec<ConfiguredHandler>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy)]
struct HookHandlerSource<'a> {
    path: &'a AbsolutePathBuf,
    is_managed: bool,
    source: HookSource,
}

pub(crate) fn discover_handlers(config_layer_stack: Option<&ConfigLayerStack>) -> DiscoveryResult {
    let Some(config_layer_stack) = config_layer_stack else {
        return DiscoveryResult {
            handlers: Vec::new(),
            warnings: Vec::new(),
        };
    };

    let mut handlers = Vec::new();
    let mut warnings = Vec::new();
    let mut display_order = 0_i64;

    append_managed_requirement_handlers(
        &mut handlers,
        &mut warnings,
        &mut display_order,
        config_layer_stack,
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

        if let Some((source_path, hook_events)) = json_hooks {
            append_hook_events(
                &mut handlers,
                &mut warnings,
                &mut display_order,
                HookHandlerSource {
                    path: &source_path,
                    is_managed: false,
                    source: hook_source,
                },
                hook_events,
            );
        }

        if let Some((source_path, hook_events)) = toml_hooks {
            append_hook_events(
                &mut handlers,
                &mut warnings,
                &mut display_order,
                HookHandlerSource {
                    path: &source_path,
                    is_managed: false,
                    source: hook_source,
                },
                hook_events,
            );
        }
    }

    DiscoveryResult { handlers, warnings }
}

fn append_managed_requirement_handlers(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    config_layer_stack: &ConfigLayerStack,
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
        warnings,
        display_order,
        HookHandlerSource {
            path: &source_path,
            is_managed: true,
            source: hook_source_for_requirement_source(managed_hooks.source.as_ref()),
        },
        managed_hooks.get().hooks.clone(),
    );
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
        return None;
    }
    if !source_path.exists() {
        warnings.push(format!(
            "skipping managed hooks from {source}: managed hook directory {} does not exist",
            source_path.display()
        ));
        return None;
    }
    if !source_path.is_dir() {
        warnings.push(format!(
            "skipping managed hooks from {source}: managed hook directory {} is not a directory",
            source_path.display()
        ));
        return None;
    }

    AbsolutePathBuf::from_absolute_path(source_path)
        .inspect_err(|err| {
            warnings.push(format!(
                "skipping managed hooks from {source}: could not normalize managed hook directory {}: {err}",
                source_path.display()
            ));
        })
        .ok()
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
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source: HookHandlerSource<'_>,
    hook_events: HookEventsToml,
) {
    for (event_name, groups) in hook_events.into_matcher_groups() {
        append_matcher_groups(
            handlers,
            warnings,
            display_order,
            source,
            event_name,
            groups,
        );
    }
}

fn append_matcher_groups(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source: HookHandlerSource<'_>,
    event_name: codex_protocol::protocol::HookEventName,
    groups: Vec<MatcherGroup>,
) {
    for group in groups {
        append_group_handlers(
            handlers,
            warnings,
            display_order,
            source,
            event_name,
            matcher_pattern_for_event(event_name, group.matcher.as_deref()),
            group.hooks,
        );
    }
}

fn append_group_handlers(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source: HookHandlerSource<'_>,
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
    group_handlers: Vec<HookHandlerConfig>,
) {
    if let Some(matcher) = matcher
        && let Err(err) = validate_matcher_pattern(matcher)
    {
        warnings.push(format!(
            "invalid matcher {matcher:?} in {}: {err}",
            source.path.display()
        ));
        return;
    }

    for handler in group_handlers {
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
                let timeout_sec = timeout_sec.unwrap_or(600).max(1);
                handlers.push(ConfiguredHandler {
                    event_name,
                    is_managed: source.is_managed,
                    matcher: matcher.map(ToOwned::to_owned),
                    command,
                    timeout_sec,
                    status_message,
                    source_path: source.path.clone(),
                    source: source.source,
                    display_order: *display_order,
                });
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
        Some(RequirementSource::CloudRequirements | RequirementSource::Unknown) | None => {
            HookSource::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use codex_config::ConfigLayerSource;
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

    fn source_path() -> AbsolutePathBuf {
        test_path_buf("/tmp/hooks.json").abs()
    }

    fn hook_source() -> HookSource {
        HookSource::User
    }

    fn hook_handler_source(path: &AbsolutePathBuf) -> super::HookHandlerSource<'_> {
        super::HookHandlerSource {
            path,
            is_managed: false,
            source: hook_source(),
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

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            hook_handler_source(&source_path),
            HookEventName::UserPromptSubmit,
            vec![command_group(Some("["))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            handlers,
            vec![ConfiguredHandler {
                event_name: HookEventName::UserPromptSubmit,
                is_managed: false,
                matcher: None,
                command: "echo hello".to_string(),
                timeout_sec: 600,
                status_message: None,
                source_path: source_path.clone(),
                source: hook_source(),
                display_order: 0,
            }]
        );
    }

    #[test]
    fn pre_tool_use_keeps_valid_matcher_during_discovery() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;
        let source_path = source_path();

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            hook_handler_source(&source_path),
            HookEventName::PreToolUse,
            vec![command_group(Some("^Bash$"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            handlers,
            vec![ConfiguredHandler {
                event_name: HookEventName::PreToolUse,
                is_managed: false,
                matcher: Some("^Bash$".to_string()),
                command: "echo hello".to_string(),
                timeout_sec: 600,
                status_message: None,
                source_path: source_path.clone(),
                source: hook_source(),
                display_order: 0,
            }]
        );
    }

    #[test]
    fn pre_tool_use_treats_star_matcher_as_match_all() {
        let mut handlers = Vec::new();
        let mut warnings = Vec::new();
        let mut display_order = 0;
        let source_path = source_path();

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            hook_handler_source(&source_path),
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

        append_matcher_groups(
            &mut handlers,
            &mut warnings,
            &mut display_order,
            hook_handler_source(&source_path),
            HookEventName::PostToolUse,
            vec![command_group(Some("Edit|Write"))],
        );

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].event_name, HookEventName::PostToolUse);
        assert_eq!(handlers[0].matcher.as_deref(), Some("Edit|Write"));
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

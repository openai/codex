use std::fs;
use std::path::Path;

use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use regex::Regex;

use super::ConfiguredHandler;
use super::config::HookHandlerConfig;
use super::config::HooksFile;

pub(crate) struct DiscoveryResult {
    pub handlers: Vec<ConfiguredHandler>,
    pub warnings: Vec<String>,
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

    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        let Some(folder) = layer.config_folder() else {
            continue;
        };
        let source_path = match folder.join("hooks.json") {
            Ok(source_path) => source_path,
            Err(err) => {
                warnings.push(format!(
                    "failed to resolve hooks config path from {}: {err}",
                    folder.display()
                ));
                continue;
            }
        };
        if !source_path.as_path().is_file() {
            continue;
        }

        let contents = match fs::read_to_string(source_path.as_path()) {
            Ok(contents) => contents,
            Err(err) => {
                warnings.push(format!(
                    "failed to read hooks config {}: {err}",
                    source_path.display()
                ));
                continue;
            }
        };

        let parsed: HooksFile = match serde_json::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                warnings.push(format!(
                    "failed to parse hooks config {}: {err}",
                    source_path.display()
                ));
                continue;
            }
        };

        for group in parsed.hooks.session_start {
            append_group_handlers(
                &mut handlers,
                &mut warnings,
                &mut display_order,
                source_path.as_path(),
                codex_protocol::protocol::HookEventName::SessionStart,
                group.matcher.as_deref(),
                group.hooks,
            );
        }

        for group in parsed.hooks.stop {
            append_group_handlers(
                &mut handlers,
                &mut warnings,
                &mut display_order,
                source_path.as_path(),
                codex_protocol::protocol::HookEventName::Stop,
                /*matcher*/ None,
                group.hooks,
            );
        }
    }

    DiscoveryResult { handlers, warnings }
}

fn append_group_handlers(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
    group_handlers: Vec<HookHandlerConfig>,
) {
    if let Some(matcher) = matcher
        && let Err(err) = Regex::new(matcher)
    {
        warnings.push(format!(
            "invalid matcher {matcher:?} in {}: {err}",
            source_path.display()
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
                        source_path.display()
                    ));
                    continue;
                }
                if command.trim().is_empty() {
                    warnings.push(format!(
                        "skipping empty hook command in {}",
                        source_path.display()
                    ));
                    continue;
                }
                let timeout_sec = timeout_sec.unwrap_or(600).max(1);
                handlers.push(ConfiguredHandler {
                    event_name,
                    matcher: matcher.map(ToOwned::to_owned),
                    command,
                    timeout_sec,
                    status_message,
                    source_path: source_path.to_path_buf(),
                    display_order: *display_order,
                });
                *display_order += 1;
            }
            HookHandlerConfig::Prompt {} => warnings.push(format!(
                "skipping prompt hook in {}: prompt hooks are not supported yet",
                source_path.display()
            )),
            HookHandlerConfig::Agent {} => warnings.push(format!(
                "skipping agent hook in {}: agent hooks are not supported yet",
                source_path.display()
            )),
        }
    }
}

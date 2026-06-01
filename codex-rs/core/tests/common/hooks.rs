use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerStack;
use codex_config::TomlValue;
use codex_core::config::Config;
use codex_features::Feature;
use codex_hooks::HookListEntry;
use codex_utils_absolute_path::AbsolutePathBuf;

pub fn trust_discovered_hooks(config: &mut Config) {
    if let Err(err) = config.features.enable(Feature::CodexHooks) {
        panic!("test config should allow feature update: {err}");
    }

    let listed = codex_hooks::list_hooks(codex_hooks::HooksConfig {
        feature_enabled: true,
        config_layer_stack: Some(config.config_layer_stack.clone()),
        ..codex_hooks::HooksConfig::default()
    });
    assert!(
        !listed.hooks.is_empty(),
        "trusted hook fixture should discover at least one hook"
    );
    trust_hooks(config, listed.hooks);
}

pub fn trust_hooks(config: &mut Config, hooks: Vec<HookListEntry>) {
    config.config_layer_stack =
        trusted_config_layer_stack(&config.config_layer_stack, &config.codex_home, hooks);
}

pub fn trusted_config_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &AbsolutePathBuf,
    hooks: Vec<HookListEntry>,
) -> ConfigLayerStack {
    let mut user_config = config_layer_stack
        .get_active_user_layer()
        .map(|layer| layer.config.clone())
        .unwrap_or_else(|| TomlValue::Table(Default::default()));
    let Some(user_table) = user_config.as_table_mut() else {
        panic!("user config should be a table");
    };
    let Some(hooks_table) = user_table
        .entry("hooks")
        .or_insert_with(|| TomlValue::Table(Default::default()))
        .as_table_mut()
    else {
        panic!("hooks config should be a table");
    };
    let Some(state_table) = hooks_table
        .entry("state")
        .or_insert_with(|| TomlValue::Table(Default::default()))
        .as_table_mut()
    else {
        panic!("hook state config should be a table");
    };
    for hook in hooks {
        let mut hook_state = TomlValue::Table(Default::default());
        let Some(hook_state_table) = hook_state.as_table_mut() else {
            panic!("hook state should be a table");
        };
        hook_state_table.insert(
            "trusted_hash".to_string(),
            TomlValue::String(hook.current_hash),
        );
        state_table.insert(hook.key, hook_state);
    }

    config_layer_stack.with_user_config(&codex_home.join(CONFIG_TOML_FILE), user_config)
}

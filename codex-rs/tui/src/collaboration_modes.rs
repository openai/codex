use codex_core::config::CollaborationModeOverride;
use codex_core::config::Config;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;

fn mode_kind(mode: &CollaborationMode) -> ModeKind {
    match mode {
        CollaborationMode::Plan(_) => ModeKind::Plan,
        CollaborationMode::PairProgramming(_) => ModeKind::PairProgramming,
        CollaborationMode::Execute(_) => ModeKind::Execute,
        CollaborationMode::Custom(_) => ModeKind::Custom,
    }
}

pub(crate) fn default_mode(
    models_manager: &ModelsManager,
    config: &Config,
) -> Option<CollaborationMode> {
    let presets = presets(models_manager, config);
    presets
        .iter()
        .find(|preset| matches!(preset, CollaborationMode::PairProgramming(_)))
        .cloned()
        .or_else(|| presets.into_iter().next())
}

pub(crate) fn mode_for_kind(
    models_manager: &ModelsManager,
    config: &Config,
    kind: ModeKind,
    fallback_custom: Settings,
) -> Option<CollaborationMode> {
    if kind == ModeKind::Custom {
        return Some(custom_mode(config, fallback_custom));
    }
    let presets = presets(models_manager, config);
    presets.into_iter().find(|preset| mode_kind(preset) == kind)
}

pub(crate) fn same_variant(a: &CollaborationMode, b: &CollaborationMode) -> bool {
    mode_kind(a) == mode_kind(b)
}

/// Cycle to the next collaboration mode preset in list order.
pub(crate) fn next_mode(
    models_manager: &ModelsManager,
    config: &Config,
    current: &CollaborationMode,
) -> Option<CollaborationMode> {
    let presets = presets(models_manager, config);
    if presets.is_empty() {
        return None;
    }
    let current_kind = mode_kind(current);
    let next_index = presets
        .iter()
        .position(|preset| mode_kind(preset) == current_kind)
        .map_or(0, |idx| (idx + 1) % presets.len());
    presets.get(next_index).cloned()
}

fn presets(models_manager: &ModelsManager, config: &Config) -> Vec<CollaborationMode> {
    models_manager.list_collaboration_modes(Some(config))
}

fn custom_mode(config: &Config, fallback_custom: Settings) -> CollaborationMode {
    let override_cfg = config
        .collaboration_modes
        .as_ref()
        .and_then(|modes| modes.custom.as_ref());
    CollaborationMode::Custom(apply_override(fallback_custom, override_cfg))
}

fn apply_override(settings: Settings, override_cfg: Option<&CollaborationModeOverride>) -> Settings {
    let Some(override_cfg) = override_cfg else {
        return settings;
    };
    Settings {
        model: override_cfg.model.clone().unwrap_or(settings.model),
        reasoning_effort: override_cfg.reasoning_effort.or(settings.reasoning_effort),
        developer_instructions: override_cfg
            .developer_instructions
            .clone()
            .or(settings.developer_instructions),
    }
}

use codex_core::config::Config;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;

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
    let presets = models_manager.list_collaboration_modes(config);
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
) -> Option<CollaborationMode> {
    let presets = models_manager.list_collaboration_modes(config);
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
    let presets = models_manager.list_collaboration_modes(config);
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

pub(crate) fn execute_mode(
    models_manager: &ModelsManager,
    config: &Config,
) -> Option<CollaborationMode> {
    models_manager
        .list_collaboration_modes(config)
        .into_iter()
        .find(|preset| mode_kind(preset) == ModeKind::Execute)
}

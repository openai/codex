use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;

fn mode_kind(mode: &CollaborationMode) -> ModeKind {
    mode.mode
}

fn is_tui_mode(kind: ModeKind) -> bool {
    matches!(kind, ModeKind::Plan | ModeKind::Code)
}

fn filtered_presets(models_manager: &ModelsManager) -> Vec<CollaborationMode> {
    models_manager
        .list_collaboration_modes()
        .into_iter()
        .filter(|preset| is_tui_mode(mode_kind(preset)))
        .collect()
}

pub(crate) fn presets_for_tui(models_manager: &ModelsManager) -> Vec<CollaborationMode> {
    filtered_presets(models_manager)
}

pub(crate) fn default_mode(models_manager: &ModelsManager) -> Option<CollaborationMode> {
    let presets = filtered_presets(models_manager);
    presets
        .iter()
        .find(|preset| preset.mode == ModeKind::Code)
        .cloned()
        .or_else(|| presets.into_iter().next())
}

pub(crate) fn mode_for_kind(
    models_manager: &ModelsManager,
    kind: ModeKind,
) -> Option<CollaborationMode> {
    if !is_tui_mode(kind) {
        return None;
    }
    let presets = filtered_presets(models_manager);
    presets.into_iter().find(|preset| mode_kind(preset) == kind)
}

pub(crate) fn same_variant(a: &CollaborationMode, b: &CollaborationMode) -> bool {
    mode_kind(a) == mode_kind(b)
}

/// Cycle to the next collaboration mode preset in list order.
pub(crate) fn next_mode(
    models_manager: &ModelsManager,
    current: &CollaborationMode,
) -> Option<CollaborationMode> {
    let presets = filtered_presets(models_manager);
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

pub(crate) fn code_mode(models_manager: &ModelsManager) -> Option<CollaborationMode> {
    filtered_presets(models_manager)
        .into_iter()
        .find(|preset| mode_kind(preset) == ModeKind::Code)
}

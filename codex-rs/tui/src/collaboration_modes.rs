use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;

fn is_tui_mode(kind: ModeKind) -> bool {
    matches!(kind, ModeKind::Plan | ModeKind::Code | ModeKind::Custom)
}

fn filtered_presets(
    models_manager: &ModelsManager,
    custom_presets: &[CollaborationModeMask],
) -> Vec<CollaborationModeMask> {
    let mut presets: Vec<CollaborationModeMask> = models_manager
        .list_collaboration_modes()
        .into_iter()
        .filter(|mask| mask.mode.is_some_and(is_tui_mode))
        .collect();
    presets.extend(
        custom_presets
            .iter()
            .filter(|mask| mask.mode == Some(ModeKind::Custom))
            .cloned(),
    );
    presets
}

pub(crate) fn presets_for_tui(
    models_manager: &ModelsManager,
    custom_presets: &[CollaborationModeMask],
) -> Vec<CollaborationModeMask> {
    filtered_presets(models_manager, custom_presets)
}

pub(crate) fn default_mask(
    models_manager: &ModelsManager,
    custom_presets: &[CollaborationModeMask],
) -> Option<CollaborationModeMask> {
    let presets = filtered_presets(models_manager, custom_presets);
    presets
        .iter()
        .find(|mask| mask.mode == Some(ModeKind::Code))
        .cloned()
        .or_else(|| presets.into_iter().next())
}

pub(crate) fn mask_for_kind(
    models_manager: &ModelsManager,
    custom_presets: &[CollaborationModeMask],
    kind: ModeKind,
) -> Option<CollaborationModeMask> {
    if !is_tui_mode(kind) {
        return None;
    }
    filtered_presets(models_manager, custom_presets)
        .into_iter()
        .find(|mask| mask.mode == Some(kind))
}

/// Cycle to the next collaboration mode preset in list order.
pub(crate) fn next_mask(
    models_manager: &ModelsManager,
    custom_presets: &[CollaborationModeMask],
    current: Option<&CollaborationModeMask>,
) -> Option<CollaborationModeMask> {
    let presets = filtered_presets(models_manager, custom_presets);
    if presets.is_empty() {
        return None;
    }
    let current_name = current.map(|mask| mask.name.as_str());
    let next_index = presets
        .iter()
        .position(|mask| Some(mask.name.as_str()) == current_name)
        .map_or(0, |idx| (idx + 1) % presets.len());
    presets.get(next_index).cloned()
}

pub(crate) fn code_mask(
    models_manager: &ModelsManager,
    custom_presets: &[CollaborationModeMask],
) -> Option<CollaborationModeMask> {
    mask_for_kind(models_manager, custom_presets, ModeKind::Code)
}

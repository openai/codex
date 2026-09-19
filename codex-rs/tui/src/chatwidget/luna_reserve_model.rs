//! The Reserve-only /model picker uses normal-model metadata while keeping Reserve routing.

use super::*;
use crate::bottom_pane::popup_consts::picker_hint_line_for_keymap;
use crate::model_catalog::LUNA_MODEL;
use crate::model_catalog::LUNA_RESERVE_MODEL;

impl ChatWidget {
    pub(super) fn open_luna_reserve_model_popup(
        &mut self,
        presets: Vec<ModelPreset>,
        view_id: &'static str,
    ) {
        let normal_model_slug = self
            .rate_limit_snapshots_by_limit_id
            .values()
            .find(|snapshot| snapshot.limit_name == LUNA_RESERVE_MODEL)
            .and_then(|snapshot| snapshot.normal_model_slug.as_deref());
        let preset = normal_model_slug
            .and_then(|slug| presets.iter().find(|preset| preset.model == slug))
            .or_else(|| presets.iter().find(|preset| preset.model == LUNA_MODEL));
        let Some(mut preset) = preset.cloned() else {
            self.bottom_pane.dismiss_view_by_id(view_id);
            self.add_info_message(
                "Luna model settings are unavailable; please try /model again in a moment."
                    .to_string(),
                /*hint*/ None,
            );
            return;
        };

        // Only borrow the normal model's presentation and supported efforts. Selecting an
        // effort must keep the authorized Reserve model and the saved ordinary return target.
        preset.model = LUNA_RESERVE_MODEL.to_string();
        let single_supported_effort = preset.supported_reasoning_efforts.len() == 1;
        let name = preset.display_name.clone();
        let description = (!preset.description.is_empty()).then_some(preset.description.clone());
        let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::OpenReasoningPopup {
                model: preset.clone(),
            });
        })];
        let header = self.model_menu_header(
            "Select Model",
            "Other models return when ordinary usage is available again.",
        );
        self.show_model_selection_view(
            vec![LUNA_RESERVE_MODEL.to_string()],
            SelectionViewParams {
                view_id: Some(view_id),
                header,
                footer_hint: Some(picker_hint_line_for_keymap(&self.bottom_pane.list_keymap())),
                items: vec![SelectionItem {
                    name,
                    description,
                    is_current: true,
                    actions,
                    dismiss_on_select: single_supported_effort,
                    dismiss_parent_on_child_accept: !single_supported_effort,
                    ..Default::default()
                }],
                ..SelectionViewParams::picker()
            },
        );
    }
}

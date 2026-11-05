//! Accept current account-scoped model replies and refresh present pickers without reopening them.
//! Preserve highlighted models by wire slug, independent of their display names.

use super::model_popups::ALL_MODELS_SELECTION_VIEW_ID;
use super::model_popups::MODEL_OPTIONS_VIEW_ID;
use super::model_popups::MODEL_SELECTION_VIEW_ID;
use super::*;

impl ChatWidget {
    pub(crate) fn model_picker_unavailable(&self) -> Option<String> {
        match self.model_catalog.provider_write.try_lock() {
            Ok(changed) => changed.then(|| "The model list may be out of date. Restart the app server and TUI to refresh /model.".into()),
            Err(_) => Some("Saving the default provider. Try /model after the save finishes.".into()),
        }
    }

    pub(super) fn scope_model_picker_event(&self) -> impl Fn(AppEvent) -> AppEvent + use<> {
        let backend = self.model_catalog.backend;
        let thread_id = self.thread_id;
        move |event| match backend {
            Some(backend) => AppEvent::ScopedModelPicker {
                backend,
                thread_id,
                event: Box::new(event),
            },
            None => event,
        }
    }

    pub(crate) fn dismiss_stale_model_picker(&mut self) {
        self.model_popup_request_id = None;
        for id in [
            MODEL_OPTIONS_VIEW_ID,
            ALL_MODELS_SELECTION_VIEW_ID,
            MODEL_SELECTION_VIEW_ID,
        ] {
            while self.bottom_pane.dismiss_view_by_id(id) {}
        }
    }

    pub(super) fn refresh_account_scoped_models(&mut self) {
        if self.model_catalog.provider_may_have_changed() {
            return;
        }
        if self.status_account_display == Some(StatusAccountDisplay::ApiKey) {
            for model in &mut Arc::make_mut(&mut self.model_catalog).models {
                model.available_access_programs = None;
            }
        }
        let request_id = uuid::Uuid::new_v4();
        self.model_popup_request_id = Some(request_id);
        self.app_event_tx.send(AppEvent::FetchModels { request_id });
    }

    fn model_popup_view_id(&self) -> Option<&'static str> {
        [MODEL_SELECTION_VIEW_ID, ALL_MODELS_SELECTION_VIEW_ID]
            .into_iter()
            .find(|view_id| {
                self.bottom_pane
                    .selected_index_for_present_view(view_id)
                    .is_some()
            })
    }

    pub(crate) fn model_popup_request_is_current(&self, request_id: uuid::Uuid) -> bool {
        self.model_popup_request_id == Some(request_id)
    }

    pub(crate) fn on_models_loaded(
        &mut self,
        request_id: uuid::Uuid,
        result: Result<Vec<ModelPreset>, String>,
    ) -> bool {
        if self.model_catalog.provider_may_have_changed()
            || !self.model_popup_request_is_current(request_id)
        {
            return false;
        }
        self.model_popup_request_id = None;
        let Ok(presets) = result else {
            return false;
        };
        if presets.is_empty()
            || self.model_catalog.try_list_models().ok().as_ref() == Some(&presets)
        {
            return false;
        }
        Arc::make_mut(&mut self.model_catalog).models = presets;
        self.set_daybreak_enabled(self.daybreak_enabled);
        self.refresh_effective_service_tier();
        self.refresh_model_dependent_surfaces();
        true
    }

    /// Keep a present model picker aligned with the active task, including Reserve entry/exit.
    pub(super) fn refresh_open_model_picker(&mut self) {
        // Refresh an existing parent without reopening it or interrupting its reasoning child.
        match self.model_popup_view_id() {
            Some(MODEL_SELECTION_VIEW_ID) => self.open_model_popup_with_presets(
                self.model_catalog.try_list_models().unwrap_or_default(),
            ),
            Some(ALL_MODELS_SELECTION_VIEW_ID) => self.open_all_models_popup(),
            _ => {}
        }
    }

    pub(super) fn show_model_selection_view(
        &mut self,
        model_ids: Vec<String>,
        mut params: SelectionViewParams,
    ) {
        let selected_index = params
            .view_id
            .and_then(|view_id| self.bottom_pane.selected_index_for_present_view(view_id));
        let selected_model = selected_index.and_then(|index| self.model_popup_model_ids.get(index));
        params.initial_selected_idx = model_ids
            .iter()
            .position(|model| Some(model) == selected_model);
        self.model_popup_model_ids = model_ids;
        if let Some(view_id) = params.view_id.filter(|_| selected_index.is_some()) {
            self.bottom_pane
                .replace_selection_view_if_present(view_id, params);
        } else {
            self.bottom_pane.show_selection_view(params);
        }
    }
}

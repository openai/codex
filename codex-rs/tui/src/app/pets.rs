//! App-level handlers for ambient terminal pet events.

use super::*;

impl App {
    pub(super) fn handle_ambient_pet_image_render_error(
        &mut self,
        tui: &mut tui::Tui,
        err: crate::pets::PetImageRenderError,
    ) -> Result<()> {
        match err {
            crate::pets::PetImageRenderError::Terminal(err) => Err(err.into()),
            crate::pets::PetImageRenderError::Asset(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to render ambient pet image; disabling pet for session"
                );
                self.chat_widget.disable_ambient_pet_for_session();
                if let Err(clear_err) = tui.clear_ambient_pet_image() {
                    match clear_err {
                        crate::pets::PetImageRenderError::Terminal(err) => return Err(err.into()),
                        crate::pets::PetImageRenderError::Asset(err) => {
                            tracing::warn!(
                                error = %err,
                                "failed to clear ambient pet image after render failure"
                            );
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub(super) fn handle_pet_picker_preview_image_render_error(
        &mut self,
        tui: &mut tui::Tui,
        err: crate::pets::PetImageRenderError,
    ) -> Result<()> {
        match err {
            crate::pets::PetImageRenderError::Terminal(err) => Err(err.into()),
            crate::pets::PetImageRenderError::Asset(err) => {
                tracing::warn!(error = %err, "failed to render pet picker preview image");
                self.chat_widget
                    .fail_pet_picker_preview_render(err.to_string());
                if let Err(clear_err) = tui.draw_pet_picker_preview_image(/*request*/ None) {
                    match clear_err {
                        crate::pets::PetImageRenderError::Terminal(err) => return Err(err.into()),
                        crate::pets::PetImageRenderError::Asset(err) => {
                            tracing::warn!(
                                error = %err,
                                "failed to clear pet picker preview image after render failure"
                            );
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub(super) fn handle_pet_selected(&mut self, tui: &mut tui::Tui, pet_id: String) {
        let request_id = self.chat_widget.show_pet_selection_loading_popup();
        tui.frame_requester().schedule_frame();
        let codex_home = self.config.codex_home.clone();
        let frame_requester = tui.frame_requester();
        let animations_enabled = self.config.animations;
        let tx = self.app_event_tx.clone();
        std::mem::drop(tokio::task::spawn_blocking(move || {
            let result = crate::pets::ensure_builtin_pack_for_pet(&pet_id, &codex_home)
                .and_then(|()| {
                    crate::pets::AmbientPet::load(
                        Some(&pet_id),
                        &codex_home,
                        frame_requester,
                        animations_enabled,
                    )
                })
                .map(Some)
                .map_err(|err| err.to_string());
            tx.send(AppEvent::PetSelectionLoaded {
                request_id,
                pet_id,
                result,
            });
        }));
    }

    pub(super) async fn handle_pet_disabled(&mut self, tui: &mut tui::Tui) {
        let edit = crate::legacy_core::config::edit::tui_pet_edit(crate::pets::DISABLED_PET_ID);
        let apply_result = ConfigEditsBuilder::new(&self.config.codex_home)
            .with_edits([edit])
            .apply()
            .await;
        match apply_result {
            Ok(()) => {
                self.sync_tui_pet_disabled();
                tui.frame_requester().schedule_frame();
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to disable pets: {err}"));
            }
        }
    }

    pub(super) fn handle_pet_preview_loaded(
        &mut self,
        tui: &mut tui::Tui,
        request_id: u64,
        result: Result<crate::pets::AmbientPet, String>,
    ) {
        self.chat_widget
            .finish_pet_picker_preview_load(request_id, result);
        tui.frame_requester().schedule_frame();
    }

    pub(super) async fn handle_pet_selection_loaded(
        &mut self,
        tui: &mut tui::Tui,
        request_id: u64,
        pet_id: String,
        result: Result<Option<crate::pets::AmbientPet>, String>,
    ) -> Result<AppRunControl> {
        if !self
            .chat_widget
            .finish_pet_selection_loading_popup(request_id)
        {
            return Ok(AppRunControl::Continue);
        }
        match result {
            Ok(ambient_pet) => {
                let edit = crate::legacy_core::config::edit::tui_pet_edit(&pet_id);
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_edits([edit])
                    .apply()
                    .await
                {
                    Ok(()) => {
                        self.config.tui_pet = Some(pet_id.clone());
                        self.chat_widget
                            .set_tui_pet_loaded(Some(pet_id), ambient_pet);
                    }
                    Err(err) => {
                        self.chat_widget
                            .add_error_message(format!("Failed to save pet selection: {err}"));
                    }
                }
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to load pet: {err}"));
            }
        }
        tui.frame_requester().schedule_frame();
        Ok(AppRunControl::Continue)
    }

    pub(super) fn handle_configured_pet_loaded(
        &mut self,
        tui: &mut tui::Tui,
        pet_id: String,
        result: Result<Option<crate::pets::AmbientPet>, String>,
    ) {
        if self.config.tui_pet.as_deref() != Some(pet_id.as_str()) {
            return;
        }

        match result {
            Ok(ambient_pet) => {
                self.chat_widget
                    .set_tui_pet_loaded(Some(pet_id), ambient_pet);
                tui.frame_requester().schedule_frame();
            }
            Err(err) => {
                self.chat_widget
                    .add_warning_message(format!("Failed to load configured pet: {err}"));
            }
        }
    }
}

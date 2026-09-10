//! Retained composer setup and failed-dispatch recovery for the agent dashboard.
//! Refreshes rebind runtime settings without replacing drafts or editor state.

use super::*;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::chatwidget::UserMessage;

impl App {
    pub(super) fn sync_agents_overview_composer(&self) {
        let mut state = self
            .agents_overview
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let composer = state.composer.get_or_insert_with(|| {
            let mut composer = ChatComposer::new_with_config(
                /*has_input_focus*/ true,
                self.app_event_tx.clone(),
                self.enhanced_keys_supported,
                "Describe a new task".to_string(),
                self.config.disable_paste_burst,
                ChatComposerConfig {
                    trim_submission: false,
                    image_paste_enabled: true,
                    ..ChatComposerConfig::plain_text()
                },
            );
            // A new task has no context usage to report yet.
            composer.set_context_window_pending(/*pending*/ true);
            composer.set_footer_hint_override(Some(Vec::new()));
            composer
        });
        composer.set_app_event_sender(self.app_event_tx.clone());
        composer.set_disable_paste_burst(self.config.disable_paste_burst);
        let vim_enabled = self.chat_widget.composer_is_vim_enabled();
        if composer.is_vim_enabled() != vim_enabled {
            composer.set_vim_enabled(vim_enabled);
            composer.resume_text_entry();
        }
        composer.set_keymap_bindings(&self.keymap);
    }

    pub(super) fn restore_agents_overview_prompt(&mut self, prompt: UserMessage) {
        if let Ok(mut state) = self.agents_overview.view_state.lock()
            && let Some(composer) = state.composer.as_mut()
            && composer.is_empty()
            && !composer.is_in_paste_burst()
            && !composer.popup_active()
        {
            composer.set_text_content(
                prompt.text,
                prompt.text_elements,
                prompt
                    .local_images
                    .into_iter()
                    .map(|image| image.path)
                    .collect(),
            );
            composer.move_cursor_to_end();
            return;
        }
        let hint = (!prompt.local_images.is_empty()).then(|| {
            format!(
                "Reattach images from: {}",
                prompt
                    .local_images
                    .iter()
                    .map(|image| image.path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
        let mut text = prompt.text.clone();
        if let Some(hint) = &hint {
            text.push_str("\n\n");
            text.push_str(hint);
        }
        self.agents_overview.unsent_prompt = Some(text);
        self.chat_widget
            .add_info_message(format!("Unsent task: {}", prompt.text), hint);
    }
}

//! Attachment bookkeeping for the chat composer, including local image placeholders,
//! remote-image rows, and keyboard selection over those rows.

use std::collections::HashSet;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::style::Stylize;
use ratatui::text::Line;

use super::InputResult;
use crate::bottom_pane::LocalImageAttachment;
use crate::bottom_pane::textarea::TextArea;
use codex_protocol::models::local_image_label_text;
use codex_protocol::user_input::TextElement;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AttachedImage {
    pub(super) placeholder: String,
    pub(super) path: PathBuf,
}

#[derive(Debug, Default)]
pub(super) struct AttachmentState {
    pub(super) local_images: Vec<AttachedImage>,
    pub(super) remote_image_urls: Vec<String>,
    pub(super) selected_remote_image_index: Option<usize>,
}

impl AttachmentState {
    pub(super) fn is_empty(&self) -> bool {
        self.local_images.is_empty() && self.remote_image_urls.is_empty()
    }

    pub(super) fn local_image_paths(&self) -> Vec<PathBuf> {
        self.local_images
            .iter()
            .map(|image| image.path.clone())
            .collect()
    }

    pub(super) fn local_images(&self) -> Vec<LocalImageAttachment> {
        self.local_images
            .iter()
            .map(|image| LocalImageAttachment {
                placeholder: image.placeholder.clone(),
                path: image.path.clone(),
            })
            .collect()
    }

    pub(super) fn set_remote_image_urls(&mut self, urls: Vec<String>, textarea: &mut TextArea) {
        self.remote_image_urls = urls;
        self.selected_remote_image_index = None;
        self.relabel_local_images(textarea);
    }

    pub(super) fn remote_image_urls(&self) -> Vec<String> {
        self.remote_image_urls.clone()
    }

    pub(super) fn take_remote_image_urls(&mut self, textarea: &mut TextArea) -> Vec<String> {
        let urls = std::mem::take(&mut self.remote_image_urls);
        self.selected_remote_image_index = None;
        self.relabel_local_images(textarea);
        urls
    }

    pub(super) fn clear_remote_image_urls(&mut self) {
        self.remote_image_urls.clear();
        self.selected_remote_image_index = None;
    }

    pub(super) fn reset_local_images(
        &mut self,
        local_image_paths: Vec<PathBuf>,
        textarea: &mut TextArea,
    ) {
        self.local_images.clear();
        self.local_images.extend(
            local_image_paths
                .into_iter()
                .enumerate()
                .map(|(index, path)| AttachedImage {
                    placeholder: local_image_label_text(self.remote_image_urls.len() + index + 1),
                    path,
                }),
        );
        self.selected_remote_image_index = None;
        self.relabel_local_images(textarea);
    }

    pub(super) fn attach_image(&mut self, textarea: &mut TextArea, path: PathBuf) {
        let image_number = self.remote_image_urls.len() + self.local_images.len() + 1;
        let placeholder = local_image_label_text(image_number);
        textarea.insert_element(&placeholder);
        self.local_images.push(AttachedImage { placeholder, path });
    }

    pub(super) fn prune_local_images_for_submission(
        &mut self,
        text: &str,
        text_elements: &[TextElement],
    ) {
        if self.local_images.is_empty() {
            return;
        }

        let image_placeholders: HashSet<&str> = text_elements
            .iter()
            .filter_map(|element| element.placeholder(text))
            .collect();
        self.local_images
            .retain(|image| image_placeholders.contains(image.placeholder.as_str()));
    }

    #[cfg(test)]
    pub(super) fn take_recent_submission_images(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.local_images)
            .into_iter()
            .map(|image| image.path)
            .collect()
    }

    pub(super) fn take_recent_submission_images_with_placeholders(
        &mut self,
    ) -> Vec<LocalImageAttachment> {
        std::mem::take(&mut self.local_images)
            .into_iter()
            .map(|image| LocalImageAttachment {
                placeholder: image.placeholder,
                path: image.path,
            })
            .collect()
    }

    pub(super) fn remote_image_lines(&self) -> Vec<Line<'static>> {
        self.remote_image_urls
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let label = local_image_label_text(index + 1);
                if self.selected_remote_image_index == Some(index) {
                    label.cyan().reversed().into()
                } else {
                    label.cyan().into()
                }
            })
            .collect()
    }

    pub(super) fn clear_remote_image_selection(&mut self) {
        self.selected_remote_image_index = None;
    }

    pub(super) fn handle_remote_image_selection_key(
        &mut self,
        key_event: &KeyEvent,
        textarea: &mut TextArea,
    ) -> Option<(InputResult, bool)> {
        if self.remote_image_urls.is_empty()
            || key_event.modifiers != KeyModifiers::NONE
            || key_event.kind != KeyEventKind::Press
        {
            return None;
        }

        match key_event.code {
            KeyCode::Up => {
                if let Some(selected) = self.selected_remote_image_index {
                    self.selected_remote_image_index = Some(selected.saturating_sub(1));
                    Some((InputResult::None, true))
                } else if textarea.cursor() == 0 {
                    self.selected_remote_image_index = Some(self.remote_image_urls.len() - 1);
                    Some((InputResult::None, true))
                } else {
                    None
                }
            }
            KeyCode::Down => {
                if let Some(selected) = self.selected_remote_image_index {
                    if selected + 1 < self.remote_image_urls.len() {
                        self.selected_remote_image_index = Some(selected + 1);
                    } else {
                        self.clear_remote_image_selection();
                    }
                    Some((InputResult::None, true))
                } else {
                    None
                }
            }
            KeyCode::Delete | KeyCode::Backspace => {
                if let Some(selected) = self.selected_remote_image_index {
                    self.remove_selected_remote_image(selected, textarea);
                    Some((InputResult::None, true))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(super) fn remove_deleted_local_placeholders(
        &mut self,
        removed_payloads: &[String],
        textarea: &mut TextArea,
    ) -> bool {
        let previous_len = self.local_images.len();
        self.local_images.retain(|image| {
            !removed_payloads
                .iter()
                .any(|payload| payload == &image.placeholder)
        });
        let removed_any = self.local_images.len() != previous_len;
        if removed_any {
            self.relabel_local_images(textarea);
        }
        removed_any
    }

    pub(super) fn relabel_local_images(&mut self, textarea: &mut TextArea) {
        for (index, image) in self.local_images.iter_mut().enumerate() {
            let expected = local_image_label_text(self.remote_image_urls.len() + index + 1);
            if image.placeholder == expected {
                continue;
            }

            let current = std::mem::replace(&mut image.placeholder, expected.clone());
            let _renamed = textarea.replace_element_payload(&current, &expected);
        }
    }

    fn remove_selected_remote_image(&mut self, selected_index: usize, textarea: &mut TextArea) {
        if selected_index >= self.remote_image_urls.len() {
            self.clear_remote_image_selection();
            return;
        }

        self.remote_image_urls.remove(selected_index);
        self.selected_remote_image_index = if self.remote_image_urls.is_empty() {
            None
        } else {
            Some(selected_index.min(self.remote_image_urls.len() - 1))
        };
        self.relabel_local_images(textarea);
    }
}

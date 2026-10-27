//! Composer payloads for the textarea's single-entry register. Copy captures original targets;
//! paste allocates fresh element identities and placeholder labels, without re-resolving targets.
//! Captured Ctrl-Y actions retain their original register generation through dot-repeat and undo.
//! The live register travels with composer replacement; repeat/undo generations stay local.
//! Like the text register, this state survives draft clears and undo, but is not a system clipboard.

use crate::bottom_pane::textarea::TextElementSnapshot;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Weak;

use super::ChatComposer;
use super::attachment_state::AttachedImage;
use super::draft_state::ComposerMentionBinding;
use crate::bottom_pane::textarea::register::RegisterEvent;
use codex_protocol::models::local_image_label_text;

#[derive(Clone)]
enum RegisterPayload {
    Mention(ComposerMentionBinding),
    Image(PathBuf),
    Paste(String),
}

type RegisterGeneration = (
    Weak<Vec<TextElementSnapshot>>,
    HashMap<u64, RegisterPayload>,
);

#[derive(Default)]
pub(super) struct VimRegister {
    payloads: Vec<RegisterGeneration>,
}

/// The live register only, paired with the exact range generation that owns its payloads.
pub(crate) struct KillBufferSnapshot {
    textarea: crate::bottom_pane::textarea::KillBufferSnapshot,
    payloads: HashMap<u64, RegisterPayload>,
}

pub(super) fn take_snapshot(composer: &mut ChatComposer) -> KillBufferSnapshot {
    sync(composer);
    let textarea = composer.draft.textarea.take_kill_buffer_snapshot();
    let source = Arc::downgrade(&textarea.elements);
    let payloads = composer
        .vim_register
        .payloads
        .iter()
        .find(|(key, _)| key.ptr_eq(&source))
        .map(|(_, payloads)| payloads.clone())
        .unwrap_or_default();
    KillBufferSnapshot { textarea, payloads }
}

pub(super) fn restore_snapshot(composer: &mut ChatComposer, snapshot: KillBufferSnapshot) {
    composer.vim_register.payloads.push((
        Arc::downgrade(&snapshot.textarea.elements),
        snapshot.payloads,
    ));
    composer
        .draft
        .textarea
        .restore_kill_buffer_snapshot(snapshot.textarea);
}

pub(super) fn sync(composer: &mut ChatComposer) {
    // Generations stay alive only while the live register or a repeat transaction needs them.
    composer
        .vim_register
        .payloads
        .retain(|(source, _)| source.strong_count() > 0);
    if let Some(event) = composer.draft.textarea.take_register_event() {
        match event {
            RegisterEvent::Copied(elements) => {
                let payloads = elements
                    .iter()
                    .filter_map(|element| {
                        let payload = if let Some(binding) =
                            composer.draft.mention_bindings.get(&element.id)
                        {
                            RegisterPayload::Mention(binding.clone())
                        } else if let Some(image) = composer
                            .attachments
                            .local_images
                            .iter()
                            .find(|image| image.placeholder == element.text)
                        {
                            RegisterPayload::Image(image.path.clone())
                        } else if let Some((_, pasted)) = composer
                            .draft
                            .pending_pastes
                            .iter()
                            .find(|(placeholder, _)| *placeholder == element.text)
                        {
                            RegisterPayload::Paste(pasted.clone())
                        } else {
                            return None;
                        };
                        Some((element.id, payload))
                    })
                    .collect();
                composer
                    .vim_register
                    .payloads
                    .push((Arc::downgrade(&elements), payloads));
            }
            RegisterEvent::Pasted(elements, ids) => {
                let source = Arc::downgrade(&elements);
                let Some((_, payloads)) = composer
                    .vim_register
                    .payloads
                    .iter()
                    .find(|(key, _)| key.ptr_eq(&source))
                else {
                    return;
                };
                let payloads: Vec<_> = ids
                    .into_iter()
                    .filter_map(|(source, destination)| {
                        payloads
                            .get(&source)
                            .cloned()
                            .map(|payload| (destination, payload))
                    })
                    .collect();
                for (destination, payload) in payloads {
                    match payload {
                        RegisterPayload::Mention(binding) => {
                            composer.draft.mention_bindings.insert(destination, binding);
                        }
                        RegisterPayload::Image(path) => {
                            let placeholder = local_image_label_text(
                                composer.attachments.remote_image_urls.len()
                                    + composer.attachments.local_images.len()
                                    + 1,
                            );
                            composer
                                .draft
                                .textarea
                                .replace_element_payload_by_id(destination, &placeholder);
                            composer
                                .attachments
                                .local_images
                                .push(AttachedImage { placeholder, path });
                        }
                        RegisterPayload::Paste(pasted) => {
                            let placeholder =
                                composer.next_large_paste_placeholder(pasted.chars().count());
                            composer
                                .draft
                                .textarea
                                .replace_element_payload_by_id(destination, &placeholder);
                            composer.draft.pending_pastes.push((placeholder, pasted));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "vim_register_tests.rs"]
mod tests;

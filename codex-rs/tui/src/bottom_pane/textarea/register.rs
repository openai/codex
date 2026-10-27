//! Atomic ranges in the single-entry register. The composer resolves copied element IDs into
//! rich payloads and attaches them to fresh IDs after each paste, before pruning deleted elements.

use super::TextArea;
use super::TextElementSnapshot;
use std::ops::Range;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct RegisterElements {
    pub(super) elements: Arc<Vec<TextElementSnapshot>>,
    pub(super) event: Option<RegisterEvent>,
}

pub(in crate::bottom_pane) enum RegisterEvent {
    Copied(Arc<Vec<TextElementSnapshot>>),
    Pasted(Arc<Vec<TextElementSnapshot>>, Vec<(u64, u64)>),
}

pub(super) struct RegisterSnapshot {
    pub(super) text: String,
    pub(super) elements: Arc<Vec<TextElementSnapshot>>,
}

pub(super) fn copy(textarea: &mut TextArea, range: Range<usize>) {
    let elements: Vec<_> = textarea
        .text_element_snapshots()
        .into_iter()
        .filter(|element| range.start <= element.range.start && element.range.end <= range.end)
        .map(|mut element| {
            element.range = element.range.start - range.start..element.range.end - range.start;
            element
        })
        .collect();
    let elements = Arc::new(elements);
    textarea.register_elements.elements = elements.clone();
    textarea.register_elements.event = Some(RegisterEvent::Copied(elements));
}

pub(super) fn paste(textarea: &mut TextArea, start: usize) {
    paste_elements(textarea, start, textarea.register_elements.elements.clone());
}

pub(super) fn paste_elements(
    textarea: &mut TextArea,
    start: usize,
    elements: Arc<Vec<TextElementSnapshot>>,
) {
    let mut ids = Vec::with_capacity(elements.len());
    for element in elements.iter() {
        let range = start + element.range.start..start + element.range.end;
        if textarea.text.get(range.clone()) == Some(element.text.as_str())
            && let Some(id) = textarea.add_element_range(range)
        {
            ids.push((element.id, id));
        }
    }
    if !ids.is_empty() {
        textarea.register_elements.event = Some(RegisterEvent::Pasted(elements, ids));
    }
}

impl std::fmt::Debug for RegisterElements {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterElements").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for RegisterSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterSnapshot").finish_non_exhaustive()
    }
}

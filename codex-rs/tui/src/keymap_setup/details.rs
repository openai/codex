//! Selected-action details for the `/keymap` picker.

use std::sync::Arc;
use std::sync::Mutex;

use codex_config::types::TuiKeymap;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::keymap::RuntimeKeymap;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;

use super::actions::KEYMAP_ACTIONS;
use super::actions::action_label;
use super::actions::bindings_for_action;
use super::actions::format_binding_summary;
use super::has_custom_binding;

#[derive(Clone, Debug)]
pub(super) struct KeymapActionDetail {
    context: String,
    context_label: String,
    action: String,
    label: String,
    description: String,
    binding_summary: String,
    custom_binding: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum KeymapActionDetailsLayout {
    Wide,
    NarrowFooter,
}

#[derive(Clone)]
pub(super) struct KeymapActionDetailsRenderable {
    details: Arc<Vec<KeymapActionDetail>>,
    selected_idx: Arc<Mutex<usize>>,
    layout: KeymapActionDetailsLayout,
}

impl KeymapActionDetailsRenderable {
    pub(super) fn new(
        details: Arc<Vec<KeymapActionDetail>>,
        selected_idx: Arc<Mutex<usize>>,
        layout: KeymapActionDetailsLayout,
    ) -> Self {
        Self {
            details,
            selected_idx,
            layout,
        }
    }

    fn selected_detail(&self) -> Option<&KeymapActionDetail> {
        let idx = self.selected_idx.lock().map(|idx| *idx).unwrap_or(0);
        self.details.get(idx).or_else(|| self.details.first())
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(detail) = self.selected_detail() else {
            return vec!["No action selected".dim().into()];
        };

        if matches!(self.layout, KeymapActionDetailsLayout::NarrowFooter) {
            return vec![truncate_line_with_ellipsis_if_overflow(
                detail.description.clone().dim().into(),
                usize::from(width),
            )];
        }

        let mut lines = vec!["Selected Action".bold().into(), Line::from("")];

        lines.push(detail.label.clone().bold().into());
        lines.push(Line::from(vec![
            detail.context_label.clone().dim(),
            "  ".dim(),
            format!("{}.{}", detail.context, detail.action).dim(),
        ]));
        lines.push(Line::from(""));

        let binding = if detail.binding_summary == "unbound" {
            detail.binding_summary.clone().dim()
        } else {
            detail.binding_summary.clone().cyan()
        };
        lines.push(Line::from(vec!["Current: ".dim(), binding]));

        let source = if detail.custom_binding {
            "root override".cyan()
        } else {
            "default keymap".dim()
        };
        lines.push(Line::from(vec!["Source: ".dim(), source]));
        lines.push(Line::from(""));

        let wrap_width = usize::from(width.max(1));
        lines.extend(
            textwrap::wrap(&detail.description, wrap_width)
                .into_iter()
                .map(|line| Line::from(line.into_owned().dim())),
        );

        lines.push(Line::from(""));
        lines.push("Enter edits this shortcut".cyan().into());
        lines
    }
}

impl Renderable for KeymapActionDetailsRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines(area.width)).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }
}

pub(super) fn build_action_details(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> Vec<KeymapActionDetail> {
    KEYMAP_ACTIONS
        .iter()
        .map(|descriptor| {
            let bindings =
                bindings_for_action(runtime_keymap, descriptor.context, descriptor.action)
                    .unwrap_or(&[]);
            KeymapActionDetail {
                context: descriptor.context.to_string(),
                context_label: descriptor.context_label.to_string(),
                action: descriptor.action.to_string(),
                label: action_label(descriptor.action),
                description: descriptor.description.to_string(),
                binding_summary: format_binding_summary(bindings),
                custom_binding: has_custom_binding(
                    keymap_config,
                    descriptor.context,
                    descriptor.action,
                )
                .unwrap_or(false),
            }
        })
        .collect()
}

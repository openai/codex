//! Shared preview-state model for the `/pets` side pane.
//!
//! The preview pane is intentionally small and stateful: the selection popup
//! renders it synchronously, while async preview loading updates this state
//! from outside the widget tree. Keeping the state in a mutex-backed object lets
//! the picker remember the last preview area for out-of-band image rendering
//! without requiring the rest of the popup machinery to know about pet images.

use std::sync::Arc;
use std::sync::Mutex;

use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::render::renderable::Renderable;

#[derive(Debug, Clone, Default)]
pub(crate) struct PetPickerPreviewState {
    inner: Arc<Mutex<PetPickerPreviewInner>>,
}

impl PetPickerPreviewState {
    /// Return a renderable wrapper for the picker side pane.
    ///
    /// The wrapper is cheap to clone and intentionally shares interior state
    /// with the controller so selection-change callbacks can update the visible
    /// loading/error/ready state without rebuilding the popup.
    pub(crate) fn renderable(&self) -> PetPickerPreviewRenderable {
        PetPickerPreviewRenderable {
            inner: Arc::clone(&self.inner),
        }
    }

    pub(crate) fn set_loading(&self) {
        self.update(|inner| {
            inner.status = PetPickerPreviewStatus::Loading;
        });
    }

    pub(crate) fn set_disabled(&self) {
        self.update(|inner| {
            inner.status = PetPickerPreviewStatus::Disabled;
        });
    }

    pub(crate) fn set_ready(&self) {
        self.update(|inner| {
            inner.status = PetPickerPreviewStatus::Ready;
        });
    }

    pub(crate) fn set_error(&self, message: String) {
        self.update(|inner| {
            inner.status = PetPickerPreviewStatus::Error { message };
        });
    }

    pub(crate) fn clear(&self) {
        self.update(|inner| {
            inner.status = PetPickerPreviewStatus::Hidden;
            inner.last_area = None;
        });
    }

    pub(crate) fn area(&self) -> Option<Rect> {
        self.inner.lock().ok().and_then(|inner| inner.last_area)
    }

    fn update(&self, f: impl FnOnce(&mut PetPickerPreviewInner)) {
        if let Ok(mut inner) = self.inner.lock() {
            f(&mut inner);
        }
    }
}

#[derive(Debug, Default)]
struct PetPickerPreviewInner {
    status: PetPickerPreviewStatus,
    last_area: Option<Rect>,
}

#[derive(Debug, Default)]
enum PetPickerPreviewStatus {
    #[default]
    Hidden,
    Loading,
    Disabled,
    Ready,
    Error {
        message: String,
    },
}

pub(crate) struct PetPickerPreviewRenderable {
    inner: Arc<Mutex<PetPickerPreviewInner>>,
}

impl Renderable for PetPickerPreviewRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let (title, body) = {
            let Ok(mut inner) = self.inner.lock() else {
                return;
            };
            inner.last_area = Some(area);
            match &inner.status {
                PetPickerPreviewStatus::Hidden => return,
                PetPickerPreviewStatus::Loading => ("Loading preview...", None),
                PetPickerPreviewStatus::Disabled => (
                    "Terminal pets disabled",
                    Some("No pet will be shown.".to_string()),
                ),
                PetPickerPreviewStatus::Ready => return,
                PetPickerPreviewStatus::Error { message } => {
                    ("Preview unavailable", Some(message.clone()))
                }
            }
        };

        let text_height = if body.is_some() { 2 } else { 1 };
        let text_area = centered_text_area(area, text_height);
        let mut lines = vec![Line::from(title.bold())];
        if let Some(body) = body {
            lines.push(Line::from(body.dim()));
        }
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .render(text_area, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        4
    }
}

fn centered_text_area(area: Rect, height: u16) -> Rect {
    let height = height.min(area.height);
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(area.x, y, area.width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_text_area_centers_vertically() {
        assert_eq!(
            centered_text_area(
                Rect::new(
                    /*x*/ 5, /*y*/ 10, /*width*/ 20, /*height*/ 8
                ),
                /*height*/ 2
            ),
            Rect::new(
                /*x*/ 5, /*y*/ 13, /*width*/ 20, /*height*/ 2
            )
        );
    }
}

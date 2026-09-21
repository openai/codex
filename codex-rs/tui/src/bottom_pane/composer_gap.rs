//! Reserve the owned transcript's hint row immediately above the composer.
//! The app paints its shared hints and reading controls into the per-frame layout rectangle.

use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::render::renderable::Renderable;

#[derive(Default)]
pub(crate) struct ComposerGap {
    /// Filled during rendering; a fresh instance prevents reusing a hidden or clipped row.
    pub(crate) area: Cell<Rect>,
}

impl Renderable for ComposerGap {
    fn render(&self, area: Rect, _buf: &mut Buffer) {
        self.area.set(area);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

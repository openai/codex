use ascii_canvas::{AsciiCanvas, AsciiView};

use std::cmp;
use std::fmt::Debug;

pub mod builder;
pub mod horiz;
pub mod indent;
pub mod message;
pub mod styled;
#[cfg(test)]
mod test;
pub mod text;
pub mod vert;
pub mod wrap;

/// Content which can be rendered.
pub trait Content: Debug {
    fn min_width(&self) -> usize;

    fn emit(&self, view: &mut dyn AsciiView);

    /// Creates a canvas at least `min_width` in width (it may be
    /// larger if the content requires that) and fills it with the
    /// current content. Returns the canvas. Typically `min_width`
    /// would be 80 or the width of the current terminal.
    fn emit_to_canvas(&self, min_width: usize) -> AsciiCanvas {
        let computed_min = self.min_width();
        let min_width = cmp::max(min_width, computed_min);
        debug!(
            "emit_to_canvas: min_width={} computed_min={} self={:#?}",
            min_width, computed_min, self
        );
        let mut canvas = AsciiCanvas::new(0, min_width);
        self.emit(&mut canvas);
        canvas
    }

    /// Emit at a particular upper-left corner, returning the
    /// lower-right corner that was emitted.
    fn emit_at(&self, view: &mut dyn AsciiView, row: usize, column: usize) -> (usize, usize) {
        debug!(
            "emit_at({},{}) self={:?} min_width={:?}",
            row,
            column,
            self,
            self.min_width()
        );
        let mut shifted_view = view.shift(row, column);
        self.emit(&mut shifted_view);
        let (r, c) = shifted_view.close();
        (r, c)
    }

    /// When items are enclosed into a wrap, this method deconstructs
    /// them into their indivisible components.
    fn into_wrap_items(self: Box<Self>, wrap_items: &mut Vec<Box<dyn Content>>);
}

/// Helper function: convert `content` into wrap items and then map
/// those with `op`, appending the final result into `wrap_items`.
/// Useful for "modifier" content items like `Styled` that do not
/// affect wrapping.
fn into_wrap_items_map<OP, C>(
    content: Box<dyn Content>,
    wrap_items: &mut Vec<Box<dyn Content>>,
    op: OP,
) where
    OP: FnMut(Box<dyn Content>) -> C,
    C: Content + 'static,
{
    let mut subvector = vec![];
    content.into_wrap_items(&mut subvector);
    wrap_items.extend(
        subvector
            .into_iter()
            .map(op)
            .map(|item| Box::new(item) as Box<dyn Content>),
    );
}

pub use self::message::Message;

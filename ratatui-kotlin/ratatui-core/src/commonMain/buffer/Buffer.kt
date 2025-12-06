use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Index, IndexMut};
use core::{cmp, fmt};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::buffer::Cell;
use crate::layout::{Position, Rect};
use crate::style::Style;
use crate::text::{Line, Span};

/// A buffer that maps to the desired content of the terminal after the draw call
///
/// No widget in the library interacts directly with the terminal. Instead each of them is required
/// to draw their state to an intermediate buffer. It is basically a grid where each cell contains
/// a grapheme, a foreground color and a background color. This grid will then be used to output
/// the appropriate escape sequences and characters to draw the UI as the user has defined it.
///
/// # Examples:
///
/// ```
/// use ratatui_core::buffer::{Buffer, Cell};
/// use ratatui_core::layout::{Position, Rect};
/// use ratatui_core::style::{Color, Style};
///
/// # fn foo() -> Option<()> {
/// let mut buf = Buffer::empty(Rect {
///     x: 0,
///     y: 0,
///     width: 10,
///     height: 5,
/// });
///
/// // indexing using Position
/// buf[Position { x: 0, y: 0 }].set_symbol("A");
/// assert_eq!(buf[Position { x: 0, y: 0 }].symbol(), "A");
///
/// // indexing using (x, y) tuple (which is converted to Position)
/// buf[(0, 1)].set_symbol("B");
/// assert_eq!(buf[(0, 1)].symbol(), "x");
///
/// // getting an Option instead of panicking if the position is outside the buffer
/// let cell = buf.cell_mut(Position { x: 0, y: 2 })?;
/// cell.set_symbol("C");
///
/// let cell = buf.cell(Position { x: 0, y: 2 })?;
/// assert_eq!(cell.symbol(), "C");
///
/// buf.set_string(
///     3,
///     0,
///     "string",
///     Style::default().fg(Color::Red).bg(Color::White),
/// );
/// let cell = &buf[(5, 0)]; // cannot move out of buf, so we borrow it
/// assert_eq!(cell.symbol(), "r");
/// assert_eq!(cell.fg, Color::Red);
/// assert_eq!(cell.bg, Color::White);
/// # Some(())
/// # }
/// ```
#[derive(Default, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Buffer {
    /// The area represented by this buffer
    pub area: Rect,
    /// The content of the buffer. The length of this Vec should always be equal to area.width *
    /// area.height
    pub content: Vec<Cell>,
}

impl Buffer {
    /// Returns a Buffer with all cells set to the default one
    #[must_use]
    pub fn empty(area: Rect) -> Self {
        Self::filled(area, Cell::EMPTY)
    }

    /// Returns a Buffer with all cells initialized with the attributes of the given Cell
    #[must_use]
    pub fn filled(area: Rect, cell: Cell) -> Self {
        let size = area.area() as usize;
        let content = vec![cell; size];
        Self { area, content }
    }

    /// Returns a Buffer containing the given lines
    #[must_use]
    pub fn with_lines<'a, Iter>(lines: Iter) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Line<'a>>,
    {
        let lines = lines.into_iter().map(Into::into).collect::<Vec<_>>();
        let height = lines.len() as u16;
        let width = lines.iter().map(Line::width).max().unwrap_or_default() as u16;
        let mut buffer = Self::empty(Rect::new(0, 0, width, height));
        for (y, line) in lines.iter().enumerate() {
            buffer.set_line(0, y as u16, line, width);
        }
        buffer
    }

    /// Returns the content of the buffer as a slice
    pub fn content(&self) -> &[Cell] {
        &self.content
    }

    /// Returns the area covered by this buffer
    pub const fn area(&self) -> &Rect {
        &self.area
    }

    /// Returns a reference to the [`Cell`] at the given coordinates
    ///
    /// Callers should use [`Buffer[]`](Self::index) or [`Buffer::cell`] instead of this method.
    ///
    /// Note: idiomatically methods named `get` usually return `Option<&T>`, but this method panics
    /// instead. This is kept for backwards compatibility. See [`cell`](Self::cell) for a safe
    /// alternative.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    #[track_caller]
    #[deprecated = "use `Buffer[(x, y)]` instead. To avoid panicking, use `Buffer::cell((x, y))`. Both methods take `impl Into<Position>`."]
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> &Cell {
        let i = self.index_of(x, y);
        &self.content[i]
    }

    /// Returns a mutable reference to the [`Cell`] at the given coordinates.
    ///
    /// Callers should use [`Buffer[]`](Self::index_mut) or [`Buffer::cell_mut`] instead of this
    /// method.
    ///
    /// Note: idiomatically methods named `get_mut` usually return `Option<&mut T>`, but this method
    /// panics instead. This is kept for backwards compatibility. See [`cell_mut`](Self::cell_mut)
    /// for a safe alternative.
    ///
    /// # Panics
    ///
    /// Panics if the position is outside the `Buffer`'s area.
    #[track_caller]
    #[deprecated = "use `Buffer[(x, y)]` instead. To avoid panicking, use `Buffer::cell_mut((x, y))`. Both methods take `impl Into<Position>`."]
    #[must_use]
    pub fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        let i = self.index_of(x, y);
        &mut self.content[i]
    }

    /// Returns a reference to the [`Cell`] at the given position or [`None`] if the position is
    /// outside the `Buffer`'s area.
    ///
    /// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
    /// `Position::new(x, y)`).
    ///
    /// For a method that panics when the position is outside the buffer instead of returning
    /// `None`, use [`Buffer[]`](Self::index).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::buffer::{Buffer, Cell};
    /// use ratatui_core::layout::{Position, Rect};
    ///
    /// let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
    ///
    /// assert_eq!(buffer.cell(Position::new(0, 0)), Some(&Cell::default()));
    /// assert_eq!(buffer.cell(Position::new(10, 10)), None);
    /// assert_eq!(buffer.cell((0, 0)), Some(&Cell::default()));
    /// assert_eq!(buffer.cell((10, 10)), None);
    /// ```
    #[must_use]
    pub fn cell<P: Into<Position>>(&self, position: P) -> Option<&Cell> {
        let position = position.into();
        let index = self.index_of_opt(position)?;
        self.content.get(index)
    }

    /// Returns a mutable reference to the [`Cell`] at the given position or [`None`] if the
    /// position is outside the `Buffer`'s area.
    ///
    /// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
    /// `Position::new(x, y)`).
    ///
    /// For a method that panics when the position is outside the buffer instead of returning
    /// `None`, use [`Buffer[]`](Self::index_mut).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::buffer::{Buffer, Cell};
    /// use ratatui_core::layout::{Position, Rect};
    /// use ratatui_core::style::{Color, Style};
    /// let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
    ///
    /// if let Some(cell) = buffer.cell_mut(Position::new(0, 0)) {
    ///     cell.set_symbol("A");
    /// }
    /// if let Some(cell) = buffer.cell_mut((0, 0)) {
    ///     cell.set_style(Style::default().fg(Color::Red));
    /// }
    /// ```
    #[must_use]
    pub fn cell_mut<P: Into<Position>>(&mut self, position: P) -> Option<&mut Cell> {
        let position = position.into();
        let index = self.index_of_opt(position)?;
        self.content.get_mut(index)
    }

    /// Returns the index in the `Vec<Cell>` for the given global (x, y) coordinates.
    ///
    /// Global coordinates are offset by the Buffer's area offset (`x`/`y`).
    ///
    /// Usage discouraged, as it exposes `self.content` as a linearly indexable array, which limits
    /// potential future abstractions. See <https://github.com/ratatui/ratatui/issues/1122>.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui_core::buffer::Buffer;
    /// use ratatui_core::layout::Rect;
    ///
    /// let buffer = Buffer::empty(Rect::new(200, 100, 10, 10));
    /// // Global coordinates to the top corner of this buffer's area
    /// assert_eq!(buffer.index_of(200, 100), 0);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when given an coordinate that is outside of this Buffer's area.
    ///
    /// ```should_panic
    /// use ratatui_core::buffer::Buffer;
    /// use ratatui_core::layout::Rect;
    ///
    /// let buffer = Buffer::empty(Rect::new(200, 100, 10, 10));
    /// // Top coordinate is outside of the buffer in global coordinate space, as the Buffer's area
    /// // starts at (200, 100).
    /// buffer.index_of(0, 0); // Panics
    /// ```
    #[track_caller]
    #[must_use]
    pub fn index_of(&self, x: u16, y: u16) -> usize {
        self.index_of_opt(Position { x, y }).unwrap_or_else(|| {
            panic!(
                "index outside of buffer: the area is {area:?} but index is ({x}, {y})",
                area = self.area,
            )
        })
    }

    /// Returns the index in the `Vec<Cell>` for the given global (x, y) coordinates.
    ///
    /// Returns `None` if the given coordinates are outside of the Buffer's area.
    ///
    /// Note that this is private because of <https://github.com/ratatui/ratatui/issues/1122>
    #[must_use]
    const fn index_of_opt(&self, position: Position) -> Option<usize> {
        let area = self.area;
        if !area.contains(position) {
            return None;
        }
        // remove offset
        let y = (position.y - self.area.y) as usize;
        let x = (position.x - self.area.x) as usize;
        let width = self.area.width as usize;
        Some(y * width + x)
    }

    /// Returns the (global) coordinates of a cell given its index.
    ///
    /// Global coordinates are offset by the Buffer's area offset (`x`/`y`).
    ///
    /// Usage discouraged, as it exposes `self.content` as a linearly indexable array, which limits
    /// potential future abstractions. See <https://github.com/ratatui/ratatui/issues/1122>.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui_core::buffer::Buffer;
    /// use ratatui_core::layout::Rect;
    ///
    /// let rect = Rect::new(200, 100, 10, 10);
    /// let buffer = Buffer::empty(rect);
    /// assert_eq!(buffer.pos_of(0), (200, 100));
    /// assert_eq!(buffer.pos_of(14), (204, 101));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when given an index that is outside the Buffer's content.
    ///
    /// ```should_panic
    /// use ratatui_core::buffer::Buffer;
    /// use ratatui_core::layout::Rect;
    ///
    /// let rect = Rect::new(0, 0, 10, 10); // 100 cells in total
    /// let buffer = Buffer::empty(rect);
    /// // Index 100 is the 101th cell, which lies outside of the area of this Buffer.
    /// buffer.pos_of(100); // Panics
    /// ```
    #[must_use]
    pub fn pos_of(&self, index: usize) -> (u16, u16) {
        debug_assert!(
            index < self.content.len(),
            "Trying to get the coords of a cell outside the buffer: i={index} len={}",
            self.content.len()
        );
        let x = index % self.area.width as usize + self.area.x as usize;
        let y = index / self.area.width as usize + self.area.y as usize;
        (
            u16::try_from(x).expect("x overflow. This should never happen as area.width is u16"),
            u16::try_from(y).expect("y overflow. This should never happen as area.height is u16"),
        )
    }

    /// Print a string, starting at the position (x, y)
    pub fn set_string<T, S>(&mut self, x: u16, y: u16, string: T, style: S)
    where
        T: AsRef<str>,
        S: Into<Style>,
    {
        self.set_stringn(x, y, string, usize::MAX, style);
    }

    /// Print at most the first n characters of a string if enough space is available
    /// until the end of the line. Skips zero-width graphemes and control characters.
    ///
    /// Use [`Buffer::set_string`] when the maximum amount of characters can be printed.
    pub fn set_stringn<T, S>(
        &mut self,
        mut x: u16,
        y: u16,
        string: T,
        max_width: usize,
        style: S,
    ) -> (u16, u16)
    where
        T: AsRef<str>,
        S: Into<Style>,
    {
        let max_width = max_width.try_into().unwrap_or(u16::MAX);
        let mut remaining_width = self.area.right().saturating_sub(x).min(max_width);
        let graphemes = UnicodeSegmentation::graphemes(string.as_ref(), true)
            .filter(|symbol| !symbol.contains(char::is_control))
            .map(|symbol| (symbol, symbol.width() as u16))
            .filter(|(_symbol, width)| *width > 0)
            .map_while(|(symbol, width)| {
                remaining_width = remaining_width.checked_sub(width)?;
                Some((symbol, width))
            });
        let style = style.into();
        for (symbol, width) in graphemes {
            self[(x, y)].set_symbol(symbol).set_style(style);
            let next_symbol = x + width;
            x += 1;
            // Reset following cells if multi-width (they would be hidden by the grapheme),
            while x < next_symbol {
                self[(x, y)].reset();
                x += 1;
            }
        }
        (x, y)
    }

    /// Print a line, starting at the position (x, y)
    pub fn set_line(&mut self, x: u16, y: u16, line: &Line<'_>, max_width: u16) -> (u16, u16) {
        let mut remaining_width = max_width;
        let mut x = x;
        for span in line {
            if remaining_width == 0 {
                break;
            }
            let pos = self.set_stringn(
                x,
                y,
                span.content.as_ref(),
                remaining_width as usize,
                line.style.patch(span.style),
            );
            let w = pos.0.saturating_sub(x);
            x = pos.0;
            remaining_width = remaining_width.saturating_sub(w);
        }
        (x, y)
    }

    /// Print a span, starting at the position (x, y)
    pub fn set_span(&mut self, x: u16, y: u16, span: &Span<'_>, max_width: u16) -> (u16, u16) {
        self.set_stringn(x, y, &span.content, max_width as usize, span.style)
    }

    /// Set the style of all cells in the given area.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: crate::style::Color
    pub fn set_style<S: Into<Style>>(&mut self, area: Rect, style: S) {
        let style = style.into();
        let area = self.area.intersection(area);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                self[(x, y)].set_style(style);
            }
        }
    }

    /// Resize the buffer so that the mapped area matches the given area and that the buffer
    /// length is equal to area.width * area.height
    pub fn resize(&mut self, area: Rect) {
        let length = area.area() as usize;
        if self.content.len() > length {
            self.content.truncate(length);
        } else {
            self.content.resize(length, Cell::EMPTY);
        }
        self.area = area;
    }

    /// Reset all cells in the buffer
    pub fn reset(&mut self) {
        for cell in &mut self.content {
            cell.reset();
        }
    }

    /// Merge an other buffer into this one
    pub fn merge(&mut self, other: &Self) {
        let area = self.area.union(other.area);
        self.content.resize(area.area() as usize, Cell::EMPTY);

        // Move original content to the appropriate space
        let size = self.area.area() as usize;
        for i in (0..size).rev() {
            let (x, y) = self.pos_of(i);
            // New index in content
            let k = ((y - area.y) * area.width + x - area.x) as usize;
            if i != k {
                self.content[k] = self.content[i].clone();
                self.content[i].reset();
            }
        }

        // Push content of the other buffer into this one (may erase previous
        // data)
        let size = other.area.area() as usize;
        for i in 0..size {
            let (x, y) = other.pos_of(i);
            // New index in content
            let k = ((y - area.y) * area.width + x - area.x) as usize;
            self.content[k] = other.content[i].clone();
        }
        self.area = area;
    }

    /// Builds a minimal sequence of coordinates and Cells necessary to update the UI from
    /// self to other.
    ///
    /// We're assuming that buffers are well-formed, that is no double-width cell is followed by
    /// a non-blank cell.
    ///
    /// # Multi-width characters handling:
    ///
    /// ```text
    /// (Index:) `01`
    /// Prev:    `コ`
    /// Next:    `aa`
    /// Updates: `0: a, 1: a'
    /// ```
    ///
    /// ```text
    /// (Index:) `01`
    /// Prev:    `a `
    /// Next:    `コ`
    /// Updates: `0: コ` (double width symbol at index 0 - skip index 1)
    /// ```
    ///
    /// ```text
    /// (Index:) `012`
    /// Prev:    `aaa`
    /// Next:    `aコ`
    /// Updates: `0: a, 1: コ` (double width symbol at index 1 - skip index 2)
    /// ```
    pub fn diff<'a>(&self, other: &'a Self) -> Vec<(u16, u16, &'a Cell)> {
        let previous_buffer = &self.content;
        let next_buffer = &other.content;

        let mut updates: Vec<(u16, u16, &Cell)> = vec![];
        // Cells invalidated by drawing/replacing preceding multi-width characters:
        let mut invalidated: usize = 0;
        // Cells from the current buffer to skip due to preceding multi-width characters taking
        // their place (the skipped cells should be blank anyway), or due to per-cell-skipping:
        let mut to_skip: usize = 0;
        for (i, (current, previous)) in next_buffer.iter().zip(previous_buffer.iter()).enumerate() {
            if !current.skip && (current != previous || invalidated > 0) && to_skip == 0 {
                let (x, y) = self.pos_of(i);
                updates.push((x, y, &next_buffer[i]));

                // If the current cell is multi-width, ensure the trailing cells are explicitly
                // cleared when they previously contained non-blank content. Some terminals do not
                // reliably clear the trailing cell(s) when printing a wide grapheme, which can
                // result in visual artifacts (e.g., leftover characters). Emitting an explicit
                // update for the trailing cells avoids this.
                let symbol = current.symbol();
                let cell_width = symbol.width();
                // Work around terminals that fail to clear the trailing cell of certain
                // emoji presentation sequences (those containing VS16 / U+FE0F).
                // Only emit explicit clears for such sequences to avoid bloating diffs
                // for standard wide characters (e.g., CJK), which terminals handle well.
                let contains_vs16 = symbol.chars().any(|c| c == '\u{FE0F}');
                if cell_width > 1 && contains_vs16 {
                    for k in 1..cell_width {
                        let j = i + k;
                        // Make sure that we are still inside the buffer.
                        if j >= next_buffer.len() || j >= previous_buffer.len() {
                            break;
                        }
                        let prev_trailing = &previous_buffer[j];
                        let next_trailing = &next_buffer[j];
                        if !next_trailing.skip && prev_trailing != next_trailing {
                            let (tx, ty) = self.pos_of(j);
                            // Push an explicit update for the trailing cell.
                            // This is expected to be a blank cell, but we use the actual
                            // content from the next buffer to handle cases where
                            // the user has explicitly set something else.
                            updates.push((tx, ty, next_trailing));
                        }
                    }
                }
            }

            to_skip = current.symbol().width().saturating_sub(1);

            let affected_width = cmp::max(current.symbol().width(), previous.symbol().width());
            invalidated = cmp::max(affected_width, invalidated).saturating_sub(1);
        }
        updates
    }
}

impl<P: Into<Position>> Index<P> for Buffer {
    type Output = Cell;

    /// Returns a reference to the [`Cell`] at the given position.
    ///
    /// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
    /// `Position::new(x, y)`).
    ///
    /// # Panics
    ///
    /// May panic if the given position is outside the buffer's area. For a method that returns
    /// `None` instead of panicking, use [`Buffer::cell`](Self::cell).
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui_core::buffer::{Buffer, Cell};
    /// use ratatui_core::layout::{Position, Rect};
    ///
    /// let buf = Buffer::empty(Rect::new(0, 0, 10, 10));
    /// let cell = &buf[(0, 0)];
    /// let cell = &buf[Position::new(0, 0)];
    /// ```
    fn index(&self, position: P) -> &Self::Output {
        let position = position.into();
        let index = self.index_of(position.x, position.y);
        &self.content[index]
    }
}

impl<P: Into<Position>> IndexMut<P> for Buffer {
    /// Returns a mutable reference to the [`Cell`] at the given position.
    ///
    /// This method accepts any value that can be converted to [`Position`] (e.g. `(x, y)` or
    /// `Position::new(x, y)`).
    ///
    /// # Panics
    ///
    /// May panic if the given position is outside the buffer's area. For a method that returns
    /// `None` instead of panicking, use [`Buffer::cell_mut`](Self::cell_mut).
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui_core::buffer::{Buffer, Cell};
    /// use ratatui_core::layout::{Position, Rect};
    ///
    /// let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
    /// buf[(0, 0)].set_symbol("A");
    /// buf[Position::new(0, 0)].set_symbol("B");
    /// ```
    fn index_mut(&mut self, position: P) -> &mut Self::Output {
        let position = position.into();
        let index = self.index_of(position.x, position.y);
        &mut self.content[index]
    }
}

impl fmt::Debug for Buffer {
    /// Writes a debug representation of the buffer to the given formatter.
    ///
    /// The format is like a pretty printed struct, with the following fields:
    /// * `area`: displayed as `Rect { x: 1, y: 2, width: 3, height: 4 }`
    /// * `content`: displayed as a list of strings representing the content of the buffer
    /// * `styles`: displayed as a list of: `{ x: 1, y: 2, fg: Color::Red, bg: Color::Blue,
    ///   modifier: Modifier::BOLD }` only showing a value when there is a change in style.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Buffer {{\n    area: {:?}", &self.area))?;

        if self.area.is_empty() {
            return f.write_str("\n}");
        }

        f.write_str(",\n    content: [\n")?;
        let mut last_style = None;
        let mut styles = vec![];
        for (y, line) in self.content.chunks(self.area.width as usize).enumerate() {
            let mut overwritten = vec![];
            let mut skip: usize = 0;
            f.write_str("        \"")?;
            for (x, c) in line.iter().enumerate() {
                if skip == 0 {
                    f.write_str(c.symbol())?;
                } else {
                    overwritten.push((x, c.symbol()));
                }
                skip = cmp::max(skip, c.symbol().width()).saturating_sub(1);
                #[cfg(feature = "underline-color")]
                {
                    let style = (c.fg, c.bg, c.underline_color, c.modifier);
                    if last_style != Some(style) {
                        last_style = Some(style);
                        styles.push((x, y, c.fg, c.bg, c.underline_color, c.modifier));
                    }
                }
                #[cfg(not(feature = "underline-color"))]
                {
                    let style = (c.fg, c.bg, c.modifier);
                    if last_style != Some(style) {
                        last_style = Some(style);
                        styles.push((x, y, c.fg, c.bg, c.modifier));
                    }
                }
            }
            f.write_str("\",")?;
            if !overwritten.is_empty() {
                f.write_fmt(format_args!(
                    " // hidden by multi-width symbols: {overwritten:?}"
                ))?;
            }
            f.write_str("\n")?;
        }
        f.write_str("    ],\n    styles: [\n")?;
        for s in styles {
            #[cfg(feature = "underline-color")]
            f.write_fmt(format_args!(
                "        x: {}, y: {}, fg: {:?}, bg: {:?}, underline: {:?}, modifier: {:?},\n",
                s.0, s.1, s.2, s.3, s.4, s.5
            ))?;
            #[cfg(not(feature = "underline-color"))]
            f.write_fmt(format_args!(
                "        x: {}, y: {}, fg: {:?}, bg: {:?}, modifier: {:?},\n",
                s.0, s.1, s.2, s.3, s.4
            ))?;
        }
        f.write_str("    ]\n}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::ToString;
    use core::iter;
    use std::{dbg, println};

    use itertools::Itertools;
    use rstest::{fixture, rstest};

    use super::*;
    use crate::style::{Color, Modifier, Stylize};

    #[test]
    fn debug_empty_buffer() {
        let buffer = Buffer::empty(Rect::ZERO);
        let result = format!("{buffer:?}");
        println!("{result}");
        let expected = "Buffer {\n    area: Rect { x: 0, y: 0, width: 0, height: 0 }\n}";
        assert_eq!(result, expected);
    }

    #[cfg(feature = "underline-color")]
    #[test]
    fn debug_grapheme_override() {
        let buffer = Buffer::with_lines(["a🦀b"]);
        let result = format!("{buffer:?}");
        println!("{result}");
        let expected = indoc::indoc!(
            r#"
            Buffer {
                area: Rect { x: 0, y: 0, width: 4, height: 1 },
                content: [
                    "a🦀b", // hidden by multi-width symbols: [(2, " ")]
                ],
                styles: [
                    x: 0, y: 0, fg: Reset, bg: Reset, underline: Reset, modifier: NONE,
                ]
            }"#
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn debug_some_example() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 2));
        buffer.set_string(0, 0, "Hello World!", Style::default());
        buffer.set_string(
            0,
            1,
            "G'day World!",
            Style::default()
                .fg(Color::Green)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        let result = format!("{buffer:?}");
        println!("{result}");
        #[cfg(feature = "underline-color")]
        let expected = indoc::indoc!(
            r#"
            Buffer {
                area: Rect { x: 0, y: 0, width: 12, height: 2 },
                content: [
                    "Hello World!",
                    "G'day World!",
                ],
                styles: [
                    x: 0, y: 0, fg: Reset, bg: Reset, underline: Reset, modifier: NONE,
                    x: 0, y: 1, fg: Green, bg: Yellow, underline: Reset, modifier: BOLD,
                ]
            }"#
        );
        #[cfg(not(feature = "underline-color"))]
        let expected = indoc::indoc!(
            r#"
            Buffer {
                area: Rect { x: 0, y: 0, width: 12, height: 2 },
                content: [
                    "Hello World!",
                    "G'day World!",
                ],
                styles: [
                    x: 0, y: 0, fg: Reset, bg: Reset, modifier: NONE,
                    x: 0, y: 1, fg: Green, bg: Yellow, modifier: BOLD,
                ]
            }"#
        );

        assert_eq!(result, expected);
    }

    #[test]
    fn it_translates_to_and_from_coordinates() {
        let rect = Rect::new(200, 100, 50, 80);
        let buf = Buffer::empty(rect);

        // First cell is at the upper left corner.
        assert_eq!(buf.pos_of(0), (200, 100));
        assert_eq!(buf.index_of(200, 100), 0);

        // Last cell is in the lower right.
        assert_eq!(buf.pos_of(buf.content.len() - 1), (249, 179));
        assert_eq!(buf.index_of(249, 179), buf.content.len() - 1);
    }

    #[test]
    #[should_panic(expected = "outside the buffer")]
    fn pos_of_panics_on_out_of_bounds() {
        let rect = Rect::new(0, 0, 10, 10);
        let buf = Buffer::empty(rect);

        // There are a total of 100 cells; zero-indexed means that 100 would be the 101st cell.
        let _ = buf.pos_of(100);
    }

    #[rstest]
    #[case::left(9, 10)]
    #[case::top(10, 9)]
    #[case::right(20, 10)]
    #[case::bottom(10, 20)]
    #[should_panic(
        expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is"
    )]
    fn index_of_panics_on_out_of_bounds(#[case] x: u16, #[case] y: u16) {
        let _ = Buffer::empty(Rect::new(10, 10, 10, 10)).index_of(x, y);
    }

    #[test]
    fn test_cell() {
        let buf = Buffer::with_lines(["Hello", "World"]);

        let mut expected = Cell::default();
        expected.set_symbol("H");

        assert_eq!(buf.cell((0, 0)), Some(&expected));
        assert_eq!(buf.cell((10, 10)), None);
        assert_eq!(buf.cell(Position::new(0, 0)), Some(&expected));
        assert_eq!(buf.cell(Position::new(10, 10)), None);
    }

    #[test]
    fn test_cell_mut() {
        let mut buf = Buffer::with_lines(["Hello", "World"]);

        let mut expected = Cell::default();
        expected.set_symbol("H");

        assert_eq!(buf.cell_mut((0, 0)), Some(&mut expected));
        assert_eq!(buf.cell_mut((10, 10)), None);
        assert_eq!(buf.cell_mut(Position::new(0, 0)), Some(&mut expected));
        assert_eq!(buf.cell_mut(Position::new(10, 10)), None);
    }

    #[test]
    fn index() {
        let buf = Buffer::with_lines(["Hello", "World"]);

        let mut expected = Cell::default();
        expected.set_symbol("H");

        assert_eq!(buf[(0, 0)], expected);
    }

    #[rstest]
    #[case::left(9, 10)]
    #[case::top(10, 9)]
    #[case::right(20, 10)]
    #[case::bottom(10, 20)]
    #[should_panic(
        expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is"
    )]
    fn index_out_of_bounds_panics(#[case] x: u16, #[case] y: u16) {
        let rect = Rect::new(10, 10, 10, 10);
        let buf = Buffer::empty(rect);
        let _ = buf[(x, y)];
    }

    #[test]
    fn index_mut() {
        let mut buf = Buffer::with_lines(["Cat", "Dog"]);
        buf[(0, 0)].set_symbol("B");
        buf[Position::new(0, 1)].set_symbol("L");
        assert_eq!(buf, Buffer::with_lines(["Bat", "Log"]));
    }

    #[rstest]
    #[case::left(9, 10)]
    #[case::top(10, 9)]
    #[case::right(20, 10)]
    #[case::bottom(10, 20)]
    #[should_panic(
        expected = "index outside of buffer: the area is Rect { x: 10, y: 10, width: 10, height: 10 } but index is"
    )]
    fn index_mut_out_of_bounds_panics(#[case] x: u16, #[case] y: u16) {
        let mut buf = Buffer::empty(Rect::new(10, 10, 10, 10));
        buf[(x, y)].set_symbol("A");
    }

    #[test]
    fn set_string() {
        let area = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(area);

        // Zero-width
        buffer.set_stringn(0, 0, "aaa", 0, Style::default());
        assert_eq!(buffer, Buffer::with_lines(["     "]));

        buffer.set_string(0, 0, "aaa", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["aaa  "]));

        // Width limit:
        buffer.set_stringn(0, 0, "bbbbbbbbbbbbbb", 4, Style::default());
        assert_eq!(buffer, Buffer::with_lines(["bbbb "]));

        buffer.set_string(0, 0, "12345", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["12345"]));

        // Width truncation:
        buffer.set_string(0, 0, "123456", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["12345"]));

        // multi-line
        buffer = Buffer::empty(Rect::new(0, 0, 5, 2));
        buffer.set_string(0, 0, "12345", Style::default());
        buffer.set_string(0, 1, "67890", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["12345", "67890"]));
    }

    #[test]
    fn set_string_multi_width_overwrite() {
        let area = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(area);

        // multi-width overwrite
        buffer.set_string(0, 0, "aaaaa", Style::default());
        buffer.set_string(0, 0, "称号", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["称号a"]));
    }

    #[test]
    fn set_string_zero_width() {
        assert_eq!("\u{200B}".width(), 0);

        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);

        // Leading grapheme with zero width
        let s = "\u{200B}a";
        buffer.set_stringn(0, 0, s, 1, Style::default());
        assert_eq!(buffer, Buffer::with_lines(["a"]));

        // Trailing grapheme with zero with
        let s = "a\u{200B}";
        buffer.set_stringn(0, 0, s, 1, Style::default());
        assert_eq!(buffer, Buffer::with_lines(["a"]));
    }

    #[test]
    fn set_string_double_width() {
        let area = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(0, 0, "コン", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["コン "]));

        // Only 1 space left.
        buffer.set_string(0, 0, "コンピ", Style::default());
        assert_eq!(buffer, Buffer::with_lines(["コン "]));
    }

    #[fixture]
    fn small_one_line_buffer() -> Buffer {
        Buffer::empty(Rect::new(0, 0, 5, 1))
    }

    #[rstest]
    #[case::empty("", "     ")]
    #[case::one("1", "1    ")]
    #[case::full("12345", "12345")]
    #[case::overflow("123456", "12345")]
    fn set_line_raw(
        mut small_one_line_buffer: Buffer,
        #[case] content: &str,
        #[case] expected: &str,
    ) {
        let line = Line::raw(content);
        small_one_line_buffer.set_line(0, 0, &line, 5);

        // note: testing with empty / set_string here instead of with_lines because with_lines calls
        // set_line
        let mut expected_buffer = Buffer::empty(small_one_line_buffer.area);
        expected_buffer.set_string(0, 0, expected, Style::default());
        assert_eq!(small_one_line_buffer, expected_buffer);
    }

    #[rstest]
    #[case::empty("", "     ")]
    #[case::one("1", "1    ")]
    #[case::full("12345", "12345")]
    #[case::overflow("123456", "12345")]
    fn set_line_styled(
        mut small_one_line_buffer: Buffer,
        #[case] content: &str,
        #[case] expected: &str,
    ) {
        let color = Color::Blue;
        let line = Line::styled(content, color);
        small_one_line_buffer.set_line(0, 0, &line, 5);

        // note: manually testing the contents here as the Buffer::with_lines calls set_line
        let actual_contents = small_one_line_buffer
            .content
            .iter()
            .map(Cell::symbol)
            .join("");
        let actual_styles = small_one_line_buffer
            .content
            .iter()
            .map(|c| c.fg)
            .collect_vec();

        // set_line only sets the style for non-empty cells (unlike Line::render which sets the
        // style for all cells)
        let expected_styles = iter::repeat_n(color, content.len().min(5))
            .chain(iter::repeat_n(
                Color::default(),
                5_usize.saturating_sub(content.len()),
            ))
            .collect_vec();
        assert_eq!(actual_contents, expected);
        assert_eq!(actual_styles, expected_styles);
    }

    #[test]
    fn set_style() {
        let mut buffer = Buffer::with_lines(["aaaaa", "bbbbb", "ccccc"]);
        buffer.set_style(Rect::new(0, 1, 5, 1), Style::new().red());
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "aaaaa".into(),
            "bbbbb".red(),
            "ccccc".into(),
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn set_style_does_not_panic_when_out_of_area() {
        let mut buffer = Buffer::with_lines(["aaaaa", "bbbbb", "ccccc"]);
        buffer.set_style(Rect::new(0, 1, 10, 3), Style::new().red());
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "aaaaa".into(),
            "bbbbb".red(),
            "ccccc".red(),
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn with_lines() {
        #[rustfmt::skip]
        let buffer = Buffer::with_lines([
            "┌────────┐",
            "│コンピュ│",
            "│ーa 上で│",
            "└────────┘",
        ]);
        assert_eq!(buffer.area.x, 0);
        assert_eq!(buffer.area.y, 0);
        assert_eq!(buffer.area.width, 10);
        assert_eq!(buffer.area.height, 4);
    }

    #[test]
    fn diff_empty_empty() {
        let area = Rect::new(0, 0, 40, 40);
        let prev = Buffer::empty(area);
        let next = Buffer::empty(area);
        let diff = prev.diff(&next);
        assert_eq!(diff, []);
    }

    #[test]
    fn diff_empty_filled() {
        let area = Rect::new(0, 0, 40, 40);
        let prev = Buffer::empty(area);
        let next = Buffer::filled(area, Cell::new("a"));
        let diff = prev.diff(&next);
        assert_eq!(diff.len(), 40 * 40);
    }

    #[test]
    fn diff_filled_filled() {
        let area = Rect::new(0, 0, 40, 40);
        let prev = Buffer::filled(area, Cell::new("a"));
        let next = Buffer::filled(area, Cell::new("a"));
        let diff = prev.diff(&next);
        assert_eq!(diff, []);
    }

    #[test]
    fn diff_single_width() {
        let prev = Buffer::with_lines([
            "          ",
            "┌Title─┐  ",
            "│      │  ",
            "│      │  ",
            "└──────┘  ",
        ]);
        let next = Buffer::with_lines([
            "          ",
            "┌TITLE─┐  ",
            "│      │  ",
            "│      │  ",
            "└──────┘  ",
        ]);
        let diff = prev.diff(&next);
        assert_eq!(
            diff,
            [
                (2, 1, &Cell::new("I")),
                (3, 1, &Cell::new("T")),
                (4, 1, &Cell::new("L")),
                (5, 1, &Cell::new("E")),
            ]
        );
    }

    #[test]
    fn diff_multi_width() {
        #[rustfmt::skip]
        let prev = Buffer::with_lines([
            "┌Title─┐  ",
            "└──────┘  ",
        ]);
        #[rustfmt::skip]
        let next = Buffer::with_lines([
            "┌称号──┐  ",
            "└──────┘  ",
        ]);
        let diff = prev.diff(&next);
        assert_eq!(
            diff,
            [
                (1, 0, &Cell::new("称")),
                // Skipped "i"
                (3, 0, &Cell::new("号")),
                // Skipped "l"
                (5, 0, &Cell::new("─")),
            ]
        );
    }

    #[test]
    fn diff_multi_width_offset() {
        let prev = Buffer::with_lines(["┌称号──┐"]);
        let next = Buffer::with_lines(["┌─称号─┐"]);

        let diff = prev.diff(&next);
        assert_eq!(
            diff,
            [
                (1, 0, &Cell::new("─")),
                (2, 0, &Cell::new("称")),
                (4, 0, &Cell::new("号")),
            ]
        );
    }

    #[test]
    fn diff_skip() {
        let prev = Buffer::with_lines(["123"]);
        let mut next = Buffer::with_lines(["456"]);
        for i in 1..3 {
            next.content[i].set_skip(true);
        }

        let diff = prev.diff(&next);
        assert_eq!(diff, [(0, 0, &Cell::new("4"))],);
    }

    #[rstest]
    #[case(Rect::new(0, 0, 2, 2), Rect::new(0, 2, 2, 2), ["11", "11", "22", "22"])]
    #[case(Rect::new(2, 2, 2, 2), Rect::new(0, 0, 2, 2), ["22  ", "22  ", "  11", "  11"])]
    fn merge<'line, Lines>(#[case] one: Rect, #[case] two: Rect, #[case] expected: Lines)
    where
        Lines: IntoIterator,
        Lines::Item: Into<Line<'line>>,
    {
        let mut one = Buffer::filled(one, Cell::new("1"));
        let two = Buffer::filled(two, Cell::new("2"));
        one.merge(&two);
        assert_eq!(one, Buffer::with_lines(expected));
    }

    #[test]
    fn merge_with_offset() {
        let mut one = Buffer::filled(
            Rect {
                x: 3,
                y: 3,
                width: 2,
                height: 2,
            },
            Cell::new("1"),
        );
        let two = Buffer::filled(
            Rect {
                x: 1,
                y: 1,
                width: 3,
                height: 4,
            },
            Cell::new("2"),
        );
        one.merge(&two);
        let mut expected = Buffer::with_lines(["222 ", "222 ", "2221", "2221"]);
        expected.area = Rect {
            x: 1,
            y: 1,
            width: 4,
            height: 4,
        };
        assert_eq!(one, expected);
    }

    #[rstest]
    #[case(false, true, [false, false, true, true, true, true])]
    #[case(true, false, [true, true, false, false, false, false])]
    fn merge_skip(#[case] skip_one: bool, #[case] skip_two: bool, #[case] expected: [bool; 6]) {
        let mut one = {
            let area = Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            };
            let mut cell = Cell::new("1");
            cell.skip = skip_one;
            Buffer::filled(area, cell)
        };
        let two = {
            let area = Rect {
                x: 0,
                y: 1,
                width: 2,
                height: 2,
            };
            let mut cell = Cell::new("2");
            cell.skip = skip_two;
            Buffer::filled(area, cell)
        };
        one.merge(&two);
        let skipped = one.content().iter().map(|c| c.skip).collect::<Vec<_>>();
        assert_eq!(skipped, expected);
    }

    #[test]
    fn with_lines_accepts_into_lines() {
        use crate::style::Stylize;
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 2));
        buf.set_string(0, 0, "foo", Style::new().red());
        buf.set_string(0, 1, "bar", Style::new().blue());
        assert_eq!(buf, Buffer::with_lines(["foo".red(), "bar".blue()]));
    }

    #[test]
    fn control_sequence_rendered_full() {
        let text = "I \x1b[0;36mwas\x1b[0m here!";

        let mut buffer = Buffer::filled(Rect::new(0, 0, 25, 3), Cell::new("x"));
        buffer.set_string(1, 1, text, Style::new());

        let expected = Buffer::with_lines([
            "xxxxxxxxxxxxxxxxxxxxxxxxx",
            "xI [0;36mwas[0m here!xxxx",
            "xxxxxxxxxxxxxxxxxxxxxxxxx",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn control_sequence_rendered_partially() {
        let text = "I \x1b[0;36mwas\x1b[0m here!";

        let mut buffer = Buffer::filled(Rect::new(0, 0, 11, 3), Cell::new("x"));
        buffer.set_string(1, 1, text, Style::new());

        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "xxxxxxxxxxx",
            "xI [0;36mwa",
            "xxxxxxxxxxx",
        ]);
        assert_eq!(buffer, expected);
    }

    /// Emojis normally contain various characters which should stay part of the Emoji.
    /// This should work fine by utilizing `unicode_segmentation` but a testcase is probably helpful
    /// due to the nature of never perfect Unicode implementations and all of its quirks.
    #[rstest]
    // Shrug without gender or skintone. Has a width of 2 like all emojis have.
    #[case::shrug("🤷", "🤷xxxxx")]
    // Technically this is a (brown) bear, a zero-width joiner and a snowflake
    // As it is joined its a single emoji and should therefore have a width of 2.
    // Prior to unicode-width 0.2, this was incorrectly detected as width 4 for some reason
    #[case::polarbear("🐻‍❄️", "🐻‍❄️xxxxx")]
    // Technically this is an eye, a zero-width joiner and a speech bubble
    // Both eye and speech bubble include a 'display as emoji' variation selector
    // Prior to unicode-width 0.2, this was incorrectly detected as width 4 for some reason
    #[case::eye_speechbubble("👁️‍🗨️", "👁️‍🗨️xxxxx")]
    // Keyboard keycap emoji: base symbol + VS16 for emoji presentation
    // This should render as a single grapheme with width 2.
    #[case::keyboard_emoji("⌨️", "⌨️xxxxx")]
    fn renders_emoji(#[case] input: &str, #[case] expected: &str) {
        use unicode_width::UnicodeWidthChar;

        dbg!(input);
        dbg!(input.len());
        dbg!(
            input
                .graphemes(true)
                .map(|symbol| (symbol, symbol.escape_unicode().to_string(), symbol.width()))
                .collect::<Vec<_>>()
        );
        dbg!(
            input
                .chars()
                .map(|char| (
                    char,
                    char.escape_unicode().to_string(),
                    char.width(),
                    char.is_control()
                ))
                .collect::<Vec<_>>()
        );

        let mut buffer = Buffer::filled(Rect::new(0, 0, 7, 1), Cell::new("x"));
        buffer.set_string(0, 0, input, Style::new());

        let expected = Buffer::with_lines([expected]);
        assert_eq!(buffer, expected);
    }

    /// Regression test for <https://github.com/ratatui/ratatui/issues/1441>
    ///
    /// Previously the `pos_of` function would incorrectly cast the index to a u16 value instead of
    /// using the index as is. This caused incorrect rendering of any buffer with an length > 65535.
    #[test]
    fn index_pos_of_u16_max() {
        let buffer = Buffer::empty(Rect::new(0, 0, 256, 256 + 1));
        assert_eq!(buffer.index_of(255, 255), 65535);
        assert_eq!(buffer.pos_of(65535), (255, 255));

        assert_eq!(buffer.index_of(0, 256), 65536);
        assert_eq!(buffer.pos_of(65536), (0, 256)); // previously (0, 0)

        assert_eq!(buffer.index_of(1, 256), 65537);
        assert_eq!(buffer.pos_of(65537), (1, 256)); // previously (1, 0)

        assert_eq!(buffer.index_of(255, 256), 65791);
        assert_eq!(buffer.pos_of(65791), (255, 256)); // previously (255, 0)
    }

    #[test]
    fn diff_clears_trailing_cell_for_wide_grapheme() {
        // Reproduce: write "ab", then overwrite with a wide emoji like "⌨️"
        let prev = Buffer::with_lines(["ab"]); // width 2 area inferred
        assert_eq!(prev.area.width, 2);

        let mut next = Buffer::with_lines(["  "]); // start with blanks
        next.set_string(0, 0, "⌨️", Style::new());

        // The next buffer contains a wide grapheme occupying cell 0 and implicitly cell 1.
        // The debug formatting shows the hidden trailing space.
        let expected_next = Buffer::with_lines(["⌨️"]);
        assert_eq!(next, expected_next);

        // The diff should include an update for (0,0) to draw the emoji. Depending on
        // terminal behavior, it may or may not be necessary to explicitly clear (1,0).
        // At minimum, ensure the first cell is updated and nothing incorrect is emitted.
        let diff = prev.diff(&next);
        assert!(
            diff.iter()
                .any(|(x, y, c)| *x == 0 && *y == 0 && c.symbol() == "⌨️")
        );
        // And it should explicitly clear the trailing cell (1,0) to avoid leftovers on terminals
        // that don't automatically clear the following cell for wide characters.
        assert!(
            diff.iter()
                .any(|(x, y, c)| *x == 1 && *y == 0 && c.symbol() == " ")
        );
    }
}

//! A [`Canvas`] and a collection of [`Shape`]s.
//!
//! The [`Canvas`] is a blank space on which you can draw anything manually or use one of the
//! predefined [`Shape`]s.
//!
//! The available shapes are:
//!
//! - [`Circle`]: A basic circle
//! - [`Line`]: A line between two points
//! - [`Map`]: A world map
//! - [`Points`]: A scatter of points
//! - [`Rectangle`]: A basic rectangle
//!
//! You can also implement your own custom [`Shape`]s.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::iter::zip;

use itertools::Itertools;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::symbols::braille::BRAILLE;
use ratatui_core::symbols::pixel::{OCTANTS, QUADRANTS, SEXTANTS};
use ratatui_core::symbols::{self, Marker};
use ratatui_core::text::Line as TextLine;
use ratatui_core::widgets::Widget;

pub use self::circle::Circle;
pub use self::line::Line;
pub use self::map::{Map, MapResolution};
pub use self::points::Points;
pub use self::rectangle::Rectangle;
use crate::block::{Block, BlockExt};
#[cfg(not(feature = "std"))]
use crate::polyfills::F64Polyfills;

mod circle;
mod line;
mod map;
mod points;
mod rectangle;
mod world;

/// Something that can be drawn on a [`Canvas`].
///
/// You may implement your own canvas custom widgets by implementing this trait.
pub trait Shape {
    /// Draws this [`Shape`] using the given [`Painter`].
    ///
    /// This is the only method required to implement a custom widget that can be drawn on a
    /// [`Canvas`].
    fn draw(&self, painter: &mut Painter);
}

/// Label to draw some text on the canvas
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Label<'a> {
    x: f64,
    y: f64,
    line: TextLine<'a>,
}

/// A single layer of the canvas.
///
/// This allows the canvas to be drawn in multiple layers. This is useful if you want to draw
/// multiple shapes on the canvas in specific order.
#[derive(Debug)]
struct Layer {
    contents: Vec<LayerCell>,
}

/// A cell within a layer.
///
/// If a [`Context`] contains multiple layers, then the symbol, foreground, and background colors
/// for a character will be determined by the top-most layer that provides a value for that
/// character. For example, a chart drawn with [`Marker::Block`] may provide the background color,
/// and a later chart drawn with [`Marker::Braille`] may provide the symbol and foreground color.
#[derive(Debug)]
struct LayerCell {
    symbol: Option<char>,
    fg: Option<Color>,
    bg: Option<Color>,
}

/// A grid of cells that can be painted on.
///
/// The grid represents a particular screen region measured in rows and columns. The underlying
/// resolution of the grid might exceed the number of rows and columns. For example, a grid of
/// Braille patterns will have a resolution of 2x4 dots per cell. This means that a grid of 10x10
/// cells will have a resolution of 20x40 dots.
trait Grid: fmt::Debug {
    /// Get the resolution of the grid in number of dots.
    ///
    /// This doesn't have to be the same as the number of rows and columns of the grid. For example,
    /// a grid of Braille patterns will have a resolution of 2x4 dots per cell. This means that a
    /// grid of 10x10 cells will have a resolution of 20x40 dots.
    fn resolution(&self) -> (f64, f64);
    /// Paint a point of the grid.
    ///
    /// The point is expressed in number of dots starting at the origin of the grid in the top left
    /// corner. Note that this is not the same as the `(x, y)` coordinates of the canvas.
    fn paint(&mut self, x: usize, y: usize, color: Color);
    /// Save the current state of the [`Grid`] as a layer to be rendered
    fn save(&self) -> Layer;
    /// Reset the grid to its initial state
    fn reset(&mut self);
}

/// The pattern and color of a `PatternGrid` cell.
#[derive(Copy, Clone, Debug, Default)]
struct PatternCell {
    /// The pattern of a grid character.
    ///
    /// The pattern is stored in the lower bits in a row-major order. For instance, for a 2x4
    /// pattern marker, bits 0 to 7 of this field should represent the following pseudo-pixels:
    ///
    /// | 0 1 |
    /// | 2 3 |
    /// | 4 5 |
    /// | 6 7 |
    pattern: u8,
    /// The color of a cell only supports foreground colors for now as there's no way to
    /// individually set the background color of each pseudo-pixel in a pattern character.
    color: Option<Color>,
}

/// The `PatternGrid` is a grid made up of cells each containing a `W`x`H` pattern character.
///
/// This makes it possible to draw shapes with a resolution of e.g. 2x4 (Braille or unicode octant)
/// per cell.
/// Font support for the relevant pattern character is required. If your terminal or font does not
/// support the relevant unicode block, you will see unicode replacement characters (�) instead.
///
/// This grid type only supports a single foreground color for each `W`x`H` pattern character.
/// There is no way to set the individual color of each pseudo-pixel.
#[derive(Debug)]
struct PatternGrid<const W: usize, const H: usize> {
    /// Width of the grid in number of terminal columns
    width: u16,
    /// Height of the grid in number of terminal rows
    height: u16,
    /// Pattern and color of the cells.
    cells: Vec<PatternCell>,
    /// Lookup table mapping patterns to characters.
    char_table: &'static [char],
}

impl<const W: usize, const H: usize> PatternGrid<W, H> {
    /// Statically check that the dimension of the pattern is supported.
    const _PATTERN_DIMENSION_CHECK: usize = u8::BITS as usize - W * H;

    /// Create a new `PatternGrid` with the given width and height measured in terminal columns
    /// and rows respectively.
    fn new(width: u16, height: u16, char_table: &'static [char]) -> Self {
        // Cause a static error if the pattern doesn't fit within 8 bits.
        let _ = Self::_PATTERN_DIMENSION_CHECK;

        let length = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            cells: vec![PatternCell::default(); length],
            char_table,
        }
    }
}

impl<const W: usize, const H: usize> Grid for PatternGrid<W, H> {
    fn resolution(&self) -> (f64, f64) {
        (
            f64::from(self.width) * W as f64,
            f64::from(self.height) * H as f64,
        )
    }

    fn save(&self) -> Layer {
        let contents = self
            .cells
            .iter()
            .map(|&cell| {
                let symbol = match cell.pattern {
                    // Skip rendering blank patterns to allow layers underneath
                    // to show through.
                    0 => None,
                    idx => Some(self.char_table[idx as usize]),
                };

                LayerCell {
                    symbol,
                    fg: cell.color,
                    // Patterns only affect foreground.
                    bg: None,
                }
            })
            .collect();

        Layer { contents }
    }

    fn reset(&mut self) {
        self.cells.fill_with(Default::default);
    }

    fn paint(&mut self, x: usize, y: usize, color: Color) {
        let index = y
            .saturating_div(H)
            .saturating_mul(self.width as usize)
            .saturating_add(x.saturating_div(W));
        // using get_mut here because we are indexing the vector with usize values
        // and we want to make sure we don't panic if the index is out of bounds
        if let Some(cell) = self.cells.get_mut(index) {
            cell.pattern |= 1u8 << ((x % W) + W * (y % H));
            cell.color = Some(color);
        }
    }
}

/// The `CharGrid` is a grid made up of cells each containing a single character.
///
/// This makes it possible to draw shapes with a resolution of 1x1 dots per cell. This is useful
/// when you want to draw shapes with a low resolution.
#[derive(Debug)]
struct CharGrid {
    /// Width of the grid in number of terminal columns
    width: u16,
    /// Height of the grid in number of terminal rows
    height: u16,
    /// The color of each cell
    cells: Vec<Option<Color>>,

    /// The character to use for every cell - e.g. a block, dot, etc.
    cell_char: char,

    /// If true, apply the color to the background as well as the foreground. This is used for
    /// [`Marker::Block`], so that it will overwrite any previous foreground character, but also
    /// leave a background that can be overlaid with an additional foreground character.
    apply_color_to_bg: bool,
}

impl CharGrid {
    /// Create a new `CharGrid` with the given width and height measured in terminal columns and
    /// rows respectively.
    fn new(width: u16, height: u16, cell_char: char) -> Self {
        let length = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            cells: vec![None; length],
            cell_char,
            apply_color_to_bg: false,
        }
    }

    fn apply_color_to_bg(self) -> Self {
        Self {
            apply_color_to_bg: true,
            ..self
        }
    }
}

impl Grid for CharGrid {
    fn resolution(&self) -> (f64, f64) {
        (f64::from(self.width), f64::from(self.height))
    }

    fn save(&self) -> Layer {
        Layer {
            contents: self
                .cells
                .iter()
                .map(|&color| LayerCell {
                    symbol: color.map(|_| self.cell_char),
                    fg: color,
                    bg: color.filter(|_| self.apply_color_to_bg),
                })
                .collect(),
        }
    }

    fn reset(&mut self) {
        self.cells.fill(None);
    }

    fn paint(&mut self, x: usize, y: usize, color: Color) {
        let index = y.saturating_mul(self.width as usize).saturating_add(x);
        // using get_mut here because we are indexing the vector with usize values
        // and we want to make sure we don't panic if the index is out of bounds
        if let Some(c) = self.cells.get_mut(index) {
            *c = Some(color);
        }
    }
}

/// The `HalfBlockGrid` is a grid made up of cells each containing a half block character.
///
/// In terminals, each character is usually twice as tall as it is wide. Unicode has a couple of
/// vertical half block characters, the upper half block '▀' and lower half block '▄' which take up
/// half the height of a normal character but the full width. Together with an empty space ' ' and a
/// full block '█', we can effectively double the resolution of a single cell. In addition, because
/// each character can have a foreground and background color, we can control the color of the upper
/// and lower half of each cell. This allows us to draw shapes with a resolution of 1x2 "pixels" per
/// cell.
///
/// This allows for more flexibility than the `PatternGrid` which only supports a single
/// foreground color for each 2x4 dots cell, and the `CharGrid` which only supports a single
/// character for each cell.
#[derive(Debug)]
struct HalfBlockGrid {
    /// Width of the grid in number of terminal columns
    width: u16,
    /// Height of the grid in number of terminal rows
    height: u16,
    /// Represents a single color for each "pixel" arranged in column, row order
    pixels: Vec<Vec<Option<Color>>>,
}

impl HalfBlockGrid {
    /// Create a new `HalfBlockGrid` with the given width and height measured in terminal columns
    /// and rows respectively.
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            pixels: vec![vec![None; width as usize]; (height as usize) * 2],
        }
    }
}

impl Grid for HalfBlockGrid {
    fn resolution(&self) -> (f64, f64) {
        (f64::from(self.width), f64::from(self.height) * 2.0)
    }

    fn save(&self) -> Layer {
        // Given that we store the pixels in a grid, and that we want to use 2 pixels arranged
        // vertically to form a single terminal cell, which can be either empty, upper half block,
        // lower half block or full block, we need examine the pixels in vertical pairs to decide
        // what character to print in each cell. So these are the 4 states we use to represent each
        // cell:
        //
        // 1. upper: reset, lower: reset => ' ' fg: reset / bg: reset
        // 2. upper: reset, lower: color => '▄' fg: lower color / bg: reset
        // 3. upper: color, lower: reset => '▀' fg: upper color / bg: reset
        // 4. upper: color, lower: color => '▀' fg: upper color / bg: lower color
        //
        // Note that because the foreground reset color (i.e. default foreground color) is usually
        // not the same as the background reset color (i.e. default background color), we need to
        // swap around the colors for that state (2 reset/color).
        //
        // When the upper and lower colors are the same, we could continue to use an upper half
        // block, but we choose to use a full block instead. This allows us to write unit tests that
        // treat the cell as a single character instead of two half block characters.

        // first we join each adjacent row together to get an iterator that contains vertical pairs
        // of pixels, with the lower row being the first element in the pair
        let vertical_color_pairs = self
            .pixels
            .iter()
            .tuples()
            .flat_map(|(upper_row, lower_row)| zip(upper_row, lower_row));

        // Then we determine the character to print for each pair, along with the color of the
        // foreground and background.
        let contents = vertical_color_pairs
            .map(|(upper, lower)| {
                let (symbol, fg, bg) = match (upper, lower) {
                    (None, None) => (None, None, None),
                    (None, Some(lower)) => (Some(symbols::half_block::LOWER), Some(*lower), None),
                    (Some(upper), None) => (Some(symbols::half_block::UPPER), Some(*upper), None),
                    (Some(upper), Some(lower)) if lower == upper => {
                        (Some(symbols::half_block::FULL), Some(*upper), Some(*lower))
                    }
                    (Some(upper), Some(lower)) => {
                        (Some(symbols::half_block::UPPER), Some(*upper), Some(*lower))
                    }
                };
                LayerCell { symbol, fg, bg }
            })
            .collect();

        Layer { contents }
    }

    fn reset(&mut self) {
        self.pixels.fill(vec![None; self.width as usize]);
    }

    fn paint(&mut self, x: usize, y: usize, color: Color) {
        self.pixels[y][x] = Some(color);
    }
}

/// Painter is an abstraction over the [`Context`] that allows to draw shapes on the grid.
///
/// It is used by the [`Shape`] trait to draw shapes on the grid. It can be useful to think of this
/// as similar to the [`Buffer`] struct that is used to draw widgets on the terminal.
#[derive(Debug)]
pub struct Painter<'a, 'b> {
    context: &'a mut Context<'b>,
    resolution: (f64, f64),
}

impl Painter<'_, '_> {
    /// Convert the `(x, y)` coordinates to location of a point on the grid
    ///
    /// `(x, y)` coordinates are expressed in the coordinate system of the canvas. The origin is in
    /// the lower left corner of the canvas (unlike most other coordinates in `Ratatui` where the
    /// origin is the upper left corner). The `x` and `y` bounds of the canvas define the specific
    /// area of some coordinate system that will be drawn on the canvas. The resolution of the grid
    /// is used to convert the `(x, y)` coordinates to the location of a point on the grid.
    ///
    /// The grid coordinates are expressed in the coordinate system of the grid. The origin is in
    /// the top left corner of the grid. The x and y bounds of the grid are always `[0, width - 1]`
    /// and `[0, height - 1]` respectively. The resolution of the grid is used to convert the
    /// `(x, y)` coordinates to the location of a point on the grid.
    ///
    /// Points are rounded to the nearest grid cell (with points exactly in the center of a cell
    /// rounding up).
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::symbols;
    /// use ratatui::widgets::canvas::{Context, Painter};
    ///
    /// let mut ctx = Context::new(2, 2, [1.0, 2.0], [0.0, 2.0], symbols::Marker::Braille);
    /// let mut painter = Painter::from(&mut ctx);
    ///
    /// let point = painter.get_point(1.0, 0.0);
    /// assert_eq!(point, Some((0, 7)));
    ///
    /// let point = painter.get_point(1.5, 1.0);
    /// assert_eq!(point, Some((2, 4)));
    ///
    /// let point = painter.get_point(0.0, 0.0);
    /// assert_eq!(point, None);
    ///
    /// let point = painter.get_point(2.0, 2.0);
    /// assert_eq!(point, Some((3, 0)));
    ///
    /// let point = painter.get_point(1.0, 2.0);
    /// assert_eq!(point, Some((0, 0)));
    /// ```
    pub fn get_point(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let [left, right] = self.context.x_bounds;
        let [bottom, top] = self.context.y_bounds;
        if x < left || x > right || y < bottom || y > top {
            return None;
        }
        let width = right - left;
        let height = top - bottom;
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let x = ((x - left) * (self.resolution.0 - 1.0) / width).round() as usize;
        let y = ((top - y) * (self.resolution.1 - 1.0) / height).round() as usize;
        Some((x, y))
    }

    /// Paint a point of the grid
    ///
    /// # Example
    ///
    /// ```
    /// use ratatui::style::Color;
    /// use ratatui::symbols;
    /// use ratatui::widgets::canvas::{Context, Painter};
    ///
    /// let mut ctx = Context::new(1, 1, [0.0, 2.0], [0.0, 2.0], symbols::Marker::Braille);
    /// let mut painter = Painter::from(&mut ctx);
    /// painter.paint(1, 3, Color::Red);
    /// ```
    pub fn paint(&mut self, x: usize, y: usize, color: Color) {
        self.context.grid.paint(x, y, color);
    }

    /// Canvas context bounds by axis.
    ///
    /// # Example
    ///
    /// ```
    /// use ratatui::style::Color;
    /// use ratatui::symbols;
    /// use ratatui::widgets::canvas::{Context, Painter};
    ///
    /// let mut ctx = Context::new(1, 1, [0.0, 2.0], [0.0, 2.0], symbols::Marker::Braille);
    /// let mut painter = Painter::from(&mut ctx);
    /// assert_eq!(painter.bounds(), (&[0.0, 2.0], &[0.0, 2.0]));
    /// ```
    pub const fn bounds(&self) -> (&[f64; 2], &[f64; 2]) {
        (&self.context.x_bounds, &self.context.y_bounds)
    }
}

impl<'a, 'b> From<&'a mut Context<'b>> for Painter<'a, 'b> {
    fn from(context: &'a mut Context<'b>) -> Self {
        let resolution = context.grid.resolution();
        Self {
            context,
            resolution,
        }
    }
}

/// Holds the state of the [`Canvas`] when painting to it.
///
/// This is used by the [`Canvas`] widget to draw shapes on the grid. It can be useful to think of
/// this as similar to the `Frame` struct that is used to draw widgets on the terminal.
#[derive(Debug)]
pub struct Context<'a> {
    // Width of the canvas in cells.
    //
    // This is NOT the resolution in dots/pixels as this varies by marker type.
    width: u16,
    // Height of the canvas in cells.
    //
    // This is NOT the resolution in dots/pixels as this varies by marker type.
    height: u16,
    // Canvas coordinate system width
    x_bounds: [f64; 2],
    // Canvas coordinate system height
    y_bounds: [f64; 2],
    grid: Box<dyn Grid>,
    dirty: bool,
    layers: Vec<Layer>,
    labels: Vec<Label<'a>>,
}

impl<'a> Context<'a> {
    /// Create a new Context with the given width and height measured in terminal columns and rows
    /// respectively. The `x` and `y` bounds define the specific area of some coordinate system that
    /// will be drawn on the canvas. The marker defines the type of points used to draw the shapes.
    ///
    /// Applications should not use this directly but rather use the [`Canvas`] widget. This will be
    /// created by the [`Canvas::paint`] method and passed to the closure that is used to draw on
    /// the canvas.
    ///
    /// The `x` and `y` bounds should be specified as left/right and bottom/top respectively. For
    /// example, if you want to draw a map of the world, you might want to use the following bounds:
    ///
    /// ```
    /// use ratatui::symbols;
    /// use ratatui::widgets::canvas::Context;
    ///
    /// let ctx = Context::new(
    ///     100,
    ///     100,
    ///     [-180.0, 180.0],
    ///     [-90.0, 90.0],
    ///     symbols::Marker::Braille,
    /// );
    /// ```
    pub fn new(
        width: u16,
        height: u16,
        x_bounds: [f64; 2],
        y_bounds: [f64; 2],
        marker: Marker,
    ) -> Self {
        let grid = Self::marker_to_grid(width, height, marker);
        Self {
            width,
            height,
            x_bounds,
            y_bounds,
            grid,
            dirty: false,
            layers: Vec::new(),
            labels: Vec::new(),
        }
    }

    fn marker_to_grid(width: u16, height: u16, marker: Marker) -> Box<dyn Grid> {
        let dot = symbols::DOT.chars().next().unwrap();
        let block = symbols::block::FULL.chars().next().unwrap();
        let bar = symbols::bar::HALF.chars().next().unwrap();
        match marker {
            Marker::Block => Box::new(CharGrid::new(width, height, block).apply_color_to_bg()),
            Marker::Bar => Box::new(CharGrid::new(width, height, bar)),
            Marker::Braille => Box::new(PatternGrid::<2, 4>::new(width, height, &BRAILLE)),
            Marker::HalfBlock => Box::new(HalfBlockGrid::new(width, height)),
            Marker::Quadrant => Box::new(PatternGrid::<2, 2>::new(width, height, &QUADRANTS)),
            Marker::Sextant => Box::new(PatternGrid::<2, 3>::new(width, height, &SEXTANTS)),
            Marker::Octant => Box::new(PatternGrid::<2, 4>::new(width, height, &OCTANTS)),
            Marker::Dot | _ => Box::new(CharGrid::new(width, height, dot)),
        }
    }

    /// Change the marker being used in this context.
    ///
    /// This will save the last layer if necessary and reset the grid to use the new marker.
    pub fn marker(&mut self, marker: Marker) {
        self.finish();
        self.grid = Self::marker_to_grid(self.width, self.height, marker);
    }

    /// Draw the given [`Shape`] in this context
    pub fn draw<S>(&mut self, shape: &S)
    where
        S: Shape,
    {
        self.dirty = true;
        let mut painter = Painter::from(self);
        shape.draw(&mut painter);
    }

    /// Save the existing state of the grid as a layer.
    ///
    /// Save the existing state as a layer to be rendered and reset the grid to its initial
    /// state for the next layer.
    ///
    /// This allows the canvas to be drawn in multiple layers. This is useful if you want to
    /// draw multiple shapes on the [`Canvas`] in specific order.
    pub fn layer(&mut self) {
        self.layers.push(self.grid.save());
        self.grid.reset();
        self.dirty = false;
    }

    /// Print a [`Text`] on the [`Canvas`] at the given position.
    ///
    /// Note that the text is always printed on top of the canvas and is **not** affected by the
    /// layers.
    ///
    /// [`Text`]: ratatui_core::text::Text
    pub fn print<T>(&mut self, x: f64, y: f64, line: T)
    where
        T: Into<TextLine<'a>>,
    {
        self.labels.push(Label {
            x,
            y,
            line: line.into(),
        });
    }

    /// Save the last layer if necessary
    fn finish(&mut self) {
        if self.dirty {
            self.layer();
        }
    }
}

/// The Canvas widget provides a means to draw shapes (Lines, Rectangles, Circles, etc.) on a grid.
///
/// By default the grid is made of Braille patterns but you may change the marker to use a different
/// set of symbols. If your terminal or font does not support this unicode block, you will see
/// unicode replacement characters (�) instead of braille dots. The Braille patterns (as well the
/// octant character patterns) provide a more fine grained result with a 2x4 resolution per
/// character, but you might want to use a simple dot, block, or bar instead by calling the
/// [`marker`] method if your target environment does not support those symbols.
///
/// See [Unicode Braille Patterns](https://en.wikipedia.org/wiki/Braille_Patterns) for more info.
///
/// The `Octant` marker is similar to the `Braille` marker but, instead of sparse dots, displays
/// densely packed and regularly spaced pseudo-pixels, without visible bands between rows and
/// columns. However, it uses characters that are not yet as widely supported as the Braille
/// unicode block.
///
/// The `Quadrant` and `Sextant` markers are in turn akin to the `Octant` marker, but with a 2x2
/// and 2x3 resolution, respectively.
///
/// The `HalfBlock` marker is useful when you want to draw shapes with a higher resolution than a
/// `CharGrid` but lower than a `PatternGrid`. This grid type supports a foreground and background
/// color for each terminal cell. This allows for more flexibility than the `PatternGrid` which
/// only supports a single foreground color for each 2x4 dots cell.
///
/// The Canvas widget is used by calling the [`Canvas::paint`] method and passing a closure that
/// will be used to draw on the canvas. The closure will be passed a [`Context`] object that can be
/// used to draw shapes on the canvas.
///
/// The [`Context`] object provides a [`Context::draw`] method that can be used to draw shapes on
/// the canvas. The [`Context::layer`] method can be used to save the current state of the canvas
/// and start a new layer. This is useful if you want to draw multiple shapes on the canvas in
/// specific order. The [`Context`] object also provides a [`Context::print`] method that can be
/// used to print text on the canvas. Note that the text is always printed on top of the canvas and
/// is not affected by the layers.
///
/// # Examples
///
/// ```
/// use ratatui::style::Color;
/// use ratatui::widgets::Block;
/// use ratatui::widgets::canvas::{Canvas, Line, Map, MapResolution, Rectangle};
///
/// Canvas::default()
///     .block(Block::bordered().title("Canvas"))
///     .x_bounds([-180.0, 180.0])
///     .y_bounds([-90.0, 90.0])
///     .paint(|ctx| {
///         ctx.draw(&Map {
///             resolution: MapResolution::High,
///             color: Color::White,
///         });
///         ctx.layer();
///         ctx.draw(&Line {
///             x1: 0.0,
///             y1: 10.0,
///             x2: 10.0,
///             y2: 10.0,
///             color: Color::White,
///         });
///         ctx.draw(&Rectangle {
///             x: 10.0,
///             y: 20.0,
///             width: 10.0,
///             height: 10.0,
///             color: Color::Red,
///         });
///     });
/// ```
///
/// [`marker`]: #method.marker
#[derive(Debug, Clone, PartialEq)]
pub struct Canvas<'a, F>
where
    F: Fn(&mut Context),
{
    block: Option<Block<'a>>,
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    paint_func: Option<F>,
    background_color: Color,
    marker: Marker,
}

impl<F> Default for Canvas<'_, F>
where
    F: Fn(&mut Context),
{
    fn default() -> Self {
        Self {
            block: None,
            x_bounds: [0.0, 0.0],
            y_bounds: [0.0, 0.0],
            paint_func: None,
            background_color: Color::Reset,
            marker: Marker::Braille,
        }
    }
}

impl<'a, F> Canvas<'a, F>
where
    F: Fn(&mut Context),
{
    /// Wraps the canvas with a custom [`Block`] widget.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Define the viewport of the canvas.
    ///
    /// If you were to "zoom" to a certain part of the world you may want to choose different
    /// bounds.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn x_bounds(mut self, bounds: [f64; 2]) -> Self {
        self.x_bounds = bounds;
        self
    }

    /// Define the viewport of the canvas.
    ///
    /// If you were to "zoom" to a certain part of the world you may want to choose different
    /// bounds.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn y_bounds(mut self, bounds: [f64; 2]) -> Self {
        self.y_bounds = bounds;
        self
    }

    /// Store the closure that will be used to draw to the [`Canvas`]
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn paint(mut self, f: F) -> Self {
        self.paint_func = Some(f);
        self
    }

    /// Change the background [`Color`] of the entire canvas
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    /// Change the type of points used to draw the shapes.
    ///
    /// By default the [`Braille`] patterns are used as they provide a more fine grained result,
    /// but you might want to use the simple [`Dot`] or [`Block`] instead if the targeted terminal
    /// does not support those symbols.
    ///
    /// The [`HalfBlock`] marker is useful when you want to draw shapes with a higher resolution
    /// than with a grid of characters (e.g. with [`Block`] or [`Dot`]) but lower than with
    /// [`Braille`]. This grid type supports a foreground and background color for each terminal
    /// cell. This allows for more flexibility than the `PatternGrid` which only supports a single
    /// foreground color for each 2x4 dots cell.
    ///
    /// [`Braille`]: ratatui_core::symbols::Marker::Braille
    /// [`HalfBlock`]: ratatui_core::symbols::Marker::HalfBlock
    /// [`Dot`]: ratatui_core::symbols::Marker::Dot
    /// [`Block`]: ratatui_core::symbols::Marker::Block
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::symbols;
    /// use ratatui::widgets::canvas::Canvas;
    ///
    /// Canvas::default()
    ///     .marker(symbols::Marker::Braille)
    ///     .paint(|ctx| {});
    ///
    /// Canvas::default()
    ///     .marker(symbols::Marker::HalfBlock)
    ///     .paint(|ctx| {});
    ///
    /// Canvas::default()
    ///     .marker(symbols::Marker::Dot)
    ///     .paint(|ctx| {});
    ///
    /// Canvas::default()
    ///     .marker(symbols::Marker::Block)
    ///     .paint(|ctx| {});
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn marker(mut self, marker: Marker) -> Self {
        self.marker = marker;
        self
    }
}

impl<F> Widget for Canvas<'_, F>
where
    F: Fn(&mut Context),
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl<F> Widget for &Canvas<'_, F>
where
    F: Fn(&mut Context),
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.block.as_ref().render(area, buf);
        let canvas_area = self.block.inner_if_some(area);
        if canvas_area.is_empty() {
            return;
        }

        buf.set_style(canvas_area, Style::default().bg(self.background_color));

        let width = canvas_area.width as usize;

        let Some(ref painter) = self.paint_func else {
            return;
        };

        // Create a blank context that match the size of the canvas
        let mut ctx = Context::new(
            canvas_area.width,
            canvas_area.height,
            self.x_bounds,
            self.y_bounds,
            self.marker,
        );
        // Paint to this context
        painter(&mut ctx);
        ctx.finish();

        // Retrieve painted points for each layer
        for layer in ctx.layers {
            for (index, layer_cell) in layer.contents.iter().enumerate() {
                let (x, y) = (
                    (index % width) as u16 + canvas_area.left(),
                    (index / width) as u16 + canvas_area.top(),
                );
                let cell = &mut buf[(x, y)];

                if let Some(symbol) = layer_cell.symbol {
                    cell.set_char(symbol);
                }
                if let Some(fg) = layer_cell.fg {
                    cell.set_fg(fg);
                }
                if let Some(bg) = layer_cell.bg {
                    cell.set_bg(bg);
                }
            }
        }

        // Finally draw the labels
        let left = self.x_bounds[0];
        let right = self.x_bounds[1];
        let top = self.y_bounds[1];
        let bottom = self.y_bounds[0];
        let width = (self.x_bounds[1] - self.x_bounds[0]).abs();
        let height = (self.y_bounds[1] - self.y_bounds[0]).abs();
        let resolution = {
            let width = f64::from(canvas_area.width - 1);
            let height = f64::from(canvas_area.height - 1);
            (width, height)
        };
        for label in ctx
            .labels
            .iter()
            .filter(|l| l.x >= left && l.x <= right && l.y <= top && l.y >= bottom)
        {
            let x = ((label.x - left) * resolution.0 / width) as u16 + canvas_area.left();
            let y = ((top - label.y) * resolution.1 / height) as u16 + canvas_area.top();
            buf.set_line(x, y, &label.line, canvas_area.right() - x);
        }
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use ratatui_core::buffer::Cell;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::block(Marker::Block, indoc!(
                "
                █xxxx
                █xxxx
                █xxxx
                █xxxx
                █████"
            ))]
    #[case::half_block(Marker::HalfBlock, indoc!(
                "
                █xxxx
                █xxxx
                █xxxx
                █xxxx
                █▄▄▄▄"
            ))]
    #[case::bar(Marker::Bar, indoc!(
                "
                ▄xxxx
                ▄xxxx
                ▄xxxx
                ▄xxxx
                ▄▄▄▄▄"
            ))]
    #[case::braille(Marker::Braille, indoc!(
                "
                ⡇xxxx
                ⡇xxxx
                ⡇xxxx
                ⡇xxxx
                ⣇⣀⣀⣀⣀"
            ))]
    #[case::quadrant(Marker::Quadrant, indoc!(
                "
                ▌xxxx
                ▌xxxx
                ▌xxxx
                ▌xxxx
                ▙▄▄▄▄"
            ))]
    #[case::sextant(Marker::Sextant, indoc!(
                "
                ▌xxxx
                ▌xxxx
                ▌xxxx
                ▌xxxx
                🬲🬭🬭🬭🬭"
            ))]
    #[case::octant(Marker::Octant, indoc!(
                "
                ▌xxxx
                ▌xxxx
                ▌xxxx
                ▌xxxx
                𜷀▂▂▂▂"
            ))]
    #[case::dot(Marker::Dot, indoc!(
                "
                •xxxx
                •xxxx
                •xxxx
                •xxxx
                •••••"
            ))]
    fn test_horizontal_with_vertical(#[case] marker: Marker, #[case] expected: &'static str) {
        let area = Rect::new(0, 0, 5, 5);
        let mut buf = Buffer::filled(area, Cell::new("x"));
        let horizontal_line = Line {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 0.0,
            color: Color::Reset,
        };
        let vertical_line = Line {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 10.0,
            color: Color::Reset,
        };
        Canvas::default()
            .marker(marker)
            .paint(|ctx| {
                ctx.draw(&vertical_line);
                ctx.draw(&horizontal_line);
            })
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .render(area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(expected.lines()));
    }

    #[rstest]
    #[case::block(Marker::Block, indoc!(
                "
                █xxx█
                x█x█x
                xx█xx
                x█x█x
                █xxx█"))]
    #[case::half_block(Marker::HalfBlock,
           indoc!(
                "
                █xxx█
                x█x█x
                xx█xx
                x█x█x
                █xxx█")
    )]
    #[case::bar(Marker::Bar, indoc!(
                "
                ▄xxx▄
                x▄x▄x
                xx▄xx
                x▄x▄x
                ▄xxx▄"))]
    #[case::braille(Marker::Braille, indoc!(
                "
                ⢣xxx⡜
                x⢣x⡜x
                xx⣿xx
                x⡜x⢣x
                ⡜xxx⢣"
            ))]
    #[case::quadrant(Marker::Quadrant, indoc!(
                "
                ▚xxx▞
                x▚x▞x
                xx█xx
                x▞x▚x
                ▞xxx▚"
            ))]
    #[case::sextant(Marker::Sextant, indoc!(
                "
                🬧xxx🬔
                x🬧x🬔x
                xx█xx
                x🬘x🬣x
                🬘xxx🬣"
            ))]
    #[case::octant(Marker::Octant, indoc!(
                "
                ▚xxx▞
                x▚x▞x
                xx█xx
                x▞x▚x
                ▞xxx▚"
            ))]
    #[case::dot(Marker::Dot, indoc!(
                "
                •xxx•
                x•x•x
                xx•xx
                x•x•x
                •xxx•"
            ))]
    fn test_diagonal_lines(#[case] marker: Marker, #[case] expected: &'static str) {
        let area = Rect::new(0, 0, 5, 5);
        let mut buf = Buffer::filled(area, Cell::new("x"));
        let diagonal_up = Line {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
            color: Color::Reset,
        };
        let diagonal_down = Line {
            x1: 0.0,
            y1: 10.0,
            x2: 10.0,
            y2: 0.0,
            color: Color::Reset,
        };
        Canvas::default()
            .marker(marker)
            .paint(|ctx| {
                ctx.draw(&diagonal_down);
                ctx.draw(&diagonal_up);
            })
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .render(area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(expected.lines()));
    }

    // The canvas methods work a lot with arithmetic so here we enter various width and height
    // values to check if there are any integer overflows we just initialize the canvas painters
    #[test]
    fn check_canvas_paint_max() {
        let mut b_grid = PatternGrid::<2, 4>::new(u16::MAX, 2, &OCTANTS);
        let mut c_grid = CharGrid::new(u16::MAX, 2, 'd');

        let max = u16::MAX as usize;

        b_grid.paint(0, 0, Color::Red);
        b_grid.paint(0, max, Color::Red);
        b_grid.paint(max, 0, Color::Red);
        b_grid.paint(max, max, Color::Red);

        c_grid.paint(0, 0, Color::Red);
        c_grid.paint(0, max, Color::Red);
        c_grid.paint(max, 0, Color::Red);
        c_grid.paint(max, max, Color::Red);
    }

    // We delibately cause integer overflow to check if we don't panic and don't get weird behavior
    #[test]
    fn check_canvas_paint_overflow() {
        let mut b_grid = PatternGrid::<2, 4>::new(u16::MAX, 3, &BRAILLE);
        let mut c_grid = CharGrid::new(u16::MAX, 3, 'd');

        let max = u16::MAX as usize + 10;

        // see if we can paint outside bounds
        b_grid.paint(max, max, Color::Red);
        c_grid.paint(max, max, Color::Red);
        // see if we can paint usize max bounds
        b_grid.paint(usize::MAX, usize::MAX, Color::Red);
        c_grid.paint(usize::MAX, usize::MAX, Color::Red);
    }

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let canvas = Canvas::default()
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .paint(|_ctx| {});
        // This should not panic, even if the buffer is too small to render the canvas.
        canvas.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines([" "]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let canvas = Canvas::default()
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .paint(|_ctx| {});
        // This should not panic, even if the buffer has zero size.
        canvas.render(buffer.area, &mut buffer);
    }
}

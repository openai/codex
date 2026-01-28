#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStyle {
    pub fg: Option<RenderColor>,
    pub bg: Option<RenderColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderColor {
    Default,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    LightBlue,
    DarkGray,
    Rgb(u8, u8, u8),
    Indexed(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderCell {
    pub content: String,
    pub style: RenderStyle,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderLine {
    pub spans: Vec<RenderCell>,
    pub alignment: Option<RenderAlignment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderParagraph {
    pub lines: Vec<RenderLine>,
    pub wrap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStyleBuilder {
    fg: Option<RenderColor>,
    bg: Option<RenderColor>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderLineBuilder {
    cells: Vec<RenderCell>,
}

impl RenderStyle {
    /// Returns a builder for constructing a render style.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Builder for render style configuration.
    pub fn builder() -> RenderStyleBuilder {
        RenderStyleBuilder::default()
    }

    /// Overlays another style on top of this one.
    ///
    /// # Arguments
    /// - `overlay` (RenderStyle): Style to overlay.
    ///
    /// # Returns
    /// - `RenderStyle`: Combined style.
    pub fn patch(self, overlay: RenderStyle) -> RenderStyle {
        merge_styles(self, overlay)
    }
}

impl RenderStyleBuilder {
    /// Sets the foreground color for the style.
    ///
    /// # Arguments
    /// - `fg` (RenderColor): Foreground color to apply.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn fg(mut self, fg: RenderColor) -> Self {
        self.fg = Some(fg);
        self
    }

    /// Sets the background color for the style.
    ///
    /// # Arguments
    /// - `bg` (RenderColor): Background color to apply.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn bg(mut self, bg: RenderColor) -> Self {
        self.bg = Some(bg);
        self
    }

    /// Enables bold styling.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Enables dim styling.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// Enables italic styling.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Enables underline styling.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Enables strikethrough styling.
    ///
    /// # Returns
    /// - `RenderStyleBuilder`: Updated builder.
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Builds the render style.
    ///
    /// # Returns
    /// - `RenderStyle`: Fully constructed style.
    pub fn build(self) -> RenderStyle {
        RenderStyle {
            fg: self.fg,
            bg: self.bg,
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
        }
    }
}

impl RenderCell {
    /// Creates a new render cell with text and style.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `style` (RenderStyle): Style applied to the cell.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn new(text: impl Into<String>, style: RenderStyle) -> Self {
        Self {
            content: text.into(),
            style,
        }
    }

    /// Alias for creating a styled render cell.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `style` (RenderStyle): Style applied to the cell.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn styled(text: impl Into<String>, style: RenderStyle) -> Self {
        Self::new(text, style)
    }

    /// Creates a raw render cell with default styling.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn raw(text: impl Into<String>) -> Self {
        Self::plain(text)
    }

    /// Applies an overlay style to the cell.
    ///
    /// # Arguments
    /// - `style` (RenderStyle): Style to overlay.
    ///
    /// # Returns
    /// - `RenderCell`: Updated render cell.
    pub fn patch_style(mut self, style: RenderStyle) -> Self {
        self.style = merge_styles(self.style, style);
        self
    }

    /// Applies a style to the cell.
    ///
    /// # Arguments
    /// - `style` (RenderStyle): Style to apply.
    ///
    /// # Returns
    /// - `RenderCell`: Updated render cell.
    pub fn style(self, style: RenderStyle) -> Self {
        self.patch_style(style)
    }

    /// Creates a plain render cell with default styling.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, RenderStyle::default())
    }

    /// Creates a dim render cell.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn dim(text: impl Into<String>) -> Self {
        Self::new(text, RenderStyle::builder().dim().build())
    }

    /// Creates a bold render cell.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn bold(text: impl Into<String>) -> Self {
        Self::new(text, RenderStyle::builder().bold().build())
    }

    /// Creates a colored render cell.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `color` (RenderColor): Foreground color to apply.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn color(text: impl Into<String>, color: RenderColor) -> Self {
        Self::new(text, RenderStyle::builder().fg(color).build())
    }

    /// Creates a bold colored render cell.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `color` (RenderColor): Foreground color to apply.
    ///
    /// # Returns
    /// - `RenderCell`: Newly constructed render cell.
    pub fn color_bold(text: impl Into<String>, color: RenderColor) -> Self {
        Self::new(text, RenderStyle::builder().fg(color).bold().build())
    }

    /// Returns the display width of the cell contents.
    ///
    /// # Returns
    /// - `usize`: The display width in columns.
    pub fn width(&self) -> usize {
        unicode_width::UnicodeWidthStr::width(self.content.as_str())
    }
}

impl RenderLine {
    /// Creates a new render line from a list of cells.
    ///
    /// # Arguments
    /// - `cells` (Vec<RenderCell>): Cells to place on the line.
    ///
    /// # Returns
    /// - `RenderLine`: Newly constructed render line.
    pub fn new(cells: Vec<RenderCell>) -> Self {
        Self {
            spans: cells,
            alignment: None,
        }
    }

    /// Returns a builder for constructing a render line.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Builder for render line configuration.
    pub fn builder() -> RenderLineBuilder {
        RenderLineBuilder::new()
    }

    /// Appends a cell to the line.
    ///
    /// # Arguments
    /// - `cell` (RenderCell): Cell to append.
    pub fn push_cell(&mut self, cell: RenderCell) {
        self.spans.push(cell);
    }

    /// Appends a span-like cell to the line.
    ///
    /// # Arguments
    /// - `span` (impl Into<RenderCell>): Cell to append.
    pub fn push_span(&mut self, span: impl Into<RenderCell>) {
        self.spans.push(span.into());
    }

    /// Applies a style to every cell in the line.
    ///
    /// # Arguments
    /// - `style` (RenderStyle): Style to overlay on existing styles.
    ///
    /// # Returns
    /// - `RenderLine`: Line with updated styles.
    pub fn with_style(mut self, style: RenderStyle) -> Self {
        for cell in &mut self.spans {
            cell.style = merge_styles(cell.style, style);
        }
        self
    }

    /// Applies a style to every cell in the line.
    ///
    /// # Arguments
    /// - `style` (RenderStyle): Style to overlay on existing styles.
    ///
    /// # Returns
    /// - `RenderLine`: Line with updated styles.
    pub fn style(self, style: RenderStyle) -> Self {
        self.with_style(style)
    }

    /// Sets the alignment for the line.
    ///
    /// # Arguments
    /// - `alignment` (RenderAlignment): Alignment to apply.
    ///
    /// # Returns
    /// - `RenderLine`: Line with updated alignment.
    pub fn alignment(mut self, alignment: RenderAlignment) -> Self {
        self.alignment = Some(alignment);
        self
    }

    pub fn red(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::Red).build())
    }

    pub fn green(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::Green).build())
    }

    pub fn yellow(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::Yellow).build())
    }

    pub fn blue(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::Blue).build())
    }

    pub fn magenta(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::Magenta).build())
    }

    pub fn cyan(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::Cyan).build())
    }

    pub fn light_blue(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::LightBlue).build())
    }

    pub fn dark_gray(self) -> Self {
        self.with_style(RenderStyle::builder().fg(RenderColor::DarkGray).build())
    }

    pub fn bold(self) -> Self {
        self.with_style(RenderStyle::builder().bold().build())
    }

    pub fn dim(self) -> Self {
        self.with_style(RenderStyle::builder().dim().build())
    }

    pub fn italic(self) -> Self {
        self.with_style(RenderStyle::builder().italic().build())
    }

    pub fn underlined(self) -> Self {
        self.with_style(RenderStyle::builder().underline().build())
    }

    pub fn crossed_out(self) -> Self {
        self.with_style(RenderStyle::builder().strikethrough().build())
    }

    /// Returns the total display width of the line.
    ///
    /// # Returns
    /// - `usize`: The display width in columns.
    pub fn width(&self) -> usize {
        self.spans.iter().map(RenderCell::width).sum()
    }
}

impl RenderLineBuilder {
    /// Creates a new render line builder.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Builder with no cells.
    pub fn new() -> Self {
        Self { cells: Vec::new() }
    }

    /// Appends a cell to the line builder.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `style` (RenderStyle): Style applied to the cell.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Updated builder.
    pub fn cell(mut self, text: impl Into<String>, style: RenderStyle) -> Self {
        self.cells.push(RenderCell::new(text, style));
        self
    }

    /// Appends a plain cell to the line builder.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Updated builder.
    pub fn cell_plain(self, text: impl Into<String>) -> Self {
        self.cell(text, RenderStyle::default())
    }

    /// Appends a dim cell to the line builder.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Updated builder.
    pub fn cell_dim(self, text: impl Into<String>) -> Self {
        self.cell(text, RenderStyle::builder().dim().build())
    }

    /// Appends a bold cell to the line builder.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Updated builder.
    pub fn cell_bold(self, text: impl Into<String>) -> Self {
        self.cell(text, RenderStyle::builder().bold().build())
    }

    /// Appends a colored cell to the line builder.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `color` (RenderColor): Foreground color to apply.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Updated builder.
    pub fn cell_color(self, text: impl Into<String>, color: RenderColor) -> Self {
        self.cell(text, RenderStyle::builder().fg(color).build())
    }

    /// Appends a bold colored cell to the line builder.
    ///
    /// # Arguments
    /// - `text` (impl Into<String>): Cell content.
    /// - `color` (RenderColor): Foreground color to apply.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Updated builder.
    pub fn cell_color_bold(self, text: impl Into<String>, color: RenderColor) -> Self {
        self.cell(text, RenderStyle::builder().fg(color).bold().build())
    }

    /// Builds the render line.
    ///
    /// # Returns
    /// - `RenderLine`: Fully constructed render line.
    pub fn build(self) -> RenderLine {
        RenderLine::new(self.cells)
    }
}

impl RenderParagraph {
    /// Creates a new render paragraph.
    ///
    /// # Arguments
    /// - `lines` (Vec<RenderLine>): Lines in the paragraph.
    /// - `wrap` (bool): Whether to wrap lines in the backend renderer.
    ///
    /// # Returns
    /// - `RenderParagraph`: Newly constructed render paragraph.
    pub fn new(lines: Vec<RenderLine>, wrap: bool) -> Self {
        Self { lines, wrap }
    }
}

impl From<&str> for RenderLine {
    fn from(value: &str) -> Self {
        RenderLine::new(vec![RenderCell::plain(value)])
    }
}

impl From<String> for RenderLine {
    fn from(value: String) -> Self {
        RenderLine::new(vec![RenderCell::plain(value)])
    }
}

impl From<&str> for RenderCell {
    fn from(value: &str) -> Self {
        RenderCell::plain(value)
    }
}

impl From<String> for RenderCell {
    fn from(value: String) -> Self {
        RenderCell::plain(value)
    }
}

impl From<Vec<RenderCell>> for RenderLine {
    fn from(value: Vec<RenderCell>) -> Self {
        RenderLine::new(value)
    }
}

impl From<RenderCell> for RenderLine {
    fn from(value: RenderCell) -> Self {
        RenderLine::new(vec![value])
    }
}

impl FromIterator<RenderCell> for RenderLine {
    fn from_iter<I: IntoIterator<Item = RenderCell>>(iter: I) -> Self {
        RenderLine::new(iter.into_iter().collect())
    }
}

impl<'a> FromIterator<&'a str> for RenderLine {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        RenderLine::new(iter.into_iter().map(RenderCell::from).collect())
    }
}

impl FromIterator<String> for RenderLine {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        RenderLine::new(iter.into_iter().map(RenderCell::from).collect())
    }
}

fn merge_styles(base: RenderStyle, overlay: RenderStyle) -> RenderStyle {
    RenderStyle {
        fg: overlay.fg.or(base.fg),
        bg: overlay.bg.or(base.bg),
        bold: base.bold || overlay.bold,
        dim: base.dim || overlay.dim,
        italic: base.italic || overlay.italic,
        underline: base.underline || overlay.underline,
        strikethrough: base.strikethrough || overlay.strikethrough,
    }
}

pub trait RenderStylize: Sized {
    fn red(self) -> RenderCell;
    fn green(self) -> RenderCell;
    fn yellow(self) -> RenderCell;
    fn blue(self) -> RenderCell;
    fn magenta(self) -> RenderCell;
    fn cyan(self) -> RenderCell;
    fn light_blue(self) -> RenderCell;
    fn dark_gray(self) -> RenderCell;
    fn bold(self) -> RenderCell;
    fn dim(self) -> RenderCell;
    fn italic(self) -> RenderCell;
    fn underlined(self) -> RenderCell;
    fn crossed_out(self) -> RenderCell;
}

impl RenderStylize for &str {
    fn red(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Red)
    }
    fn green(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Green)
    }
    fn yellow(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Yellow)
    }
    fn blue(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Blue)
    }
    fn magenta(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Magenta)
    }
    fn cyan(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Cyan)
    }
    fn light_blue(self) -> RenderCell {
        RenderCell::color(self, RenderColor::LightBlue)
    }
    fn dark_gray(self) -> RenderCell {
        RenderCell::color(self, RenderColor::DarkGray)
    }
    fn bold(self) -> RenderCell {
        RenderCell::bold(self)
    }
    fn dim(self) -> RenderCell {
        RenderCell::dim(self)
    }
    fn italic(self) -> RenderCell {
        RenderCell::new(self, RenderStyle::builder().italic().build())
    }
    fn underlined(self) -> RenderCell {
        RenderCell::new(self, RenderStyle::builder().underline().build())
    }
    fn crossed_out(self) -> RenderCell {
        RenderCell::new(self, RenderStyle::builder().strikethrough().build())
    }
}

impl RenderStylize for String {
    fn red(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Red)
    }
    fn green(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Green)
    }
    fn yellow(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Yellow)
    }
    fn blue(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Blue)
    }
    fn magenta(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Magenta)
    }
    fn cyan(self) -> RenderCell {
        RenderCell::color(self, RenderColor::Cyan)
    }
    fn light_blue(self) -> RenderCell {
        RenderCell::color(self, RenderColor::LightBlue)
    }
    fn dark_gray(self) -> RenderCell {
        RenderCell::color(self, RenderColor::DarkGray)
    }
    fn bold(self) -> RenderCell {
        RenderCell::bold(self)
    }
    fn dim(self) -> RenderCell {
        RenderCell::dim(self)
    }
    fn italic(self) -> RenderCell {
        RenderCell::new(self, RenderStyle::builder().italic().build())
    }
    fn underlined(self) -> RenderCell {
        RenderCell::new(self, RenderStyle::builder().underline().build())
    }
    fn crossed_out(self) -> RenderCell {
        RenderCell::new(self, RenderStyle::builder().strikethrough().build())
    }
}

impl RenderStylize for RenderCell {
    fn red(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::Red).build(),
        );
        self
    }
    fn green(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::Green).build(),
        );
        self
    }
    fn yellow(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::Yellow).build(),
        );
        self
    }
    fn blue(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::Blue).build(),
        );
        self
    }
    fn magenta(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::Magenta).build(),
        );
        self
    }
    fn cyan(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::Cyan).build(),
        );
        self
    }
    fn light_blue(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::LightBlue).build(),
        );
        self
    }
    fn dark_gray(mut self) -> RenderCell {
        self.style = merge_styles(
            self.style,
            RenderStyle::builder().fg(RenderColor::DarkGray).build(),
        );
        self
    }
    fn bold(mut self) -> RenderCell {
        self.style = merge_styles(self.style, RenderStyle::builder().bold().build());
        self
    }
    fn dim(mut self) -> RenderCell {
        self.style = merge_styles(self.style, RenderStyle::builder().dim().build());
        self
    }
    fn italic(mut self) -> RenderCell {
        self.style = merge_styles(self.style, RenderStyle::builder().italic().build());
        self
    }
    fn underlined(mut self) -> RenderCell {
        self.style = merge_styles(self.style, RenderStyle::builder().underline().build());
        self
    }
    fn crossed_out(mut self) -> RenderCell {
        self.style = merge_styles(self.style, RenderStyle::builder().strikethrough().build());
        self
    }
}

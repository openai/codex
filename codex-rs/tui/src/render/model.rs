#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStyle {
    pub fg: Option<RenderColor>,
    pub bg: Option<RenderColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderColor {
    Default,
    Red,
    Green,
    Magenta,
    Cyan,
    Gray,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderCell {
    pub text: String,
    pub style: RenderStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderLine {
    pub cells: Vec<RenderCell>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderParagraph {
    pub lines: Vec<RenderLine>,
    pub wrap: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStyleBuilder {
    fg: Option<RenderColor>,
    bg: Option<RenderColor>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
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
}

#[allow(dead_code)]
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
            text: text.into(),
            style,
        }
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
        Self { cells }
    }

    /// Returns a builder for constructing a render line.
    ///
    /// # Returns
    /// - `RenderLineBuilder`: Builder for render line configuration.
    pub fn builder() -> RenderLineBuilder {
        RenderLineBuilder::new()
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

    /// Builds the render line.
    ///
    /// # Returns
    /// - `RenderLine`: Fully constructed render line.
    pub fn build(self) -> RenderLine {
        RenderLine::new(self.cells)
    }
}

#[allow(dead_code)]
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

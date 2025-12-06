//! Elements related to the `Block` base widget.
//!
//! This holds everything needed to display and configure a [`Block`].
//!
//! In its simplest form, a `Block` is a [border](Borders) around another widget. It can have a
//! [title](Block::title) and [padding](Block::padding).

use alloc::vec::Vec;

use itertools::Itertools;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Rect};
use ratatui_core::style::{Style, Styled};
use ratatui_core::symbols::border;
use ratatui_core::symbols::merge::MergeStrategy;
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;
use strum::{Display, EnumString};

pub use self::padding::Padding;
use crate::borders::{BorderType, Borders};

mod padding;

/// A widget that renders borders, titles, and padding around other widgets.
///
/// A `Block` is a foundational widget that creates visual containers by drawing borders around an
/// area. It serves as a wrapper or frame for other widgets, providing structure and visual
/// separation in terminal UIs. Most built-in widgets in Ratatui use a pattern where they accept an
/// optional `Block` parameter that wraps the widget's content.
///
/// When a widget renders with a block, the widget's style is applied first, then the block's style,
/// and finally the widget's content is rendered within the inner area calculated by the block. This
/// layered approach allows for flexible styling where the block can provide background colors,
/// borders, and padding while the inner widget handles its own content styling.
///
/// Multiple blocks can be nested within each other. The [`Block::inner`] method calculates the area
/// available for content after accounting for borders, titles, and padding, making it easy to nest
/// blocks or position widgets within a block's boundaries.
///
/// # Constructor Methods
///
/// - [`Block::new`] - Creates a block with no borders or padding
/// - [`Block::bordered`] - Creates a block with all borders enabled
///
/// # Border Configuration
///
/// - [`Block::borders`] - Specifies which borders to display
/// - [`Block::border_style`] - Sets the style of the borders
/// - [`Block::border_type`] - Sets border symbols (single, double, thick, rounded, etc.)
/// - [`Block::border_set`] - Sets custom border symbols as a [`border::Set`]
/// - [`Block::merge_borders`] - Controls how borders merge with adjacent blocks
///
/// # Title Configuration
///
/// - [`Block::title`] - Adds a title to the block
/// - [`Block::title_top`] - Adds a title to the top of the block
/// - [`Block::title_bottom`] - Adds a title to the bottom of the block
/// - [`Block::title_alignment`] - Sets default alignment for all titles
/// - [`Block::title_style`] - Sets the style for all titles
/// - [`Block::title_position`] - Sets default position for titles
///
/// # Styling and Layout
///
/// - [`Block::style`] - Sets the base style of the block
/// - [`Block::padding`] - Adds internal padding within the borders
/// - [`Block::inner`] - Calculates the inner area available for content
///
/// # Title Behavior
///
/// You can add multiple titles to a block, and they will be rendered with spaces separating titles
/// that share the same position or alignment. When both centered and non-centered titles exist, the
/// centered space is calculated based on the full width of the block.
///
/// Titles are set using the `.title`, `.title_top`, and `.title_bottom` methods. These methods
/// accept a string or any type that can be converted into a [`Line`], such as a string slice,
/// `String`, or a vector of [`Span`]s. To control the alignment of a title (left, center, right),
/// pass a `Line` with the desired alignment, e.g. `Line::from("Title").centered()`.
///
/// By default, `.title` places the title at the top of the block, but you can use `.title_top` or
/// `.title_bottom` to explicitly set the position. The default alignment for all titles can be set
/// with [`Block::title_alignment`], and the default position for all titles can be set with
/// [`Block::title_position`].
///
/// Note that prior to `v0.30.0`, the `block::Title` struct was used to create titles. This struct
/// has been removed. The new recommended approach is to use [`Line`] with a specific alignment for
/// the title's content and the [`Block::title_top`] and [`Block::title_bottom`] methods for
/// positioning.
///
/// Titles avoid being rendered in corners when borders are present, but will align to edges when no
/// border exists on that side:
///
/// ```plain
/// ┌With at least a left border───
///
/// Without left border───
/// ```
///
/// # Nesting Widgets with `inner`
///
/// The [`Block::inner`] method computes the area inside the block after accounting for borders,
/// titles, and padding. This allows you to nest widgets inside a block by rendering the block
/// first, then rendering other widgets in the returned inner area.
///
/// For example, you can nest a block inside another block:
///
/// ```
/// use ratatui::Frame;
/// use ratatui::widgets::Block;
///
/// # fn render_nested_block(frame: &mut Frame) {
/// let outer_block = Block::bordered().title("Outer");
/// let inner_block = Block::bordered().title("Inner");
///
/// let outer_area = frame.area();
/// let inner_area = outer_block.inner(outer_area);
///
/// frame.render_widget(outer_block, outer_area);
/// frame.render_widget(inner_block, inner_area);
/// # }
/// ```
///
/// You can also use the standard [`Layout`] functionality to further subdivide the inner area and
/// lay out multiple widgets inside a block.
///
/// # Integration with Other Widgets
///
/// Most widgets in Ratatui accept a block parameter. For example, [`Paragraph`], [`List`],
/// [`Table`], and other widgets can be wrapped with a block:
///
/// ```
/// use ratatui::widgets::{Block, Paragraph};
///
/// let paragraph = Paragraph::new("Hello, world!").block(Block::bordered().title("My Paragraph"));
/// ```
///
/// This pattern allows widgets to focus on their content while blocks handle the visual framing.
///
/// # Styling
///
/// Styles are applied in a specific order: first the block's base style, then border styles, then
/// title styles, and finally any content widget styles. This layered approach allows for flexible
/// styling where outer styles provide defaults that inner styles can override.
///
/// `Block` implements [`Stylize`](ratatui_core::style::Stylize), allowing you to use style
/// shorthand methods:
///
/// ```
/// use ratatui::style::Stylize;
/// use ratatui::widgets::Block;
///
/// let block = Block::bordered().red().on_white().bold();
/// ```
///
/// # Examples
///
/// Create a simple bordered block:
///
/// ```
/// use ratatui::widgets::Block;
///
/// let block = Block::bordered().title("My Block");
/// ```
///
/// Create a block with custom border styling:
///
/// ```
/// use ratatui::style::{Color, Style, Stylize};
/// use ratatui::widgets::{Block, BorderType};
///
/// let block = Block::bordered()
///     .title("Styled Block")
///     .border_type(BorderType::Rounded)
///     .border_style(Style::new().cyan())
///     .style(Style::new().on_black());
/// ```
///
/// Use a block to wrap another widget:
///
/// ```
/// use ratatui::widgets::{Block, Paragraph};
///
/// let paragraph = Paragraph::new("Hello, world!").block(Block::bordered().title("Greeting"));
/// ```
///
/// Add multiple titles with different alignments:
///
/// ```
/// use ratatui::text::Line;
/// use ratatui::widgets::Block;
///
/// let block = Block::bordered()
///     .title_top(Line::from("Left").left_aligned())
///     .title_top(Line::from("Center").centered())
///     .title_top(Line::from("Right").right_aligned())
///     .title_bottom("Status: OK");
/// ```
///
/// # See Also
///
/// - [Block recipe] - Visual examples and common patterns (on the ratatui website)
/// - [Collapse borders recipe] - Techniques for creating seamless layouts (on the ratatui website)
/// - [`MergeStrategy`] - Controls how borders merge with adjacent elements
///
/// [Block recipe]: https://ratatui.rs/recipes/widgets/block/
/// [Collapse borders recipe]: https://ratatui.rs/recipes/layout/collapse-borders/
/// [`Paragraph`]: crate::paragraph::Paragraph
/// [`Span`]: ratatui_core::text::Span
/// [`Table`]: crate::table::Table
/// [`Stylize`]: ratatui_core::style::Stylize
/// [`List`]: crate::list::List
/// [`Layout`]: ratatui_core::layout::Layout
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Block<'a> {
    /// List of titles
    titles: Vec<(Option<TitlePosition>, Line<'a>)>,
    /// The style to be patched to all titles of the block
    titles_style: Style,
    /// The default alignment of the titles that don't have one
    titles_alignment: Alignment,
    /// The default position of the titles that don't have one
    titles_position: TitlePosition,
    /// Visible borders
    borders: Borders,
    /// Border style
    border_style: Style,
    /// The symbols used to render the border. The default is plain lines but one can choose to
    /// have rounded or doubled lines instead or a custom set of symbols
    border_set: border::Set<'a>,
    /// Widget style
    style: Style,
    /// Block padding
    padding: Padding,
    /// Border merging strategy
    merge_borders: MergeStrategy,
}

/// Defines the position of the title.
///
/// The title can be positioned on top or at the bottom of the block.
///
/// # Example
///
/// ```
/// use ratatui::widgets::{Block, TitlePosition};
///
/// Block::bordered()
///     .title_position(TitlePosition::Top)
///     .title("Top Title");
/// Block::bordered()
///     .title_position(TitlePosition::Bottom)
///     .title("Bottom Title");
/// ```
#[derive(Debug, Default, Display, EnumString, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitlePosition {
    /// Position the title at the top of the block.
    #[default]
    Top,
    /// Position the title at the bottom of the block.
    Bottom,
}

impl<'a> Block<'a> {
    /// Creates a new block with no [`Borders`] or [`Padding`].
    pub const fn new() -> Self {
        Self {
            titles: Vec::new(),
            titles_style: Style::new(),
            titles_alignment: Alignment::Left,
            titles_position: TitlePosition::Top,
            borders: Borders::NONE,
            border_style: Style::new(),
            border_set: BorderType::Plain.to_border_set(),
            style: Style::new(),
            padding: Padding::ZERO,
            merge_borders: MergeStrategy::Replace,
        }
    }

    /// Create a new block with [all borders](Borders::ALL) shown
    ///
    /// ```
    /// use ratatui::widgets::{Block, Borders};
    ///
    /// assert_eq!(Block::bordered(), Block::new().borders(Borders::ALL));
    /// ```
    pub const fn bordered() -> Self {
        let mut block = Self::new();
        block.borders = Borders::ALL;
        block
    }

    /// Adds a title to the block using the default position.
    ///
    /// The position of the title is determined by the `title_position` field of the block, which
    /// defaults to `Top`. This can be changed using the [`Block::title_position`] method. For
    /// explicit positioning, use [`Block::title_top`] or [`Block::title_bottom`].
    ///
    /// The `title` function allows you to add a title to the block. You can call this function
    /// multiple times to add multiple titles.
    ///
    /// Each title will be rendered with a single space separating titles that are in the same
    /// position or alignment. When both centered and non-centered titles are rendered, the centered
    /// space is calculated based on the full width of the block, rather than the leftover width.
    ///
    /// You can provide any type that can be converted into [`Line`] including: strings, string
    /// slices (`&str`), borrowed strings (`Cow<str>`), [spans](ratatui_core::text::Span), or
    /// vectors of [spans](ratatui_core::text::Span) (`Vec<Span>`).
    ///
    /// By default, the titles will avoid being rendered in the corners of the block but will align
    /// against the left or right edge of the block if there is no border on that edge. The
    /// following demonstrates this behavior, notice the second title is one character off to the
    /// left.
    ///
    /// ```plain
    /// ┌With at least a left border───
    ///
    /// Without left border───
    /// ```
    ///
    /// Note: If the block is too small and multiple titles overlap, the border might get cut off at
    /// a corner.
    ///
    /// # Examples
    ///
    /// See the [Block example] for a visual representation of how the various borders and styles
    /// look when rendered.
    ///
    /// The following example demonstrates:
    /// - Default title alignment
    /// - Multiple titles (notice "Center" is centered according to the full with of the block, not
    ///   the leftover space)
    /// - Two titles with the same alignment (notice the left titles are separated)
    /// ```
    /// use ratatui::text::Line;
    /// use ratatui::widgets::Block;
    ///
    /// Block::bordered()
    ///     .title("Title")
    ///     .title(Line::from("Left").left_aligned())
    ///     .title(Line::from("Right").right_aligned())
    ///     .title(Line::from("Center").centered());
    /// ```
    ///
    /// # See also
    ///
    /// Titles attached to a block can have default behaviors. See
    /// - [`Block::title_style`]
    /// - [`Block::title_alignment`]
    ///
    /// # History
    ///
    /// In previous releases of Ratatui this method accepted `Into<Title>` instead of
    /// [`Into<Line>`]. We found that storing the position in the block and the alignment in the
    /// line better reflects the intended use of the block and its titles. See
    /// <https://github.com/ratatui/ratatui/issues/738> for more information.
    ///
    /// [Block example]: https://github.com/ratatui/ratatui/blob/main/examples/README.md#block
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn title<T>(mut self, title: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.titles.push((None, title.into()));
        self
    }

    /// Adds a title to the top of the block.
    ///
    /// You can provide any type that can be converted into [`Line`] including: strings, string
    /// slices (`&str`), borrowed strings (`Cow<str>`), [spans](ratatui_core::text::Span), or
    /// vectors of [spans](ratatui_core::text::Span) (`Vec<Span>`).
    ///
    /// # Example
    ///
    /// ```
    /// use ratatui::{ widgets::Block, text::Line };
    ///
    /// Block::bordered()
    ///     .title_top("Left1") // By default in the top left corner
    ///     .title_top(Line::from("Left2").left_aligned())
    ///     .title_top(Line::from("Right").right_aligned())
    ///     .title_top(Line::from("Center").centered());
    ///
    /// // Renders
    /// // ┌Left1─Left2───Center─────────Right┐
    /// // │                                  │
    /// // └──────────────────────────────────┘
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn title_top<T: Into<Line<'a>>>(mut self, title: T) -> Self {
        let line = title.into();
        self.titles.push((Some(TitlePosition::Top), line));
        self
    }

    /// Adds a title to the bottom of the block.
    ///
    /// You can provide any type that can be converted into [`Line`] including: strings, string
    /// slices (`&str`), borrowed strings (`Cow<str>`), [spans](ratatui_core::text::Span), or
    /// vectors of [spans](ratatui_core::text::Span) (`Vec<Span>`).
    ///
    /// # Example
    ///
    /// ```
    /// use ratatui::{ widgets::Block, text::Line };
    ///
    /// Block::bordered()
    ///     .title_bottom("Left1") // By default in the top left corner
    ///     .title_bottom(Line::from("Left2").left_aligned())
    ///     .title_bottom(Line::from("Right").right_aligned())
    ///     .title_bottom(Line::from("Center").centered());
    ///
    /// // Renders
    /// // ┌──────────────────────────────────┐
    /// // │                                  │
    /// // └Left1─Left2───Center─────────Right┘
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn title_bottom<T: Into<Line<'a>>>(mut self, title: T) -> Self {
        let line = title.into();
        self.titles.push((Some(TitlePosition::Bottom), line));
        self
    }

    /// Applies the style to all titles.
    ///
    /// This style will be applied to all titles of the block. If a title has a style set, it will
    /// be applied after this style. This style will be applied after any [`Block::style`] or
    /// [`Block::border_style`] is applied.
    ///
    /// See [`Style`] for more information on how merging styles works.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn title_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.titles_style = style.into();
        self
    }

    /// Sets the default [`Alignment`] for all block titles.
    ///
    /// Titles that explicitly set an [`Alignment`] will ignore this.
    ///
    /// # Example
    ///
    /// This example aligns all titles in the center except the "right" title which explicitly sets
    /// [`Alignment::Right`].
    /// ```
    /// use ratatui::layout::Alignment;
    /// use ratatui::text::Line;
    /// use ratatui::widgets::Block;
    ///
    /// Block::bordered()
    ///     .title_alignment(Alignment::Center)
    ///     // This title won't be aligned in the center
    ///     .title(Line::from("right").right_aligned())
    ///     .title("foo")
    ///     .title("bar");
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn title_alignment(mut self, alignment: Alignment) -> Self {
        self.titles_alignment = alignment;
        self
    }

    /// Sets the default [`TitlePosition`] for all block titles.
    ///
    /// # Example
    ///
    /// This example positions all titles on the bottom by default. The "top" title explicitly sets
    /// its position to `Top`, so it is not affected. The "foo" and "bar" titles will be positioned
    /// at the bottom.
    ///
    /// ```
    /// use ratatui::widgets::{Block, TitlePosition};
    ///
    /// Block::bordered()
    ///     .title_position(TitlePosition::Bottom)
    ///     .title("foo") // will be at the bottom
    ///     .title_top("top") // will be at the top
    ///     .title("bar"); // will be at the bottom
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn title_position(mut self, position: TitlePosition) -> Self {
        self.titles_position = position;
        self
    }

    /// Defines the style of the borders.
    ///
    /// This style is applied only to the areas covered by borders, and is applied to the block
    /// after any [`Block::style`] is applied.
    ///
    /// See [`Style`] for more information on how merging styles works.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// # Example
    ///
    /// This example shows a `Block` with blue borders.
    /// ```
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::widgets::Block;
    /// Block::bordered().border_style(Style::new().blue());
    /// ```
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn border_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.border_style = style.into();
        self
    }

    /// Defines the style of the entire block.
    ///
    /// This is the most generic [`Style`] a block can receive, it will be merged with any other
    /// more specific styles. Elements can be styled further with [`Block::title_style`] and
    /// [`Block::border_style`], which will be applied on top of this style. If the block is used as
    /// a container for another widget (e.g. a [`Paragraph`]), then the style of the widget is
    /// generally applied before this style.
    ///
    /// See [`Style`] for more information on how merging styles works.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// # Example
    ///
    /// ```
    /// use ratatui::style::{Color, Style, Stylize};
    /// use ratatui::widgets::{Block, Paragraph};
    ///
    /// let block = Block::new().style(Style::new().red().on_black());
    ///
    /// // For border and title you can additionally apply styles on top of the block level style.
    /// let block = Block::new()
    ///     .style(Style::new().red().bold().italic())
    ///     .border_style(Style::new().not_italic()) // will be red and bold
    ///     .title_style(Style::new().not_bold()) // will be red and italic
    ///     .title("Title");
    ///
    /// // To style the inner widget, you can style the widget itself.
    /// let paragraph = Paragraph::new("Content")
    ///     .block(block)
    ///     .style(Style::new().white().not_bold()); // will be white, and italic
    /// ```
    ///
    /// [`Paragraph`]: crate::paragraph::Paragraph
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Defines which borders to display.
    ///
    /// [`Borders`] can also be styled with [`Block::border_style`] and [`Block::border_type`].
    ///
    /// # Examples
    ///
    /// Display left and right borders.
    /// ```
    /// use ratatui::widgets::{Block, Borders};
    /// Block::new().borders(Borders::LEFT | Borders::RIGHT);
    /// ```
    ///
    /// To show all borders you can abbreviate this with [`Block::bordered`]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn borders(mut self, flag: Borders) -> Self {
        self.borders = flag;
        self
    }

    /// Sets the symbols used to display the border (e.g. single line, double line, thick or
    /// rounded borders).
    ///
    /// Setting this overwrites any custom [`border_set`](Block::border_set) that was set.
    ///
    /// See [`BorderType`] for the full list of available symbols.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::widgets::{Block, BorderType};
    /// Block::bordered()
    ///     .border_type(BorderType::Rounded)
    ///     .title("Block");
    /// // Renders
    /// // ╭Block╮
    /// // │     │
    /// // ╰─────╯
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn border_type(mut self, border_type: BorderType) -> Self {
        self.border_set = border_type.to_border_set();
        self
    }

    /// Sets the symbols used to display the border as a [`ratatui_core::symbols::border::Set`].
    ///
    /// Setting this overwrites any [`border_type`](Block::border_type) that was set.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::{widgets::Block, symbols};
    ///
    /// Block::bordered().border_set(symbols::border::DOUBLE).title("Block");
    /// // Renders
    /// // ╔Block╗
    /// // ║     ║
    /// // ╚═════╝
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn border_set(mut self, border_set: border::Set<'a>) -> Self {
        self.border_set = border_set;
        self
    }

    /// Defines the padding inside a `Block`.
    ///
    /// See [`Padding`] for more information.
    ///
    /// # Examples
    ///
    /// This renders a `Block` with no padding (the default).
    /// ```
    /// use ratatui::widgets::{Block, Padding};
    ///
    /// Block::bordered().padding(Padding::ZERO);
    /// // Renders
    /// // ┌───────┐
    /// // │content│
    /// // └───────┘
    /// ```
    ///
    /// This example shows a `Block` with padding left and right ([`Padding::horizontal`]).
    /// Notice the two spaces before and after the content.
    /// ```
    /// use ratatui::widgets::{Block, Padding};
    ///
    /// Block::bordered().padding(Padding::horizontal(2));
    /// // Renders
    /// // ┌───────────┐
    /// // │  content  │
    /// // └───────────┘
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    /// Sets the block's [`MergeStrategy`] for overlapping characters, defaulting to [`Replace`].
    ///
    /// Changing the strategy to [`Exact`] or [`Fuzzy`] collapses border characters that intersect
    /// with any previously rendered borders.
    ///
    /// For more information and examples, see the [collapse borders recipe] and [`MergeStrategy`]
    /// docs.
    ///
    /// # Example
    ///
    /// ```
    /// use ratatui::symbols::merge::MergeStrategy;
    /// # use ratatui::widgets::{Block, BorderType};
    ///
    /// // Given several blocks with plain borders (1)
    /// Block::bordered();
    /// // and other blocks with thick borders (2) which are rendered on top of the first
    /// Block::bordered()
    ///     .border_type(BorderType::Thick)
    ///     .merge_borders(MergeStrategy::Exact);
    /// ```
    ///
    /// Rendering these blocks with `MergeStrategy::Exact` or `MergeStrategy::Fuzzy` will collapse
    /// the borders, resulting in a clean layout without connected borders.
    ///
    /// ```plain
    /// ┌───┐    ┌───┐  ┌───┲━━━┓┌───┐
    /// │   │    │ 1 │  │   ┃   ┃│   │
    /// │ 1 │    │ ┏━┿━┓│ 1 ┃ 2 ┃│ 1 │
    /// │   │    │ ┃ │ ┃│   ┃   ┃│   │
    /// └───╆━━━┓└─╂─┘ ┃└───┺━━━┛┢━━━┪
    ///     ┃   ┃  ┃ 2 ┃         ┃   ┃
    ///     ┃ 2 ┃  ┗━━━┛         ┃ 2 ┃
    ///     ┃   ┃                ┃   ┃
    ///     ┗━━━┛                ┗━━━┛
    /// ```
    ///
    /// [collapse borders recipe]: https://ratatui.rs/recipes/layout/collapse-borders/
    /// [`Replace`]: MergeStrategy::Replace
    /// [`Exact`]: MergeStrategy::Exact
    /// [`Fuzzy`]: MergeStrategy::Fuzzy
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn merge_borders(mut self, strategy: MergeStrategy) -> Self {
        self.merge_borders = strategy;
        self
    }

    /// Computes the inner area of a block after subtracting space for borders, titles, and padding.
    ///
    /// # Examples
    ///
    /// Draw a block nested within another block
    /// ```
    /// use ratatui::Frame;
    /// use ratatui::widgets::Block;
    ///
    /// # fn render_nested_block(frame: &mut Frame) {
    /// let outer_block = Block::bordered().title("Outer");
    /// let inner_block = Block::bordered().title("Inner");
    ///
    /// let outer_area = frame.area();
    /// let inner_area = outer_block.inner(outer_area);
    ///
    /// frame.render_widget(outer_block, outer_area);
    /// frame.render_widget(inner_block, inner_area);
    /// # }
    /// // Renders
    /// // ┌Outer────────┐
    /// // │┌Inner──────┐│
    /// // ││           ││
    /// // │└───────────┘│
    /// // └─────────────┘
    /// ```
    pub fn inner(&self, area: Rect) -> Rect {
        let mut inner = area;
        if self.borders.intersects(Borders::LEFT) {
            inner.x = inner.x.saturating_add(1).min(inner.right());
            inner.width = inner.width.saturating_sub(1);
        }
        if self.borders.intersects(Borders::TOP) || self.has_title_at_position(TitlePosition::Top) {
            inner.y = inner.y.saturating_add(1).min(inner.bottom());
            inner.height = inner.height.saturating_sub(1);
        }
        if self.borders.intersects(Borders::RIGHT) {
            inner.width = inner.width.saturating_sub(1);
        }
        if self.borders.intersects(Borders::BOTTOM)
            || self.has_title_at_position(TitlePosition::Bottom)
        {
            inner.height = inner.height.saturating_sub(1);
        }

        inner.x = inner.x.saturating_add(self.padding.left);
        inner.y = inner.y.saturating_add(self.padding.top);

        inner.width = inner
            .width
            .saturating_sub(self.padding.left + self.padding.right);
        inner.height = inner
            .height
            .saturating_sub(self.padding.top + self.padding.bottom);

        inner
    }

    fn has_title_at_position(&self, position: TitlePosition) -> bool {
        self.titles
            .iter()
            .any(|(pos, _)| pos.unwrap_or(self.titles_position) == position)
    }
}

impl Widget for Block<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Block<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        buf.set_style(area, self.style);
        self.render_borders(area, buf);
        self.render_titles(area, buf);
    }
}

impl Block<'_> {
    fn render_borders(&self, area: Rect, buf: &mut Buffer) {
        self.render_sides(area, buf);
        self.render_corners(area, buf);
    }

    fn render_sides(&self, area: Rect, buf: &mut Buffer) {
        let left = area.left();
        let top = area.top();
        // area.right() and area.bottom() are outside the rect, subtract 1 to get the last row/col
        let right = area.right() - 1;
        let bottom = area.bottom() - 1;

        // The first and last element of each line are not drawn when there is an adjacent line as
        // this would cause the corner to initially be merged with a side character and then a
        // corner character to be drawn on top of it. Some merge strategies would not produce a
        // correct character in that case.
        let is_replace = self.merge_borders != MergeStrategy::Replace;
        let left_inset = left + u16::from(is_replace && self.borders.contains(Borders::LEFT));
        let top_inset = top + u16::from(is_replace && self.borders.contains(Borders::TOP));
        let right_inset = right - u16::from(is_replace && self.borders.contains(Borders::RIGHT));
        let bottom_inset = bottom - u16::from(is_replace && self.borders.contains(Borders::BOTTOM));

        let sides = [
            (
                Borders::LEFT,
                left..=left,
                top_inset..=bottom_inset,
                self.border_set.vertical_left,
            ),
            (
                Borders::TOP,
                left_inset..=right_inset,
                top..=top,
                self.border_set.horizontal_top,
            ),
            (
                Borders::RIGHT,
                right..=right,
                top_inset..=bottom_inset,
                self.border_set.vertical_right,
            ),
            (
                Borders::BOTTOM,
                left_inset..=right_inset,
                bottom..=bottom,
                self.border_set.horizontal_bottom,
            ),
        ];
        for (border, x_range, y_range, symbol) in sides {
            if self.borders.contains(border) {
                for x in x_range {
                    for y in y_range.clone() {
                        buf[(x, y)]
                            .merge_symbol(symbol, self.merge_borders)
                            .set_style(self.border_style);
                    }
                }
            }
        }
    }

    fn render_corners(&self, area: Rect, buf: &mut Buffer) {
        let corners = [
            (
                Borders::RIGHT | Borders::BOTTOM,
                area.right() - 1,
                area.bottom() - 1,
                self.border_set.bottom_right,
            ),
            (
                Borders::RIGHT | Borders::TOP,
                area.right() - 1,
                area.top(),
                self.border_set.top_right,
            ),
            (
                Borders::LEFT | Borders::BOTTOM,
                area.left(),
                area.bottom() - 1,
                self.border_set.bottom_left,
            ),
            (
                Borders::LEFT | Borders::TOP,
                area.left(),
                area.top(),
                self.border_set.top_left,
            ),
        ];

        for (border, x, y, symbol) in corners {
            if self.borders.contains(border) {
                buf[(x, y)]
                    .merge_symbol(symbol, self.merge_borders)
                    .set_style(self.border_style);
            }
        }
    }
    fn render_titles(&self, area: Rect, buf: &mut Buffer) {
        self.render_title_position(TitlePosition::Top, area, buf);
        self.render_title_position(TitlePosition::Bottom, area, buf);
    }

    fn render_title_position(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
        // NOTE: the order in which these functions are called defines the overlapping behavior
        self.render_left_titles(position, area, buf);
        self.render_center_titles(position, area, buf);
        self.render_right_titles(position, area, buf);
    }

    /// Render titles aligned to the right of the block
    ///
    /// Currently (due to the way lines are truncated), the right side of the leftmost title will
    /// be cut off if the block is too small to fit all titles. This is not ideal and should be
    /// the left side of that leftmost that is cut off. This is due to the line being truncated
    /// incorrectly. See <https://github.com/ratatui/ratatui/issues/932>
    #[expect(clippy::similar_names)]
    fn render_right_titles(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
        let titles = self.filtered_titles(position, Alignment::Right);
        let mut titles_area = self.titles_area(area, position);

        // render titles in reverse order to align them to the right
        for title in titles.rev() {
            if titles_area.is_empty() {
                break;
            }
            let title_width = title.width() as u16;
            let title_area = Rect {
                x: titles_area
                    .right()
                    .saturating_sub(title_width)
                    .max(titles_area.left()),
                width: title_width.min(titles_area.width),
                ..titles_area
            };
            buf.set_style(title_area, self.titles_style);
            title.render(title_area, buf);

            // bump the width of the titles area to the left
            titles_area.width = titles_area
                .width
                .saturating_sub(title_width)
                .saturating_sub(1); // space between titles
        }
    }

    /// Render titles in the center of the block
    fn render_center_titles(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
        let area = self.titles_area(area, position);
        let titles = self
            .filtered_titles(position, Alignment::Center)
            .collect_vec();
        // titles are rendered with a space after each title except the last one
        let total_width = titles
            .iter()
            .map(|title| title.width() as u16 + 1)
            .sum::<u16>()
            .saturating_sub(1);

        if total_width <= area.width {
            self.render_centered_titles_without_truncation(titles, total_width, area, buf);
        } else {
            self.render_centered_titles_with_truncation(titles, total_width, area, buf);
        }
    }

    fn render_centered_titles_without_truncation(
        &self,
        titles: Vec<&Line<'_>>,
        total_width: u16,
        area: Rect,
        buf: &mut Buffer,
    ) {
        // titles fit in the area, center them
        let x = area.left() + area.width.saturating_sub(total_width) / 2;
        let mut area = Rect { x, ..area };
        for title in titles {
            let width = title.width() as u16;
            let title_area = Rect { width, ..area };
            buf.set_style(title_area, self.titles_style);
            title.render(title_area, buf);
            // Move the rendering cursor to the right, leaving 1 column space.
            area.x = area.x.saturating_add(width + 1);
            area.width = area.width.saturating_sub(width + 1);
        }
    }

    fn render_centered_titles_with_truncation(
        &self,
        titles: Vec<&Line<'_>>,
        total_width: u16,
        mut area: Rect,
        buf: &mut Buffer,
    ) {
        // titles do not fit in the area, truncate the left side using an offset. The right side
        // is truncated by the area width.
        let mut offset = total_width.saturating_sub(area.width) / 2;
        for title in titles {
            if area.is_empty() {
                break;
            }
            let width = area.width.min(title.width() as u16).saturating_sub(offset);
            let title_area = Rect { width, ..area };
            buf.set_style(title_area, self.titles_style);
            if offset > 0 {
                // truncate the left side of the title to fit the area
                title.clone().right_aligned().render(title_area, buf);
                offset = offset.saturating_sub(width).saturating_sub(1);
            } else {
                // truncate the right side of the title to fit the area if needed
                title.clone().left_aligned().render(title_area, buf);
            }
            // Leave 1 column of spacing between titles.
            area.x = area.x.saturating_add(width + 1);
            area.width = area.width.saturating_sub(width + 1);
        }
    }

    /// Render titles aligned to the left of the block
    #[expect(clippy::similar_names)]
    fn render_left_titles(&self, position: TitlePosition, area: Rect, buf: &mut Buffer) {
        let titles = self.filtered_titles(position, Alignment::Left);
        let mut titles_area = self.titles_area(area, position);
        for title in titles {
            if titles_area.is_empty() {
                break;
            }
            let title_width = title.width() as u16;
            let title_area = Rect {
                width: title_width.min(titles_area.width),
                ..titles_area
            };
            buf.set_style(title_area, self.titles_style);
            title.render(title_area, buf);

            // bump the titles area to the right and reduce its width
            titles_area.x = titles_area.x.saturating_add(title_width + 1);
            titles_area.width = titles_area.width.saturating_sub(title_width + 1);
        }
    }

    /// An iterator over the titles that match the position and alignment
    fn filtered_titles(
        &self,
        position: TitlePosition,
        alignment: Alignment,
    ) -> impl DoubleEndedIterator<Item = &Line<'_>> {
        self.titles
            .iter()
            .filter(move |(pos, _)| pos.unwrap_or(self.titles_position) == position)
            .filter(move |(_, line)| line.alignment.unwrap_or(self.titles_alignment) == alignment)
            .map(|(_, line)| line)
    }

    /// An area that is one line tall and spans the width of the block excluding the borders and
    /// is positioned at the top or bottom of the block.
    fn titles_area(&self, area: Rect, position: TitlePosition) -> Rect {
        let left_border = u16::from(self.borders.contains(Borders::LEFT));
        let right_border = u16::from(self.borders.contains(Borders::RIGHT));
        Rect {
            x: area.left() + left_border,
            y: match position {
                TitlePosition::Top => area.top(),
                TitlePosition::Bottom => area.bottom() - 1,
            },
            width: area
                .width
                .saturating_sub(left_border)
                .saturating_sub(right_border),
            height: 1,
        }
    }

    /// Calculate the left, and right space the [`Block`] will take up.
    ///
    /// The result takes the [`Block`]'s, [`Borders`], and [`Padding`] into account.
    pub(crate) fn horizontal_space(&self) -> (u16, u16) {
        let left = self
            .padding
            .left
            .saturating_add(u16::from(self.borders.contains(Borders::LEFT)));
        let right = self
            .padding
            .right
            .saturating_add(u16::from(self.borders.contains(Borders::RIGHT)));
        (left, right)
    }

    /// Calculate the top, and bottom space that the [`Block`] will take up.
    ///
    /// Takes the [`Padding`], [`TitlePosition`], and the [`Borders`] that are selected into
    /// account when calculating the result.
    pub(crate) fn vertical_space(&self) -> (u16, u16) {
        let has_top =
            self.borders.contains(Borders::TOP) || self.has_title_at_position(TitlePosition::Top);
        let top = self.padding.top + u16::from(has_top);
        let has_bottom = self.borders.contains(Borders::BOTTOM)
            || self.has_title_at_position(TitlePosition::Bottom);
        let bottom = self.padding.bottom + u16::from(has_bottom);
        (top, bottom)
    }
}

/// An extension trait for [`Block`] that provides some convenience methods.
///
/// This is implemented for [`Option<Block>`](Option) to simplify the common case of having a
/// widget with an optional block.
pub trait BlockExt {
    /// Return the inner area of the block if it is `Some`. Otherwise, returns `area`.
    ///
    /// This is a useful convenience method for widgets that have an `Option<Block>` field
    fn inner_if_some(&self, area: Rect) -> Rect;
}

impl BlockExt for Option<Block<'_>> {
    fn inner_if_some(&self, area: Rect) -> Rect {
        self.as_ref().map_or(area, |block| block.inner(area))
    }
}

impl Styled for Block<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, vec};

    use itertools::iproduct;
    use ratatui_core::layout::Offset;
    use ratatui_core::style::{Color, Modifier, Stylize};
    use rstest::rstest;
    use strum::ParseError;

    use super::*;

    #[test]
    fn create_with_all_borders() {
        let block = Block::bordered();
        assert_eq!(block.borders, Borders::all());
    }

    #[rstest]
    #[case::none_0(Borders::NONE, Rect::ZERO, Rect::ZERO)]
    #[case::none_1(Borders::NONE, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, 1))]
    #[case::left_0(Borders::LEFT, Rect::ZERO, Rect::ZERO)]
    #[case::left_w1(Borders::LEFT, Rect::new(0, 0, 0, 1), Rect::new(0, 0, 0, 1))]
    #[case::left_w2(Borders::LEFT, Rect::new(0, 0, 1, 1), Rect::new(1, 0, 0, 1))]
    #[case::left_w3(Borders::LEFT, Rect::new(0, 0, 2, 1), Rect::new(1, 0, 1, 1))]
    #[case::top_0(Borders::TOP, Rect::ZERO, Rect::ZERO)]
    #[case::top_h1(Borders::TOP, Rect::new(0, 0, 1, 0), Rect::new(0, 0, 1, 0))]
    #[case::top_h2(Borders::TOP, Rect::new(0, 0, 1, 1), Rect::new(0, 1, 1, 0))]
    #[case::top_h3(Borders::TOP, Rect::new(0, 0, 1, 2), Rect::new(0, 1, 1, 1))]
    #[case::right_0(Borders::RIGHT, Rect::ZERO, Rect::ZERO)]
    #[case::right_w1(Borders::RIGHT, Rect::new(0, 0, 0, 1), Rect::new(0, 0, 0, 1))]
    #[case::right_w2(Borders::RIGHT, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 0, 1))]
    #[case::right_w3(Borders::RIGHT, Rect::new(0, 0, 2, 1), Rect::new(0, 0, 1, 1))]
    #[case::bottom_0(Borders::BOTTOM, Rect::ZERO, Rect::ZERO)]
    #[case::bottom_h1(Borders::BOTTOM, Rect::new(0, 0, 1, 0), Rect::new(0, 0, 1, 0))]
    #[case::bottom_h2(Borders::BOTTOM, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, 0))]
    #[case::bottom_h3(Borders::BOTTOM, Rect::new(0, 0, 1, 2), Rect::new(0, 0, 1, 1))]
    #[case::all_0(Borders::ALL, Rect::ZERO, Rect::ZERO)]
    #[case::all_1(Borders::ALL, Rect::new(0, 0, 1, 1), Rect::new(1, 1, 0, 0))]
    #[case::all_2(Borders::ALL, Rect::new(0, 0, 2, 2), Rect::new(1, 1, 0, 0))]
    #[case::all_3(Borders::ALL, Rect::new(0, 0, 3, 3), Rect::new(1, 1, 1, 1))]
    fn inner_takes_into_account_the_borders(
        #[case] borders: Borders,
        #[case] area: Rect,
        #[case] expected: Rect,
    ) {
        let block = Block::new().borders(borders);
        assert_eq!(block.inner(area), expected);
    }

    #[rstest]
    #[case::left(Alignment::Left)]
    #[case::center(Alignment::Center)]
    #[case::right(Alignment::Right)]
    fn inner_takes_into_account_the_title(#[case] alignment: Alignment) {
        let area = Rect::new(0, 0, 0, 1);
        let expected = Rect::new(0, 1, 0, 0);

        let block = Block::new().title(Line::from("Test").alignment(alignment));
        assert_eq!(block.inner(area), expected);
    }

    #[rstest]
    #[case::top_top(Block::new().title_top("Test").borders(Borders::TOP), Rect::new(0, 1, 0, 1))]
    #[case::top_bot(Block::new().title_top("Test").borders(Borders::BOTTOM), Rect::new(0, 1, 0, 0))]
    #[case::bot_top(Block::new().title_bottom("Test").borders(Borders::TOP), Rect::new(0, 1, 0, 0))]
    #[case::bot_bot(Block::new().title_bottom("Test").borders(Borders::BOTTOM), Rect::new(0, 0, 0, 1))]
    fn inner_takes_into_account_border_and_title(#[case] block: Block, #[case] expected: Rect) {
        let area = Rect::new(0, 0, 0, 2);
        assert_eq!(block.inner(area), expected);
    }

    #[test]
    fn has_title_at_position_takes_into_account_all_positioning_declarations() {
        let block = Block::new();
        assert!(!block.has_title_at_position(TitlePosition::Top));
        assert!(!block.has_title_at_position(TitlePosition::Bottom));

        let block = Block::new().title_top("test");
        assert!(block.has_title_at_position(TitlePosition::Top));
        assert!(!block.has_title_at_position(TitlePosition::Bottom));

        let block = Block::new().title_bottom("test");
        assert!(!block.has_title_at_position(TitlePosition::Top));
        assert!(block.has_title_at_position(TitlePosition::Bottom));

        let block = Block::new().title_top("test").title_bottom("test");
        assert!(block.has_title_at_position(TitlePosition::Top));
        assert!(block.has_title_at_position(TitlePosition::Bottom));
    }

    #[rstest]
    #[case::none(Borders::NONE, (0, 0))]
    #[case::top(Borders::TOP, (1, 0))]
    #[case::right(Borders::RIGHT, (0, 0))]
    #[case::bottom(Borders::BOTTOM, (0, 1))]
    #[case::left(Borders::LEFT, (0, 0))]
    #[case::top_right(Borders::TOP | Borders::RIGHT, (1, 0))]
    #[case::top_bottom(Borders::TOP | Borders::BOTTOM, (1, 1))]
    #[case::top_left(Borders::TOP | Borders::LEFT, (1, 0))]
    #[case::bottom_right(Borders::BOTTOM | Borders::RIGHT, (0, 1))]
    #[case::bottom_left(Borders::BOTTOM | Borders::LEFT, (0, 1))]
    #[case::left_right(Borders::LEFT | Borders::RIGHT, (0, 0))]
    fn vertical_space_takes_into_account_borders(
        #[case] borders: Borders,
        #[case] vertical_space: (u16, u16),
    ) {
        let block = Block::new().borders(borders);
        assert_eq!(block.vertical_space(), vertical_space);
    }

    #[rstest]
    #[case::top_border_top_p1(Borders::TOP, Padding::new(0, 0, 1, 0), (2, 0))]
    #[case::right_border_top_p1(Borders::RIGHT, Padding::new(0, 0, 1, 0), (1, 0))]
    #[case::bottom_border_top_p1(Borders::BOTTOM, Padding::new(0, 0, 1, 0), (1, 1))]
    #[case::left_border_top_p1(Borders::LEFT, Padding::new(0, 0, 1, 0), (1, 0))]
    #[case::top_bottom_border_all_p3(Borders::TOP | Borders::BOTTOM, Padding::new(100, 100, 4, 5), (5, 6))]
    #[case::no_border(Borders::NONE, Padding::new(100, 100, 10, 13), (10, 13))]
    #[case::all(Borders::ALL, Padding::new(100, 100, 1, 3), (2, 4))]
    fn vertical_space_takes_into_account_padding(
        #[case] borders: Borders,
        #[case] padding: Padding,
        #[case] vertical_space: (u16, u16),
    ) {
        let block = Block::new().borders(borders).padding(padding);
        assert_eq!(block.vertical_space(), vertical_space);
    }

    #[test]
    fn vertical_space_takes_into_account_titles() {
        let block = Block::new().title_top("Test");
        assert_eq!(block.vertical_space(), (1, 0));

        let block = Block::new().title_bottom("Test");
        assert_eq!(block.vertical_space(), (0, 1));
    }

    #[rstest]
    #[case::top_border_top_title(Block::new(), Borders::TOP, TitlePosition::Top, (1, 0))]
    #[case::right_border_top_title(Block::new(), Borders::RIGHT, TitlePosition::Top, (1, 0))]
    #[case::bottom_border_top_title(Block::new(), Borders::BOTTOM, TitlePosition::Top, (1, 1))]
    #[case::left_border_top_title(Block::new(), Borders::LEFT, TitlePosition::Top, (1, 0))]
    #[case::top_border_top_title(Block::new(), Borders::TOP, TitlePosition::Bottom, (1, 1))]
    #[case::right_border_top_title(Block::new(), Borders::RIGHT, TitlePosition::Bottom, (0, 1))]
    #[case::bottom_border_top_title(Block::new(), Borders::BOTTOM, TitlePosition::Bottom, (0, 1))]
    #[case::left_border_top_title(Block::new(), Borders::LEFT, TitlePosition::Bottom, (0, 1))]
    fn vertical_space_takes_into_account_borders_and_title(
        #[case] block: Block,
        #[case] borders: Borders,
        #[case] pos: TitlePosition,
        #[case] vertical_space: (u16, u16),
    ) {
        let block = block.borders(borders).title_position(pos).title("Test");
        assert_eq!(block.vertical_space(), vertical_space);
    }

    #[test]
    fn horizontal_space_takes_into_account_borders() {
        let block = Block::bordered();
        assert_eq!(block.horizontal_space(), (1, 1));

        let block = Block::new().borders(Borders::LEFT);
        assert_eq!(block.horizontal_space(), (1, 0));

        let block = Block::new().borders(Borders::RIGHT);
        assert_eq!(block.horizontal_space(), (0, 1));
    }

    #[test]
    fn horizontal_space_takes_into_account_padding() {
        let block = Block::new().padding(Padding::new(1, 1, 100, 100));
        assert_eq!(block.horizontal_space(), (1, 1));

        let block = Block::new().padding(Padding::new(3, 5, 0, 0));
        assert_eq!(block.horizontal_space(), (3, 5));

        let block = Block::new().padding(Padding::new(0, 1, 100, 100));
        assert_eq!(block.horizontal_space(), (0, 1));

        let block = Block::new().padding(Padding::new(1, 0, 100, 100));
        assert_eq!(block.horizontal_space(), (1, 0));
    }

    #[rstest]
    #[case::all_bordered_all_padded(Block::bordered(), Padding::new(1, 1, 1, 1), (2, 2))]
    #[case::all_bordered_left_padded(Block::bordered(), Padding::new(1, 0, 0, 0), (2, 1))]
    #[case::all_bordered_right_padded(Block::bordered(), Padding::new(0, 1, 0, 0), (1, 2))]
    #[case::all_bordered_top_padded(Block::bordered(), Padding::new(0, 0, 1, 0), (1, 1))]
    #[case::all_bordered_bottom_padded(Block::bordered(), Padding::new(0, 0, 0, 1), (1, 1))]
    #[case::left_bordered_left_padded(Block::new().borders(Borders::LEFT), Padding::new(1, 0, 0, 0), (2, 0))]
    #[case::left_bordered_right_padded(Block::new().borders(Borders::LEFT), Padding::new(0, 1, 0, 0), (1, 1))]
    #[case::right_bordered_right_padded(Block::new().borders(Borders::RIGHT), Padding::new(0, 1, 0, 0), (0, 2))]
    #[case::right_bordered_left_padded(Block::new().borders(Borders::RIGHT), Padding::new(1, 0, 0, 0), (1, 1))]
    fn horizontal_space_takes_into_account_borders_and_padding(
        #[case] block: Block,
        #[case] padding: Padding,
        #[case] horizontal_space: (u16, u16),
    ) {
        let block = block.padding(padding);
        assert_eq!(block.horizontal_space(), horizontal_space);
    }

    #[test]
    const fn border_type_can_be_const() {
        const _PLAIN: border::Set = BorderType::border_symbols(BorderType::Plain);
    }

    #[test]
    fn block_new() {
        assert_eq!(
            Block::new(),
            Block {
                titles: Vec::new(),
                titles_style: Style::new(),
                titles_alignment: Alignment::Left,
                titles_position: TitlePosition::Top,
                borders: Borders::NONE,
                border_style: Style::new(),
                border_set: BorderType::Plain.to_border_set(),
                style: Style::new(),
                padding: Padding::ZERO,
                merge_borders: MergeStrategy::Replace,
            }
        );
    }

    #[test]
    const fn block_can_be_const() {
        const _DEFAULT_STYLE: Style = Style::new();
        const _DEFAULT_PADDING: Padding = Padding::uniform(1);
        const _DEFAULT_BLOCK: Block = Block::bordered()
            // the following methods are no longer const because they use Into<Style>
            // .style(_DEFAULT_STYLE)           // no longer const
            // .border_style(_DEFAULT_STYLE)    // no longer const
            // .title_style(_DEFAULT_STYLE)     // no longer const
            .title_alignment(Alignment::Left)
            .title_position(TitlePosition::Top)
            .padding(_DEFAULT_PADDING);
    }

    /// Ensure Style from/into works the way a user would use it.
    #[test]
    fn style_into_works_from_user_view() {
        // nominal style
        let block = Block::new().style(Style::new().red());
        assert_eq!(block.style, Style::new().red());

        // auto-convert from Color
        let block = Block::new().style(Color::Red);
        assert_eq!(block.style, Style::new().red());

        // auto-convert from (Color, Color)
        let block = Block::new().style((Color::Red, Color::Blue));
        assert_eq!(block.style, Style::new().red().on_blue());

        // auto-convert from Modifier
        let block = Block::new().style(Modifier::BOLD | Modifier::ITALIC);
        assert_eq!(block.style, Style::new().bold().italic());

        // auto-convert from (Modifier, Modifier)
        let block = Block::new().style((Modifier::BOLD | Modifier::ITALIC, Modifier::DIM));
        assert_eq!(block.style, Style::new().bold().italic().not_dim());

        // auto-convert from (Color, Modifier)
        let block = Block::new().style((Color::Red, Modifier::BOLD));
        assert_eq!(block.style, Style::new().red().bold());

        // auto-convert from (Color, Color, Modifier)
        let block = Block::new().style((Color::Red, Color::Blue, Modifier::BOLD));
        assert_eq!(block.style, Style::new().red().on_blue().bold());

        // auto-convert from (Color, Color, Modifier, Modifier)
        let block = Block::new().style((
            Color::Red,
            Color::Blue,
            Modifier::BOLD | Modifier::ITALIC,
            Modifier::DIM,
        ));
        assert_eq!(
            block.style,
            Style::new().red().on_blue().bold().italic().not_dim()
        );
    }

    #[test]
    fn can_be_stylized() {
        let block = Block::new().black().on_white().bold().not_dim();
        assert_eq!(
            block.style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn title_top_bottom() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 11, 3));
        Block::bordered()
            .title_top(Line::raw("A").left_aligned())
            .title_top(Line::raw("B").centered())
            .title_top(Line::raw("C").right_aligned())
            .title_bottom(Line::raw("D").left_aligned())
            .title_bottom(Line::raw("E").centered())
            .title_bottom(Line::raw("F").right_aligned())
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌A───B───C┐",
            "│         │",
            "└D───E───F┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn title_alignment() {
        let tests = vec![
            (Alignment::Left, "test    "),
            (Alignment::Center, "  test  "),
            (Alignment::Right, "    test"),
        ];
        for (alignment, expected) in tests {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
            Block::new()
                .title_alignment(alignment)
                .title("test")
                .render(buffer.area, &mut buffer);
            assert_eq!(buffer, Buffer::with_lines([expected]));
        }
    }

    #[test]
    fn title_alignment_overrides_block_title_alignment() {
        let tests = vec![
            (Alignment::Right, Alignment::Left, "test    "),
            (Alignment::Left, Alignment::Center, "  test  "),
            (Alignment::Center, Alignment::Right, "    test"),
        ];
        for (block_title_alignment, alignment, expected) in tests {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
            Block::new()
                .title_alignment(block_title_alignment)
                .title(Line::from("test").alignment(alignment))
                .render(buffer.area, &mut buffer);
            assert_eq!(buffer, Buffer::with_lines([expected]));
        }
    }

    /// This is a regression test for bug <https://github.com/ratatui/ratatui/issues/929>
    #[test]
    fn render_right_aligned_empty_title() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 15, 3));
        Block::new()
            .title_alignment(Alignment::Right)
            .title("")
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["               "; 3]));
    }

    #[test]
    fn title_position() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
        Block::new()
            .title_position(TitlePosition::Bottom)
            .title("test")
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["    ", "test"]));
    }

    #[test]
    fn title_content_style() {
        for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
            Block::new()
                .title_alignment(alignment)
                .title("test".yellow())
                .render(buffer.area, &mut buffer);
            assert_eq!(buffer, Buffer::with_lines(["test".yellow()]));
        }
    }

    #[test]
    fn block_title_style() {
        for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
            Block::new()
                .title_alignment(alignment)
                .title_style(Style::new().yellow())
                .title("test")
                .render(buffer.area, &mut buffer);
            assert_eq!(buffer, Buffer::with_lines(["test".yellow()]));
        }
    }

    #[test]
    fn title_style_overrides_block_title_style() {
        for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
            Block::new()
                .title_alignment(alignment)
                .title_style(Style::new().green().on_red())
                .title("test".yellow())
                .render(buffer.area, &mut buffer);
            assert_eq!(buffer, Buffer::with_lines(["test".yellow().on_red()]));
        }
    }

    #[test]
    fn title_border_style() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .title("test")
            .border_style(Style::new().yellow())
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let mut expected = Buffer::with_lines([
            "┌test────┐",
            "│        │",
            "└────────┘",
        ]);
        expected.set_style(Rect::new(0, 0, 10, 3), Style::new().yellow());
        expected.set_style(Rect::new(1, 1, 8, 1), Style::reset());
        assert_eq!(buffer, expected);
    }

    #[test]
    fn border_type_to_string() {
        assert_eq!(format!("{}", BorderType::Plain), "Plain");
        assert_eq!(format!("{}", BorderType::Rounded), "Rounded");
        assert_eq!(format!("{}", BorderType::Double), "Double");
        assert_eq!(format!("{}", BorderType::Thick), "Thick");
        assert_eq!(
            format!("{}", BorderType::LightDoubleDashed),
            "LightDoubleDashed"
        );
        assert_eq!(
            format!("{}", BorderType::HeavyDoubleDashed),
            "HeavyDoubleDashed"
        );
        assert_eq!(
            format!("{}", BorderType::LightTripleDashed),
            "LightTripleDashed"
        );
        assert_eq!(
            format!("{}", BorderType::HeavyTripleDashed),
            "HeavyTripleDashed"
        );
        assert_eq!(
            format!("{}", BorderType::LightQuadrupleDashed),
            "LightQuadrupleDashed"
        );
        assert_eq!(
            format!("{}", BorderType::HeavyQuadrupleDashed),
            "HeavyQuadrupleDashed"
        );
    }

    #[test]
    fn border_type_from_str() {
        assert_eq!("Plain".parse(), Ok(BorderType::Plain));
        assert_eq!("Rounded".parse(), Ok(BorderType::Rounded));
        assert_eq!("Double".parse(), Ok(BorderType::Double));
        assert_eq!("Thick".parse(), Ok(BorderType::Thick));
        assert_eq!(
            "LightDoubleDashed".parse(),
            Ok(BorderType::LightDoubleDashed)
        );
        assert_eq!(
            "HeavyDoubleDashed".parse(),
            Ok(BorderType::HeavyDoubleDashed)
        );
        assert_eq!(
            "LightTripleDashed".parse(),
            Ok(BorderType::LightTripleDashed)
        );
        assert_eq!(
            "HeavyTripleDashed".parse(),
            Ok(BorderType::HeavyTripleDashed)
        );
        assert_eq!(
            "LightQuadrupleDashed".parse(),
            Ok(BorderType::LightQuadrupleDashed)
        );
        assert_eq!(
            "HeavyQuadrupleDashed".parse(),
            Ok(BorderType::HeavyQuadrupleDashed)
        );
        assert_eq!("".parse::<BorderType>(), Err(ParseError::VariantNotFound));
    }

    #[test]
    fn render_plain_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::Plain)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌────────┐",
            "│        │",
            "└────────┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_rounded_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::Rounded)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "╭────────╮",
            "│        │",
            "╰────────╯",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_double_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::Double)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "╔════════╗",
            "║        ║",
            "╚════════╝",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_quadrant_inside() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::QuadrantInside)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "▗▄▄▄▄▄▄▄▄▖",
            "▐        ▌",
            "▝▀▀▀▀▀▀▀▀▘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_border_quadrant_outside() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::QuadrantOutside)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "▛▀▀▀▀▀▀▀▀▜",
            "▌        ▐",
            "▙▄▄▄▄▄▄▄▄▟",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_solid_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::Thick)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┏━━━━━━━━┓",
            "┃        ┃",
            "┗━━━━━━━━┛",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_light_double_dashed_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::LightDoubleDashed)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌╌╌╌╌╌╌╌╌┐",
            "╎        ╎",
            "└╌╌╌╌╌╌╌╌┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_heavy_double_dashed_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::HeavyDoubleDashed)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┏╍╍╍╍╍╍╍╍┓",
            "╏        ╏",
            "┗╍╍╍╍╍╍╍╍┛",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_light_triple_dashed_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::LightTripleDashed)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌┄┄┄┄┄┄┄┄┐",
            "┆        ┆",
            "└┄┄┄┄┄┄┄┄┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_heavy_triple_dashed_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::HeavyTripleDashed)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┏┅┅┅┅┅┅┅┅┓",
            "┇        ┇",
            "┗┅┅┅┅┅┅┅┅┛",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_light_quadruple_dashed_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::LightQuadrupleDashed)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌┈┈┈┈┈┈┈┈┐",
            "┊        ┊",
            "└┈┈┈┈┈┈┈┈┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_heavy_quadruple_dashed_border() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_type(BorderType::HeavyQuadrupleDashed)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┏┉┉┉┉┉┉┉┉┓",
            "┋        ┋",
            "┗┉┉┉┉┉┉┉┉┛",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_custom_border_set() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::bordered()
            .border_set(border::Set {
                top_left: "1",
                top_right: "2",
                bottom_left: "3",
                bottom_right: "4",
                vertical_left: "L",
                vertical_right: "R",
                horizontal_top: "T",
                horizontal_bottom: "B",
            })
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "1TTTTTTTT2",
            "L        R",
            "3BBBBBBBB4",
        ]);
        assert_eq!(buffer, expected);
    }

    #[rstest]
    #[case::replace(MergeStrategy::Replace)]
    #[case::exact(MergeStrategy::Exact)]
    #[case::fuzzy(MergeStrategy::Fuzzy)]
    fn render_partial_borders(#[case] strategy: MergeStrategy) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌────────┐",
            "│        │",
            "└────────┘",
        ]);
        assert_eq!(buffer, expected);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::TOP | Borders::LEFT)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌─────────",
            "│         ",
            "│         ",
        ]);
        assert_eq!(buffer, expected);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::TOP | Borders::RIGHT)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "─────────┐",
            "         │",
            "         │",
        ]);
        assert_eq!(buffer, expected);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::BOTTOM | Borders::LEFT)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "│         ",
            "│         ",
            "└─────────",
        ]);
        assert_eq!(buffer, expected);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::BOTTOM | Borders::RIGHT)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "         │",
            "         │",
            "─────────┘",
        ]);
        assert_eq!(buffer, expected);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "──────────",
            "          ",
            "──────────",
        ]);
        assert_eq!(buffer, expected);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Block::new()
            .borders(Borders::LEFT | Borders::RIGHT)
            .merge_borders(strategy)
            .render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "│        │",
            "│        │",
            "│        │",
        ]);
        assert_eq!(buffer, expected);
    }

    /// Renders a series of blocks with all the possible border types and merges them according to
    /// the specified strategy. The resulting buffer is compared against the expected output for
    /// each merge strategy.
    ///
    /// At some point, it might be convenient to replace the manual `include_str!` calls with
    /// [insta](https://crates.io/crates/insta)
    #[rstest]
    #[case::replace(MergeStrategy::Replace, include_str!("../tests/block/merge_replace.txt"))]
    #[case::exact(MergeStrategy::Exact, include_str!("../tests/block/merge_exact.txt"))]
    #[case::fuzzy(MergeStrategy::Fuzzy, include_str!("../tests/block/merge_fuzzy.txt"))]
    fn render_merged_borders(#[case] strategy: MergeStrategy, #[case] expected: &'static str) {
        let border_types = [
            BorderType::Plain,
            BorderType::Rounded,
            BorderType::Thick,
            BorderType::Double,
            BorderType::LightDoubleDashed,
            BorderType::HeavyDoubleDashed,
            BorderType::LightTripleDashed,
            BorderType::HeavyTripleDashed,
            BorderType::LightQuadrupleDashed,
            BorderType::HeavyQuadrupleDashed,
        ];
        let rects = [
            // touching at corners
            (Rect::new(0, 0, 5, 5), Rect::new(4, 4, 5, 5)),
            // overlapping
            (Rect::new(10, 0, 5, 5), Rect::new(12, 2, 5, 5)),
            // touching vertical edges
            (Rect::new(18, 0, 5, 5), Rect::new(22, 0, 5, 5)),
            // touching horizontal edges
            (Rect::new(28, 0, 5, 5), Rect::new(28, 4, 5, 5)),
        ];

        let mut buffer = Buffer::empty(Rect::new(0, 0, 43, 1000));

        let mut offset = Offset::ZERO;
        for (border_type_1, border_type_2) in iproduct!(border_types, border_types) {
            let title = format!("{border_type_1} + {border_type_2}");
            let title_area = Rect::new(0, 0, 43, 1).offset(offset);
            title.render(title_area, &mut buffer);
            offset.y += 1;
            for (rect_1, rect_2) in rects {
                Block::bordered()
                    .border_type(border_type_1)
                    .merge_borders(strategy)
                    .render(rect_1.offset(offset), &mut buffer);
                Block::bordered()
                    .border_type(border_type_2)
                    .merge_borders(strategy)
                    .render(rect_2.offset(offset), &mut buffer);
            }
            offset.y += 9;
        }
        pretty_assertions::assert_eq!(Buffer::with_lines(expected.lines()), buffer);
    }

    #[rstest]
    #[case::replace(MergeStrategy::Replace, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┗━━━━━━━━━━━┛",
            "│           │",
            "└───────────┘",
        ])
    )]
    #[case::replace(MergeStrategy::Exact, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┡block btm━━┩",
            "│           │",
            "└───────────┘",
        ])
    )]
    #[case::replace(MergeStrategy::Fuzzy, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┡block btm━━┩",
            "│           │",
            "└───────────┘",
        ])
    )]
    fn merged_titles_bottom_first(#[case] strategy: MergeStrategy, #[case] expected: Buffer) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 13, 5));
        Block::bordered()
            .title("block btm")
            .render(Rect::new(0, 2, 13, 3), &mut buffer);
        Block::bordered()
            .title("block top")
            .border_type(BorderType::Thick)
            .merge_borders(strategy)
            .render(Rect::new(0, 0, 13, 3), &mut buffer);
        assert_eq!(buffer, expected);
    }

    #[rstest]
    #[case::replace(MergeStrategy::Replace, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┌block btm──┐",
            "│           │",
            "└───────────┘",
        ])
    )]
    #[case::replace(MergeStrategy::Exact, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┞block btm──┦",
            "│           │",
            "└───────────┘",
        ])
    )]
    #[case::replace(MergeStrategy::Fuzzy, Buffer::with_lines([
            "┏block top━━┓",
            "┃           ┃",
            "┞block btm──┦",
            "│           │",
            "└───────────┘",
        ])
    )]
    fn merged_titles_top_first(#[case] strategy: MergeStrategy, #[case] expected: Buffer) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 13, 5));
        Block::bordered()
            .title("block top")
            .border_type(BorderType::Thick)
            .render(Rect::new(0, 0, 13, 3), &mut buffer);
        Block::bordered()
            .title("block btm")
            .merge_borders(strategy)
            .render(Rect::new(0, 2, 13, 3), &mut buffer);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn left_titles() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title("L12")
            .title("L34")
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["L12 L34   "]));
    }

    #[test]
    fn left_titles_truncated() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title("L12345")
            .title("L67890")
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["L12345 L67"]));
    }

    #[test]
    fn center_titles() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title(Line::from("C12").centered())
            .title(Line::from("C34").centered())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines([" C12 C34  "]));
    }

    #[test]
    fn center_titles_truncated() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title(Line::from("C12345").centered())
            .title(Line::from("C67890").centered())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["12345 C678"]));
    }

    #[test]
    fn right_titles() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title(Line::from("R12").right_aligned())
            .title(Line::from("R34").right_aligned())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["   R12 R34"]));
    }

    #[test]
    fn right_titles_truncated() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title(Line::from("R12345").right_aligned())
            .title(Line::from("R67890").right_aligned())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["345 R67890"]));
    }

    #[test]
    fn center_title_truncates_left_title() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title("L1234")
            .title(Line::from("C5678").centered())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["L1C5678   "]));
    }

    #[test]
    fn right_title_truncates_left_title() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title("L12345")
            .title(Line::from("R67890").right_aligned())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["L123R67890"]));
    }

    #[test]
    fn right_title_truncates_center_title() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        Block::new()
            .title(Line::from("C12345").centered())
            .title(Line::from("R67890").right_aligned())
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["  C1R67890"]));
    }

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        // This should not panic, even if the buffer is too small to render the block.
        Block::bordered()
            .title("I'm too big for this buffer")
            .padding(Padding::uniform(10))
            .render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["┌"]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        // This should not panic, even if the buffer has zero size.
        Block::bordered()
            .title("I'm too big for this buffer")
            .padding(Padding::uniform(10))
            .render(buffer.area, &mut buffer);
    }
}

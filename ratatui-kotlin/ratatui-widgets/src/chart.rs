//! The [`Chart`] widget is used to plot one or more [`Dataset`] in a cartesian coordinate system.
use alloc::vec::Vec;
use core::cmp::max;
use core::ops::Not;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Constraint, Flex, Layout, Position, Rect};
use ratatui_core::style::{Color, Style, Styled};
use ratatui_core::symbols;
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;
use strum::{Display, EnumString};

use crate::block::{Block, BlockExt};
use crate::canvas::{Canvas, Line as CanvasLine, Points};

/// An X or Y axis for the [`Chart`] widget
///
/// An axis can have a [title](Axis::title) which will be displayed at the end of the axis. For an
/// X axis this is the right, for a Y axis, this is the top.
///
/// You can also set the bounds and labels on this axis using respectively [`Axis::bounds`] and
/// [`Axis::labels`].
///
/// See [`Chart::x_axis`] and [`Chart::y_axis`] to set an axis on a chart.
///
/// # Example
///
/// ```rust
/// use ratatui::style::{Style, Stylize};
/// use ratatui::widgets::Axis;
///
/// let axis = Axis::default()
///     .title("X Axis")
///     .style(Style::default().gray())
///     .bounds([0.0, 50.0])
///     .labels(["0".bold(), "25".into(), "50".bold()]);
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Axis<'a> {
    /// Title displayed next to axis end
    title: Option<Line<'a>>,
    /// Bounds for the axis (all data points outside these limits will not be represented)
    bounds: [f64; 2],
    /// A list of labels to put to the left or below the axis
    labels: Vec<Line<'a>>,
    /// The style used to draw the axis itself
    style: Style,
    /// The alignment of the labels of the Axis
    labels_alignment: Alignment,
}

impl<'a> Axis<'a> {
    /// Sets the axis title
    ///
    /// It will be displayed at the end of the axis. For an X axis this is the right, for a Y axis,
    /// this is the top.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn title<T>(mut self, title: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.title = Some(title.into());
        self
    }

    /// Sets the bounds of this axis
    ///
    /// In other words, sets the min and max value on this axis.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn bounds(mut self, bounds: [f64; 2]) -> Self {
        self.bounds = bounds;
        self
    }

    /// Sets the axis labels
    ///
    /// - For the X axis, the labels are displayed left to right.
    /// - For the Y axis, the labels are displayed bottom to top.
    ///
    /// Currently, you need to give at least two labels or the render will panic. Also, giving
    /// more than 3 labels is currently broken and the middle labels won't be in the correct
    /// position, see [issue 334].
    ///
    /// [issue 334]: https://github.com/ratatui/ratatui/issues/334
    ///
    /// `labels` is a vector of any type that can be converted into a [`Line`] (e.g. `&str`,
    /// `String`, `&Line`, `Span`, ...). This allows you to style the labels using the methods
    /// provided by [`Line`]. Any alignment set on the labels will be ignored as the alignment is
    /// determined by the axis.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::widgets::Axis;
    ///
    /// let axis = Axis::default()
    ///     .bounds([0.0, 50.0])
    ///     .labels(["0".bold(), "25".into(), "50".bold()]);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn labels<Labels>(mut self, labels: Labels) -> Self
    where
        Labels: IntoIterator,
        Labels::Item: Into<Line<'a>>,
    {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the axis style
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// # Example
    ///
    /// [`Axis`] also implements [`Stylize`](ratatui_core::style::Stylize) which mean you can style
    /// it like so
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::widgets::Axis;
    ///
    /// let axis = Axis::default().red();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the labels alignment of the axis
    ///
    /// The alignment behaves differently based on the axis:
    /// - Y axis: The labels are aligned within the area on the left of the axis
    /// - X axis: The first X-axis label is aligned relative to the Y-axis
    ///
    /// On the X axis, this parameter only affects the first label.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn labels_alignment(mut self, alignment: Alignment) -> Self {
        self.labels_alignment = alignment;
        self
    }
}

/// Used to determine which style of graphing to use
#[derive(Debug, Default, Display, EnumString, Clone, Copy, Eq, PartialEq, Hash)]
pub enum GraphType {
    /// Draw each point. This is the default.
    #[default]
    Scatter,

    /// Draw a line between each following point.
    ///
    /// The order of the lines will be the same as the order of the points in the dataset, which
    /// allows this widget to draw lines both left-to-right and right-to-left
    Line,

    /// Draw a bar chart. This will draw a bar for each point in the dataset.
    Bar,
}

/// Allow users to specify the position of a legend in a [`Chart`]
///
/// See [`Chart::legend_position`]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum LegendPosition {
    /// Legend is centered on top
    Top,
    /// Legend is in the top-right corner. This is the **default**.
    #[default]
    TopRight,
    /// Legend is in the top-left corner
    TopLeft,
    /// Legend is centered on the left
    Left,
    /// Legend is centered on the right
    Right,
    /// Legend is centered on the bottom
    Bottom,
    /// Legend is in the bottom-right corner
    BottomRight,
    /// Legend is in the bottom-left corner
    BottomLeft,
}

impl LegendPosition {
    fn layout(
        self,
        area: Rect,
        legend_width: u16,
        legend_height: u16,
        x_title_width: u16,
        y_title_width: u16,
    ) -> Option<Rect> {
        let mut height_margin = i32::from(area.height - legend_height);
        if x_title_width != 0 {
            height_margin -= 1;
        }
        if y_title_width != 0 {
            height_margin -= 1;
        }
        if height_margin < 0 {
            return None;
        }

        let (x, y) = match self {
            Self::TopRight => {
                if legend_width + y_title_width > area.width {
                    (area.right() - legend_width, area.top() + 1)
                } else {
                    (area.right() - legend_width, area.top())
                }
            }
            Self::TopLeft => {
                if y_title_width != 0 {
                    (area.left(), area.top() + 1)
                } else {
                    (area.left(), area.top())
                }
            }
            Self::Top => {
                let x = (area.width - legend_width) / 2;
                if area.left() + y_title_width > x {
                    (area.left() + x, area.top() + 1)
                } else {
                    (area.left() + x, area.top())
                }
            }
            Self::Left => {
                let mut y = (area.height - legend_height) / 2;
                if y_title_width != 0 {
                    y += 1;
                }
                if x_title_width != 0 {
                    y = y.saturating_sub(1);
                }
                (area.left(), area.top() + y)
            }
            Self::Right => {
                let mut y = (area.height - legend_height) / 2;
                if y_title_width != 0 {
                    y += 1;
                }
                if x_title_width != 0 {
                    y = y.saturating_sub(1);
                }
                (area.right() - legend_width, area.top() + y)
            }
            Self::BottomLeft => {
                if x_title_width + legend_width > area.width {
                    (area.left(), area.bottom() - legend_height - 1)
                } else {
                    (area.left(), area.bottom() - legend_height)
                }
            }
            Self::BottomRight => {
                if x_title_width != 0 {
                    (
                        area.right() - legend_width,
                        area.bottom() - legend_height - 1,
                    )
                } else {
                    (area.right() - legend_width, area.bottom() - legend_height)
                }
            }
            Self::Bottom => {
                let x = area.left() + (area.width - legend_width) / 2;
                if x + legend_width > area.right() - x_title_width {
                    (x, area.bottom() - legend_height - 1)
                } else {
                    (x, area.bottom() - legend_height)
                }
            }
        };

        Some(Rect::new(x, y, legend_width, legend_height))
    }
}

/// A group of data points
///
/// This is the main element composing a [`Chart`].
///
/// A dataset can be [named](Dataset::name). Only named datasets will be rendered in the legend.
///
/// After that, you can pass it data with [`Dataset::data`]. Data is an array of `f64` tuples
/// (`(f64, f64)`), the first element being X and the second Y. It's also worth noting that, unlike
/// the [`Rect`], here the Y axis is bottom to top, as in math.
///
/// You can also customize the rendering by using [`Dataset::marker`] and [`Dataset::graph_type`].
///
/// # Example
///
/// This example draws a red line between two points.
///
/// ```rust
/// use ratatui::style::Stylize;
/// use ratatui::symbols::Marker;
/// use ratatui::widgets::{Dataset, GraphType};
///
/// let dataset = Dataset::default()
///     .name("dataset 1")
///     .data(&[(1., 1.), (5., 5.)])
///     .marker(Marker::Braille)
///     .graph_type(GraphType::Line)
///     .red();
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Dataset<'a> {
    /// Name of the dataset (used in the legend if shown)
    name: Option<Line<'a>>,
    /// A reference to the actual data
    data: &'a [(f64, f64)],
    /// Symbol used for each points of this dataset
    marker: symbols::Marker,
    /// Determines graph type used for drawing points
    graph_type: GraphType,
    /// Style used to plot this dataset
    style: Style,
}

impl<'a> Dataset<'a> {
    /// Sets the name of the dataset
    ///
    /// The dataset's name is used when displaying the chart legend. Datasets don't require a name
    /// and can be created without specifying one. Once assigned, a name can't be removed, only
    /// changed
    ///
    /// The name can be styled (see [`Line`] for that), but the dataset's style will always have
    /// precedence.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn name<S>(mut self, name: S) -> Self
    where
        S: Into<Line<'a>>,
    {
        self.name = Some(name.into());
        self
    }

    /// Sets the data points of this dataset
    ///
    /// Points will then either be rendered as scattered points or with lines between them
    /// depending on [`Dataset::graph_type`].
    ///
    /// Data consist in an array of `f64` tuples (`(f64, f64)`), the first element being X and the
    /// second Y. It's also worth noting that, unlike the [`Rect`], here the Y axis is bottom to
    /// top, as in math.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn data(mut self, data: &'a [(f64, f64)]) -> Self {
        self.data = data;
        self
    }

    /// Sets the kind of character to use to display this dataset
    ///
    /// You can use dots (`•`), blocks (`█`), bars (`▄`), braille (`⠓`, `⣇`, `⣿`) or half-blocks
    /// (`█`, `▄`, and `▀`). See [`symbols::Marker`] for more details.
    ///
    /// Note [`Marker::Braille`](symbols::Marker::Braille) requires a font that supports Unicode
    /// Braille Patterns.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn marker(mut self, marker: symbols::Marker) -> Self {
        self.marker = marker;
        self
    }

    /// Sets how the dataset should be drawn
    ///
    /// [`Chart`] can draw [scatter](GraphType::Scatter), [line](GraphType::Line) or
    /// [bar](GraphType::Bar) charts. A scatter chart draws only the points in the dataset, a line
    /// char draws a line between each point, and a bar chart draws a line from the x axis to the
    /// point.  See [`GraphType`] for more details
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn graph_type(mut self, graph_type: GraphType) -> Self {
        self.graph_type = graph_type;
        self
    }

    /// Sets the style of this dataset
    ///
    /// The given style will be used to draw the legend and the data points. Currently the legend
    /// will use the entire style whereas the data points will only use the foreground.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Example
    ///
    /// [`Dataset`] also implements [`Stylize`](ratatui_core::style::Stylize) which mean you can
    /// style it like so
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::widgets::Dataset;
    ///
    /// let dataset = Dataset::default().red();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

/// A container that holds all the infos about where to display each elements of the chart (axis,
/// labels, legend, ...).
struct ChartLayout {
    /// Location of the title of the x axis
    title_x: Option<Position>,
    /// Location of the title of the y axis
    title_y: Option<Position>,
    /// Location of the first label of the x axis
    label_x: Option<u16>,
    /// Location of the first label of the y axis
    label_y: Option<u16>,
    /// Y coordinate of the horizontal axis
    axis_x: Option<u16>,
    /// X coordinate of the vertical axis
    axis_y: Option<u16>,
    /// Area of the legend
    legend_area: Option<Rect>,
    /// Area of the graph
    graph_area: Rect,
}

/// A widget to plot one or more [`Dataset`] in a cartesian coordinate system
///
/// To use this widget, start by creating one or more [`Dataset`]. With it, you can set the
/// [data points](Dataset::data), the [name](Dataset::name) or the
/// [chart type](Dataset::graph_type). See [`Dataset`] for a complete documentation of what is
/// possible.
///
/// Then, you'll usually want to configure the [`Axis`]. Axis [titles](Axis::title),
/// [bounds](Axis::bounds) and [labels](Axis::labels) can be configured on both axis. See [`Axis`]
/// for a complete documentation of what is possible.
///
/// Finally, you can pass all of that to the `Chart` via [`Chart::new`], [`Chart::x_axis`] and
/// [`Chart::y_axis`].
///
/// Additionally, `Chart` allows configuring the legend [position](Chart::legend_position) and
/// [hiding constraints](Chart::hidden_legend_constraints).
///
/// # Examples
///
/// ```
/// use ratatui::style::{Style, Stylize};
/// use ratatui::symbols;
/// use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType};
///
/// // Create the datasets to fill the chart with
/// let datasets = vec![
///     // Scatter chart
///     Dataset::default()
///         .name("data1")
///         .marker(symbols::Marker::Dot)
///         .graph_type(GraphType::Scatter)
///         .style(Style::default().cyan())
///         .data(&[(0.0, 5.0), (1.0, 6.0), (1.5, 6.434)]),
///     // Line chart
///     Dataset::default()
///         .name("data2")
///         .marker(symbols::Marker::Braille)
///         .graph_type(GraphType::Line)
///         .style(Style::default().magenta())
///         .data(&[(4.0, 5.0), (5.0, 8.0), (7.66, 13.5)]),
/// ];
///
/// // Create the X axis and define its properties
/// let x_axis = Axis::default()
///     .title("X Axis".red())
///     .style(Style::default().white())
///     .bounds([0.0, 10.0])
///     .labels(["0.0", "5.0", "10.0"]);
///
/// // Create the Y axis and define its properties
/// let y_axis = Axis::default()
///     .title("Y Axis".red())
///     .style(Style::default().white())
///     .bounds([0.0, 10.0])
///     .labels(["0.0", "5.0", "10.0"]);
///
/// // Create the chart and link all the parts together
/// let chart = Chart::new(datasets)
///     .block(Block::new().title("Chart"))
///     .x_axis(x_axis)
///     .y_axis(y_axis);
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Chart<'a> {
    /// A block to display around the widget eventually
    block: Option<Block<'a>>,
    /// The horizontal axis
    x_axis: Axis<'a>,
    /// The vertical axis
    y_axis: Axis<'a>,
    /// A reference to the datasets
    datasets: Vec<Dataset<'a>>,
    /// The widget base style
    style: Style,
    /// Constraints used to determine whether the legend should be shown or not
    hidden_legend_constraints: (Constraint, Constraint),
    /// The position determine where the length is shown or hide regardless of
    /// `hidden_legend_constraints`
    legend_position: Option<LegendPosition>,
}

impl<'a> Chart<'a> {
    /// Creates a chart with the given [datasets](Dataset)
    ///
    /// A chart can render multiple datasets.
    ///
    /// # Example
    ///
    /// This creates a simple chart with one [`Dataset`]
    ///
    /// ```rust
    /// use ratatui::widgets::{Chart, Dataset};
    ///
    /// let data_points = vec![];
    /// let chart = Chart::new(vec![Dataset::default().data(&data_points)]);
    /// ```
    ///
    /// This creates a chart with multiple [`Dataset`]s
    ///
    /// ```rust
    /// use ratatui::widgets::{Chart, Dataset};
    ///
    /// let data_points = vec![];
    /// let data_points2 = vec![];
    /// let chart = Chart::new(vec![
    ///     Dataset::default().data(&data_points),
    ///     Dataset::default().data(&data_points2),
    /// ]);
    /// ```
    pub fn new(datasets: Vec<Dataset<'a>>) -> Self {
        Self {
            block: None,
            x_axis: Axis::default(),
            y_axis: Axis::default(),
            style: Style::default(),
            datasets,
            hidden_legend_constraints: (Constraint::Ratio(1, 4), Constraint::Ratio(1, 4)),
            legend_position: Some(LegendPosition::default()),
        }
    }

    /// Wraps the chart with the given [`Block`]
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Sets the style of the entire chart
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// Styles of [`Axis`] and [`Dataset`] will have priority over this style.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the X [`Axis`]
    ///
    /// The default is an empty [`Axis`], i.e. only a line.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui::widgets::{Axis, Chart};
    ///
    /// let chart = Chart::new(vec![]).x_axis(
    ///     Axis::default()
    ///         .title("X Axis")
    ///         .bounds([0.0, 20.0])
    ///         .labels(["0", "20"]),
    /// );
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn x_axis(mut self, axis: Axis<'a>) -> Self {
        self.x_axis = axis;
        self
    }

    /// Sets the Y [`Axis`]
    ///
    /// The default is an empty [`Axis`], i.e. only a line.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui::widgets::{Axis, Chart};
    ///
    /// let chart = Chart::new(vec![]).y_axis(
    ///     Axis::default()
    ///         .title("Y Axis")
    ///         .bounds([0.0, 20.0])
    ///         .labels(["0", "20"]),
    /// );
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn y_axis(mut self, axis: Axis<'a>) -> Self {
        self.y_axis = axis;
        self
    }

    /// Sets the constraints used to determine whether the legend should be shown or not.
    ///
    /// The tuple's first constraint is used for the width and the second for the height. If the
    /// legend takes more space than what is allowed by any constraint, the legend is hidden.
    /// [`Constraint::Min`] is an exception and will always show the legend.
    ///
    /// If this is not set, the default behavior is to hide the legend if it is greater than 25% of
    /// the chart, either horizontally or vertically.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// Hide the legend when either its width is greater than 33% of the total widget width or if
    /// its height is greater than 25% of the total widget height.
    ///
    /// ```
    /// use ratatui::layout::Constraint;
    /// use ratatui::widgets::Chart;
    ///
    /// let constraints = (Constraint::Ratio(1, 3), Constraint::Ratio(1, 4));
    /// let chart = Chart::new(vec![]).hidden_legend_constraints(constraints);
    /// ```
    ///
    /// Always show the legend, note the second constraint doesn't matter in this case since the
    /// first one is always true.
    ///
    /// ```
    /// use ratatui::layout::Constraint;
    /// use ratatui::widgets::Chart;
    ///
    /// let constraints = (Constraint::Min(0), Constraint::Ratio(1, 4));
    /// let chart = Chart::new(vec![]).hidden_legend_constraints(constraints);
    /// ```
    ///
    /// Always hide the legend. Note this can be accomplished more explicitly by passing `None` to
    /// [`Chart::legend_position`].
    ///
    /// ```
    /// use ratatui::layout::Constraint;
    /// use ratatui::widgets::Chart;
    ///
    /// let constraints = (Constraint::Length(0), Constraint::Ratio(1, 4));
    /// let chart = Chart::new(vec![]).hidden_legend_constraints(constraints);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn hidden_legend_constraints(
        mut self,
        constraints: (Constraint, Constraint),
    ) -> Self {
        self.hidden_legend_constraints = constraints;
        self
    }

    /// Sets the position of a legend or hide it
    ///
    /// The default is [`LegendPosition::TopRight`].
    ///
    /// If [`None`] is given, hide the legend even if [`hidden_legend_constraints`] determines it
    /// should be shown. In contrast, if `Some(...)` is given, [`hidden_legend_constraints`] might
    /// still decide whether to show the legend or not.
    ///
    /// See [`LegendPosition`] for all available positions.
    ///
    /// [`hidden_legend_constraints`]: Self::hidden_legend_constraints
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// Show the legend on the top left corner.
    ///
    /// ```
    /// use ratatui::widgets::{Chart, LegendPosition};
    ///
    /// let chart: Chart = Chart::new(vec![]).legend_position(Some(LegendPosition::TopLeft));
    /// ```
    ///
    /// Hide the legend altogether
    ///
    /// ```
    /// use ratatui::widgets::{Chart, LegendPosition};
    ///
    /// let chart = Chart::new(vec![]).legend_position(None);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn legend_position(mut self, position: Option<LegendPosition>) -> Self {
        self.legend_position = position;
        self
    }

    /// Compute the internal layout of the chart given the area. If the area is too small some
    /// elements may be automatically hidden
    fn layout(&self, area: Rect) -> Option<ChartLayout> {
        if area.height == 0 || area.width == 0 {
            return None;
        }
        let mut x = area.left();
        let mut y = area.bottom() - 1;

        let mut label_x = None;
        if !self.x_axis.labels.is_empty() && y > area.top() {
            label_x = Some(y);
            y -= 1;
        }

        let label_y = self.y_axis.labels.is_empty().not().then_some(x);
        x += self.max_width_of_labels_left_of_y_axis(area, !self.y_axis.labels.is_empty());

        let mut axis_x = None;
        if !self.x_axis.labels.is_empty() && y > area.top() {
            axis_x = Some(y);
            y -= 1;
        }

        let mut axis_y = None;
        if !self.y_axis.labels.is_empty() && x + 1 < area.right() {
            axis_y = Some(x);
            x += 1;
        }

        let graph_width = area.right().saturating_sub(x);
        let graph_height = y.saturating_sub(area.top()).saturating_add(1);
        debug_assert_ne!(
            graph_width, 0,
            "Axis and labels should have been hidden due to the small area"
        );
        debug_assert_ne!(
            graph_height, 0,
            "Axis and labels should have been hidden due to the small area"
        );
        let graph_area = Rect::new(x, area.top(), graph_width, graph_height);

        let mut title_x = None;
        if let Some(ref title) = self.x_axis.title {
            let w = title.width() as u16;
            if w < graph_area.width && graph_area.height > 2 {
                title_x = Some(Position::new(x + graph_area.width - w, y));
            }
        }

        let mut title_y = None;
        if let Some(ref title) = self.y_axis.title {
            let w = title.width() as u16;
            if w + 1 < graph_area.width && graph_area.height > 2 {
                title_y = Some(Position::new(x, area.top()));
            }
        }

        let mut legend_area = None;
        if let Some(legend_position) = self.legend_position {
            let legends = self
                .datasets
                .iter()
                .filter_map(|d| Some(d.name.as_ref()?.width() as u16));

            if let Some(inner_width) = legends.clone().max() {
                let legend_width = inner_width + 2;
                let legend_height = legends.count() as u16 + 2;

                let [max_legend_width] = Layout::horizontal([self.hidden_legend_constraints.0])
                    .flex(Flex::Start)
                    .areas(graph_area);

                let [max_legend_height] = Layout::vertical([self.hidden_legend_constraints.1])
                    .flex(Flex::Start)
                    .areas(graph_area);

                if inner_width > 0
                    && legend_width <= max_legend_width.width
                    && legend_height <= max_legend_height.height
                {
                    legend_area = legend_position.layout(
                        graph_area,
                        legend_width,
                        legend_height,
                        title_x
                            .and(self.x_axis.title.as_ref())
                            .map(|t| t.width() as u16)
                            .unwrap_or_default(),
                        title_y
                            .and(self.y_axis.title.as_ref())
                            .map(|t| t.width() as u16)
                            .unwrap_or_default(),
                    );
                }
            }
        }
        Some(ChartLayout {
            title_x,
            title_y,
            label_x,
            label_y,
            axis_x,
            axis_y,
            legend_area,
            graph_area,
        })
    }

    fn max_width_of_labels_left_of_y_axis(&self, area: Rect, has_y_axis: bool) -> u16 {
        let mut max_width = self
            .y_axis
            .labels
            .iter()
            .map(Line::width)
            .max()
            .unwrap_or_default() as u16;

        if let Some(first_x_label) = self.x_axis.labels.first() {
            let first_label_width = first_x_label.width() as u16;
            let width_left_of_y_axis = match self.x_axis.labels_alignment {
                Alignment::Left => {
                    // The last character of the label should be below the Y-Axis when it exists,
                    // not on its left
                    let y_axis_offset = u16::from(has_y_axis);
                    first_label_width.saturating_sub(y_axis_offset)
                }
                Alignment::Center => first_label_width / 2,
                Alignment::Right => 0,
            };
            max_width = max(max_width, width_left_of_y_axis);
        }
        // labels of y axis and first label of x axis can take at most 1/3rd of the total width
        max_width.min(area.width / 3)
    }

    fn render_x_labels(
        &self,
        buf: &mut Buffer,
        layout: &ChartLayout,
        chart_area: Rect,
        graph_area: Rect,
    ) {
        let Some(y) = layout.label_x else { return };
        let labels = &self.x_axis.labels;
        let labels_len = labels.len() as u16;
        if labels_len < 2 {
            return;
        }

        let width_between_ticks = graph_area.width / labels_len;

        let label_area = self.first_x_label_area(
            y,
            labels.first().unwrap().width() as u16,
            width_between_ticks,
            chart_area,
            graph_area,
        );

        let label_alignment = match self.x_axis.labels_alignment {
            Alignment::Left => Alignment::Right,
            Alignment::Center => Alignment::Center,
            Alignment::Right => Alignment::Left,
        };

        Self::render_label(buf, labels.first().unwrap(), label_area, label_alignment);

        for (i, label) in labels[1..labels.len() - 1].iter().enumerate() {
            // We add 1 to x (and width-1 below) to leave at least one space before each
            // intermediate labels
            let x = graph_area.left() + (i + 1) as u16 * width_between_ticks + 1;
            let label_area = Rect::new(x, y, width_between_ticks.saturating_sub(1), 1);

            Self::render_label(buf, label, label_area, Alignment::Center);
        }

        let x = graph_area.right() - width_between_ticks;
        let label_area = Rect::new(x, y, width_between_ticks, 1);
        // The last label should be aligned Right to be at the edge of the graph area
        Self::render_label(buf, labels.last().unwrap(), label_area, Alignment::Right);
    }

    fn first_x_label_area(
        &self,
        y: u16,
        label_width: u16,
        max_width_after_y_axis: u16,
        chart_area: Rect,
        graph_area: Rect,
    ) -> Rect {
        let (min_x, max_x) = match self.x_axis.labels_alignment {
            Alignment::Left => (chart_area.left(), graph_area.left()),
            Alignment::Center => (
                chart_area.left(),
                graph_area.left() + max_width_after_y_axis.min(label_width),
            ),
            Alignment::Right => (
                graph_area.left().saturating_sub(1),
                graph_area.left() + max_width_after_y_axis,
            ),
        };

        Rect::new(min_x, y, max_x - min_x, 1)
    }

    fn render_label(buf: &mut Buffer, label: &Line, label_area: Rect, alignment: Alignment) {
        let label = match alignment {
            Alignment::Left => label.clone().left_aligned(),
            Alignment::Center => label.clone().centered(),
            Alignment::Right => label.clone().right_aligned(),
        };
        label.render(label_area, buf);
    }

    fn render_y_labels(
        &self,
        buf: &mut Buffer,
        layout: &ChartLayout,
        chart_area: Rect,
        graph_area: Rect,
    ) {
        let Some(x) = layout.label_y else { return };
        let labels = &self.y_axis.labels;
        let labels_len = labels.len() as u16;
        for (i, label) in labels.iter().enumerate() {
            let dy = i as u16 * (graph_area.height - 1) / (labels_len - 1);
            if dy < graph_area.bottom() {
                let label_area = Rect::new(
                    x,
                    graph_area.bottom().saturating_sub(1) - dy,
                    (graph_area.left() - chart_area.left()).saturating_sub(1),
                    1,
                );
                Self::render_label(buf, label, label_area, self.y_axis.labels_alignment);
            }
        }
    }
}

impl Widget for Chart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Chart<'_> {
    #[expect(clippy::too_many_lines)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);

        self.block.as_ref().render(area, buf);
        let chart_area = self.block.inner_if_some(area);
        let Some(layout) = self.layout(chart_area) else {
            return;
        };
        let graph_area = layout.graph_area;

        // Sample the style of the entire widget. This sample will be used to reset the style of
        // the cells that are part of the components put on top of the grah area (i.e legend and
        // axis names).
        let original_style = buf[(area.left(), area.top())].style();

        self.render_x_labels(buf, &layout, chart_area, graph_area);
        self.render_y_labels(buf, &layout, chart_area, graph_area);

        if let Some(y) = layout.axis_x {
            for x in graph_area.left()..graph_area.right() {
                buf[(x, y)]
                    .set_symbol(symbols::line::HORIZONTAL)
                    .set_style(self.x_axis.style);
            }
        }

        if let Some(x) = layout.axis_y {
            for y in graph_area.top()..graph_area.bottom() {
                buf[(x, y)]
                    .set_symbol(symbols::line::VERTICAL)
                    .set_style(self.y_axis.style);
            }
        }

        if let Some(y) = layout.axis_x {
            if let Some(x) = layout.axis_y {
                buf[(x, y)]
                    .set_symbol(symbols::line::BOTTOM_LEFT)
                    .set_style(self.x_axis.style);
            }
        }

        Canvas::default()
            .background_color(self.style.bg.unwrap_or(Color::Reset))
            .x_bounds(self.x_axis.bounds)
            .y_bounds(self.y_axis.bounds)
            .paint(|ctx| {
                for dataset in &self.datasets {
                    ctx.marker(dataset.marker);

                    let color = dataset.style.fg.unwrap_or(Color::Reset);
                    ctx.draw(&Points {
                        coords: dataset.data,
                        color,
                    });
                    match dataset.graph_type {
                        GraphType::Line => {
                            for data in dataset.data.windows(2) {
                                ctx.draw(&CanvasLine {
                                    x1: data[0].0,
                                    y1: data[0].1,
                                    x2: data[1].0,
                                    y2: data[1].1,
                                    color,
                                });
                            }
                        }
                        GraphType::Bar => {
                            for (x, y) in dataset.data {
                                ctx.draw(&CanvasLine {
                                    x1: *x,
                                    y1: 0.0,
                                    x2: *x,
                                    y2: *y,
                                    color,
                                });
                            }
                        }
                        GraphType::Scatter => {}
                    }
                }
            })
            .render(graph_area, buf);

        if let Some(Position { x, y }) = layout.title_x {
            let title = self.x_axis.title.as_ref().unwrap();
            let width = graph_area
                .right()
                .saturating_sub(x)
                .min(title.width() as u16);
            buf.set_style(
                Rect {
                    x,
                    y,
                    width,
                    height: 1,
                },
                original_style,
            );
            buf.set_line(x, y, title, width);
        }

        if let Some(Position { x, y }) = layout.title_y {
            let title = self.y_axis.title.as_ref().unwrap();
            let width = graph_area
                .right()
                .saturating_sub(x)
                .min(title.width() as u16);
            buf.set_style(
                Rect {
                    x,
                    y,
                    width,
                    height: 1,
                },
                original_style,
            );
            buf.set_line(x, y, title, width);
        }

        if let Some(legend_area) = layout.legend_area {
            buf.set_style(legend_area, original_style);
            Block::bordered().render(legend_area, buf);

            for (i, (dataset_name, dataset_style)) in self
                .datasets
                .iter()
                .filter_map(|ds| Some((ds.name.as_ref()?, ds.style())))
                .enumerate()
            {
                let name = dataset_name.clone().patch_style(dataset_style);
                name.render(
                    Rect {
                        x: legend_area.x + 1,
                        y: legend_area.y + 1 + i as u16,
                        width: legend_area.width - 2,
                        height: 1,
                    },
                    buf,
                );
            }
        }
    }
}

impl Styled for Axis<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl Styled for Dataset<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl Styled for Chart<'_> {
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
    use alloc::string::ToString;
    use alloc::{format, vec};

    use ratatui_core::style::{Modifier, Stylize};
    use rstest::rstest;
    use strum::ParseError;

    use super::*;

    struct LegendTestCase {
        chart_area: Rect,
        hidden_legend_constraints: (Constraint, Constraint),
        legend_area: Option<Rect>,
    }

    #[test]
    fn it_should_hide_the_legend() {
        let data = [(0.0, 5.0), (1.0, 6.0), (3.0, 7.0)];
        let cases = [
            LegendTestCase {
                chart_area: Rect::new(0, 0, 100, 100),
                hidden_legend_constraints: (Constraint::Ratio(1, 4), Constraint::Ratio(1, 4)),
                legend_area: Some(Rect::new(88, 0, 12, 12)),
            },
            LegendTestCase {
                chart_area: Rect::new(0, 0, 100, 100),
                hidden_legend_constraints: (Constraint::Ratio(1, 10), Constraint::Ratio(1, 4)),
                legend_area: None,
            },
        ];
        for case in &cases {
            let datasets = (0..10)
                .map(|i| {
                    let name = format!("Dataset #{i}");
                    Dataset::default().name(name).data(&data)
                })
                .collect::<Vec<_>>();
            let chart = Chart::new(datasets)
                .x_axis(Axis::default().title("X axis"))
                .y_axis(Axis::default().title("Y axis"))
                .hidden_legend_constraints(case.hidden_legend_constraints);
            let layout = chart.layout(case.chart_area).unwrap();
            assert_eq!(layout.legend_area, case.legend_area);
        }
    }

    #[test]
    fn axis_can_be_stylized() {
        assert_eq!(
            Axis::default().black().on_white().bold().not_dim().style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn dataset_can_be_stylized() {
        assert_eq!(
            Dataset::default().black().on_white().bold().not_dim().style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn chart_can_be_stylized() {
        assert_eq!(
            Chart::new(vec![]).black().on_white().bold().not_dim().style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn graph_type_to_string() {
        assert_eq!(GraphType::Scatter.to_string(), "Scatter");
        assert_eq!(GraphType::Line.to_string(), "Line");
        assert_eq!(GraphType::Bar.to_string(), "Bar");
    }

    #[test]
    fn graph_type_from_str() {
        assert_eq!("Scatter".parse::<GraphType>(), Ok(GraphType::Scatter));
        assert_eq!("Line".parse::<GraphType>(), Ok(GraphType::Line));
        assert_eq!("Bar".parse::<GraphType>(), Ok(GraphType::Bar));
        assert_eq!("".parse::<GraphType>(), Err(ParseError::VariantNotFound));
    }

    #[test]
    fn it_does_not_panic_if_title_is_wider_than_buffer() {
        let widget = Chart::default()
            .y_axis(Axis::default().title("xxxxxxxxxxxxxxxx"))
            .x_axis(Axis::default().title("xxxxxxxxxxxxxxxx"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 4));
        widget.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(vec![" ".repeat(8); 4]));
    }

    #[test]
    fn datasets_without_name_dont_contribute_to_legend_height() {
        let data_named_1 = Dataset::default().name("data1"); // must occupy a row in legend
        let data_named_2 = Dataset::default().name(""); // must occupy a row in legend, even if name is empty
        let data_unnamed = Dataset::default(); // must not occupy a row in legend
        let widget = Chart::new(vec![data_named_1, data_unnamed, data_named_2]);
        let buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        let layout = widget.layout(buffer.area).unwrap();

        assert!(layout.legend_area.is_some());
        assert_eq!(layout.legend_area.unwrap().height, 4); // 2 for borders, 2 for rows
    }

    #[test]
    fn no_legend_if_no_named_datasets() {
        let dataset = Dataset::default();
        let widget = Chart::new(vec![dataset; 3]);
        let buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        let layout = widget.layout(buffer.area).unwrap();

        assert!(layout.legend_area.is_none());
    }

    #[test]
    fn dataset_legend_style_is_patched() {
        let long_dataset_name = Dataset::default().name("Very long name");
        let short_dataset =
            Dataset::default().name(Line::from("Short name").alignment(Alignment::Right));
        let widget = Chart::new(vec![long_dataset_name, short_dataset])
            .hidden_legend_constraints((100.into(), 100.into()));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 5));
        widget.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "    ┌──────────────┐",
            "    │Very long name│",
            "    │    Short name│",
            "    └──────────────┘",
            "                    ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_chart_have_a_topleft_legend() {
        let chart = Chart::new(vec![Dataset::default().name("Ds1")])
            .legend_position(Some(LegendPosition::TopLeft));
        let area = Rect::new(0, 0, 30, 20);
        let mut buffer = Buffer::empty(area);
        chart.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "┌───┐                         ",
            "│Ds1│                         ",
            "└───┘                         ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_chart_have_a_long_y_axis_title_overlapping_legend() {
        let chart = Chart::new(vec![Dataset::default().name("Ds1")])
            .y_axis(Axis::default().title("The title overlap a legend."));
        let area = Rect::new(0, 0, 30, 20);
        let mut buffer = Buffer::empty(area);
        chart.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "The title overlap a legend.   ",
            "                         ┌───┐",
            "                         │Ds1│",
            "                         └───┘",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_chart_have_overflowed_y_axis() {
        let chart = Chart::new(vec![Dataset::default().name("Ds1")])
            .y_axis(Axis::default().title("The title overlap a legend."));
        let area = Rect::new(0, 0, 10, 10);
        let mut buffer = Buffer::empty(area);
        chart.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_legend_area_can_fit_same_chart_area() {
        let name = "Data";
        let chart = Chart::new(vec![Dataset::default().name(name)])
            .hidden_legend_constraints((Constraint::Percentage(100), Constraint::Percentage(100)));
        let area = Rect::new(0, 0, name.len() as u16 + 2, 3);
        let mut buffer = Buffer::empty(area);
        for position in [
            LegendPosition::TopLeft,
            LegendPosition::Top,
            LegendPosition::TopRight,
            LegendPosition::Left,
            LegendPosition::Right,
            LegendPosition::Bottom,
            LegendPosition::BottomLeft,
            LegendPosition::BottomRight,
        ] {
            let chart = chart.clone().legend_position(Some(position));
            buffer.reset();
            chart.render(buffer.area, &mut buffer);
            #[rustfmt::skip]
            let expected = Buffer::with_lines([
                "┌────┐",
                "│Data│",
                "└────┘",
            ]);
            assert_eq!(buffer, expected);
        }
    }

    #[rstest]
    #[case(Some(LegendPosition::TopLeft), [
        "┌────┐   ",
        "│Data│   ",
        "└────┘   ",
        "         ",
        "         ",
        "         ",
    ])]
    #[case(Some(LegendPosition::Top), [
        " ┌────┐  ",
        " │Data│  ",
        " └────┘  ",
        "         ",
        "         ",
        "         ",
    ])]
    #[case(Some(LegendPosition::TopRight), [
        "   ┌────┐",
        "   │Data│",
        "   └────┘",
        "         ",
        "         ",
        "         ",
    ])]
    #[case(Some(LegendPosition::Left), [
        "         ",
        "┌────┐   ",
        "│Data│   ",
        "└────┘   ",
        "         ",
        "         ",
    ])]
    #[case(Some(LegendPosition::Right), [
        "         ",
        "   ┌────┐",
        "   │Data│",
        "   └────┘",
        "         ",
        "         ",
    ])]
    #[case(Some(LegendPosition::BottomLeft), [
        "         ",
        "         ",
        "         ",
        "┌────┐   ",
        "│Data│   ",
        "└────┘   ",
    ])]
    #[case(Some(LegendPosition::Bottom), [
        "         ",
        "         ",
        "         ",
        " ┌────┐  ",
        " │Data│  ",
        " └────┘  ",
    ])]
    #[case(Some(LegendPosition::BottomRight), [
        "         ",
        "         ",
        "         ",
        "   ┌────┐",
        "   │Data│",
        "   └────┘",
    ])]
    #[case(None, [
        "         ",
        "         ",
        "         ",
        "         ",
        "         ",
        "         ",
    ])]
    fn test_legend_of_chart_have_odd_margin_size<'line, Lines>(
        #[case] legend_position: Option<LegendPosition>,
        #[case] expected: Lines,
    ) where
        Lines: IntoIterator,
        Lines::Item: Into<Line<'line>>,
    {
        let name = "Data";
        let area = Rect::new(0, 0, name.len() as u16 + 2 + 3, 3 + 3);
        let mut buffer = Buffer::empty(area);
        let chart = Chart::new(vec![Dataset::default().name(name)])
            .legend_position(legend_position)
            .hidden_legend_constraints((Constraint::Percentage(100), Constraint::Percentage(100)));
        chart.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(expected));
    }

    #[test]
    fn bar_chart() {
        let data = [
            (0.0, 0.0),
            (2.0, 1.0),
            (4.0, 4.0),
            (6.0, 8.0),
            (8.0, 9.0),
            (10.0, 10.0),
        ];
        let chart = Chart::new(vec![
            Dataset::default()
                .data(&data)
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Bar),
        ])
        .x_axis(Axis::default().bounds([0.0, 10.0]))
        .y_axis(Axis::default().bounds([0.0, 10.0]));
        let area = Rect::new(0, 0, 11, 11);
        let mut buffer = Buffer::empty(area);
        chart.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "          •",
            "        • •",
            "      • • •",
            "      • • •",
            "      • • •",
            "      • • •",
            "    • • • •",
            "    • • • •",
            "    • • • •",
            "  • • • • •",
            "• • • • • •",
        ]);
        assert_eq!(buffer, expected);
    }

    #[rstest]
    #[case::dot(symbols::Marker::Dot, '•')]
    #[case::dot(symbols::Marker::Braille, '⢣')]
    fn overlapping_lines(#[case] marker: symbols::Marker, #[case] symbol: char) {
        let data_diagonal_up = [(0.0, 0.0), (5.0, 5.0)];
        let data_diagonal_down = [(0.0, 5.0), (5.0, 0.0)];
        let lines = vec![
            Dataset::default()
                .data(&data_diagonal_up)
                .marker(symbols::Marker::Block)
                .graph_type(GraphType::Line)
                .blue(),
            Dataset::default()
                .data(&data_diagonal_down)
                .marker(marker)
                .graph_type(GraphType::Line)
                .red(),
        ];
        let chart = Chart::new(lines)
            .x_axis(Axis::default().bounds([0.0, 5.0]))
            .y_axis(Axis::default().bounds([0.0, 5.0]));
        let area = Rect::new(0, 0, 5, 5);
        let mut buffer = Buffer::empty(area);
        chart.render(buffer.area, &mut buffer);
        #[rustfmt::skip]
        let mut expected = Buffer::with_lines([
            format!("{symbol}   █"),
            format!(" {symbol} █ "),
            format!("  {symbol}  "),
            format!(" █ {symbol} "),
            format!("█   {symbol}"),
        ]);
        for i in 0..5 {
            // The Marker::Dot and Marker::Braille tiles have the
            // foreground set to Red.
            expected.set_style(Rect::new(i, i, 1, 1), Style::new().fg(Color::Red));
            // The Marker::Block tiles have both the foreground and
            // background set to Blue.
            expected.set_style(
                Rect::new(i, 4 - i, 1, 1),
                Style::new().fg(Color::Blue).bg(Color::Blue),
            );
        }
        // Where the Marker::Dot/Braille overlaps with Marker::Block,
        // the background is set to blue from the Block, but the
        // foreground is set to red from the Dot/Braille.  This allows
        // two line plots to overlap, so long as one of them is a
        // Block.
        expected.set_style(
            Rect::new(2, 2, 1, 1),
            Style::new().fg(Color::Red).bg(Color::Blue),
        );

        assert_eq!(buffer, expected);
    }

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let chart = Chart::new(vec![Dataset::default().data(&[(0.0, 0.0), (1.0, 1.0)])])
            .x_axis(Axis::default().bounds([0.0, 1.0]))
            .y_axis(Axis::default().bounds([0.0, 1.0]));
        // This should not panic, even if the buffer is too small to render the chart.
        chart.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["•"]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let chart = Chart::new(vec![Dataset::default().data(&[(0.0, 0.0), (1.0, 1.0)])])
            .x_axis(Axis::default().bounds([0.0, 1.0]))
            .y_axis(Axis::default().bounds([0.0, 1.0]));
        // This should not panic, even if the buffer has zero size.
        chart.render(buffer.area, &mut buffer);
    }
}

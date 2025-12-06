use alloc::rc::Rc;
use alloc::vec::Vec;
use core::array::TryFromSliceError;
use core::iter;
#[cfg(feature = "layout-cache")]
use core::num::NonZeroUsize;

use hashbrown::HashMap;
use itertools::Itertools;
use kasuari::WeightedRelation::{EQ, GE, LE};
use kasuari::{AddConstraintError, Expression, Solver, Strength, Variable};
#[cfg(feature = "layout-cache")]
use lru::LruCache;

use self::strengths::{
    ALL_SEGMENT_GROW, FILL_GROW, GROW, LENGTH_SIZE_EQ, MAX_SIZE_EQ, MAX_SIZE_LE, MIN_SIZE_EQ,
    MIN_SIZE_GE, PERCENTAGE_SIZE_EQ, RATIO_SIZE_EQ, SPACE_GROW, SPACER_SIZE_EQ,
};
use crate::layout::{Constraint, Direction, Flex, Margin, Rect};

type Rects = Rc<[Rect]>;
type Segments = Rects;
type Spacers = Rects;
// The solution to a Layout solve contains two `Rects`, where `Rects` is effectively a `[Rect]`.
//
// 1. `[Rect]` that contains positions for the segments corresponding to user provided constraints
// 2. `[Rect]` that contains spacers around the user provided constraints
//
// <------------------------------------80 px------------------------------------->
// ┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐
//   1  │        a         │  2  │         b        │  3  │         c        │  4
// └   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘
//
// Number of spacers will always be one more than number of segments.
#[cfg(feature = "layout-cache")]
type Cache = LruCache<(Rect, Layout), (Segments, Spacers)>;

// Multiplier that decides floating point precision when rounding.
// The number of zeros in this number is the precision for the rounding of f64 to u16 in layout
// calculations.
const FLOAT_PRECISION_MULTIPLIER: f64 = 100.0;

#[cfg(feature = "layout-cache")]
std::thread_local! {
    static LAYOUT_CACHE: core::cell::RefCell<Cache> = core::cell::RefCell::new(Cache::new(
        NonZeroUsize::new(Layout::DEFAULT_CACHE_SIZE).unwrap(),
    ));
}

/// Represents the spacing between segments in a layout.
///
/// The `Spacing` enum is used to define the spacing between segments in a layout. It can represent
/// either positive spacing (space between segments) or negative spacing (overlap between segments).
///
/// # Variants
///
/// - `Space(u16)`: Represents positive spacing between segments. The value indicates the number of
///   cells.
/// - `Overlap(u16)`: Represents negative spacing, causing overlap between segments. The value
///   indicates the number of overlapping cells.
///
/// # Default
///
/// The default value for `Spacing` is `Space(0)`, which means no spacing or no overlap between
/// segments.
///
/// # Conversions
///
/// The `Spacing` enum can be created from different integer types:
///
/// - From `u16`: Directly converts the value to `Spacing::Space`.
/// - From `i16`: Converts negative values to `Spacing::Overlap` and non-negative values to
///   `Spacing::Space`.
/// - From `i32`: Clamps the value to the range of `i16` and converts negative values to
///   `Spacing::Overlap` and non-negative values to `Spacing::Space`.
///
/// See the [`Layout::spacing`] method for details on how to use this enum.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Spacing {
    Space(u16),
    Overlap(u16),
}

impl Default for Spacing {
    fn default() -> Self {
        Self::Space(0)
    }
}

impl From<i32> for Spacing {
    fn from(value: i32) -> Self {
        Self::from(value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
    }
}

impl From<u16> for Spacing {
    fn from(value: u16) -> Self {
        Self::Space(value)
    }
}

impl From<i16> for Spacing {
    fn from(value: i16) -> Self {
        if value < 0 {
            Self::Overlap(value.unsigned_abs())
        } else {
            Self::Space(value.unsigned_abs())
        }
    }
}

/// The primary layout engine for dividing terminal space using constraints and direction.
///
/// A layout is a set of constraints that can be applied to a given area to split it into smaller
/// rectangular areas. This is the core building block for creating structured user interfaces in
/// terminal applications.
///
/// A layout is composed of:
/// - a direction (horizontal or vertical)
/// - a set of constraints (length, ratio, percentage, fill, min, max)
/// - a margin (horizontal and vertical), the space between the edge of the main area and the split
///   areas
/// - a flex option that controls space distribution
/// - a spacing option that controls gaps between segments
///
/// The algorithm used to compute the layout is based on the [`kasuari`] solver, a linear constraint
/// solver that computes positions and sizes to satisfy as many constraints as possible in order of
/// their priorities.
///
/// When the layout is computed, the result is cached in a thread-local cache, so that subsequent
/// calls with the same parameters are faster. The cache is a `LruCache`, and the size of the cache
/// can be configured using [`Layout::init_cache()`] when the `layout-cache` feature is enabled.
///
/// # Construction
///
/// - [`default`](Default::default) - Create a layout with default values (vertical direction, no
///   constraints, no margin)
/// - [`new`](Self::new) - Create a new layout with a given direction and constraints
/// - [`vertical`](Self::vertical) - Create a new vertical layout with the given constraints
/// - [`horizontal`](Self::horizontal) - Create a new horizontal layout with the given constraints
///
/// # Configuration
///
/// - [`direction`](Self::direction) - Set the direction of the layout
/// - [`constraints`](Self::constraints) - Set the constraints of the layout
/// - [`margin`](Self::margin) - Set uniform margin on all sides
/// - [`horizontal_margin`](Self::horizontal_margin) - Set the horizontal margin of the layout
/// - [`vertical_margin`](Self::vertical_margin) - Set the vertical margin of the layout
/// - [`flex`](Self::flex) - Set the way space is distributed when constraints are satisfied
/// - [`spacing`](Self::spacing) - Set the gap between the constraints of the layout
///
/// # Layout Operations
///
/// - [`areas`](Self::areas) - Split area into fixed number of rectangles (compile-time known)
/// - [`spacers`](Self::spacers) - Get spacer rectangles between layout areas
/// - [`split`](Self::split) - Split area into rectangles (runtime determined count)
/// - [`split_with_spacers`](Self::split_with_spacers) - Split area and return both areas and
///   spacers
///
/// # Cache Management
///
/// - [`init_cache`](Self::init_cache) - Initialize layout cache with custom size
///
/// # Example
///
/// ```rust
/// use ratatui_core::buffer::Buffer;
/// use ratatui_core::layout::{Constraint, Direction, Layout, Rect};
/// use ratatui_core::text::Text;
/// use ratatui_core::widgets::Widget;
///
/// fn render(area: Rect, buf: &mut Buffer) {
///     let layout = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]);
///     let [top, bottom] = layout.areas(area);
///     Text::from("foo").render(top, buf);
///     Text::from("bar").render(bottom, buf);
/// }
/// ```
///
/// See the `layout`, `flex`, and `constraints` examples in the [Examples] folder for more details
/// about how to use layouts.
///
/// For comprehensive layout documentation and examples, see the [`layout`](crate::layout) module.
///
/// ![layout
/// example](https://camo.githubusercontent.com/77d22f3313b782a81e5e033ef82814bb48d786d2598699c27f8e757ccee62021/68747470733a2f2f7668732e636861726d2e73682f7668732d315a4e6f4e4c4e6c4c746b4a58706767396e435635652e676966)
///
/// [`kasuari`]: https://crates.io/crates/kasuari
/// [Examples]: https://github.com/ratatui/ratatui/blob/main/examples/README.md
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Layout {
    direction: Direction,
    constraints: Vec<Constraint>,
    margin: Margin,
    flex: Flex,
    spacing: Spacing,
}

impl Layout {
    /// This is a somewhat arbitrary size for the layout cache based on adding the columns and rows
    /// on my laptop's terminal (171+51 = 222) and doubling it for good measure and then adding a
    /// bit more to make it a round number. This gives enough entries to store a layout for every
    /// row and every column, twice over, which should be enough for most apps. For those that need
    /// more, the cache size can be set with `Layout::init_cache()` (requires the `layout-cache`
    /// feature).
    #[cfg(feature = "layout-cache")]
    pub const DEFAULT_CACHE_SIZE: usize = 500;

    /// Creates a new layout with default values.
    ///
    /// The `constraints` parameter accepts any type that implements `IntoIterator<Item =
    /// Into<Constraint>>`. This includes arrays, slices, vectors, iterators. `Into<Constraint>` is
    /// implemented on `u16`, so you can pass an array, `Vec`, etc. of `u16` to this function to
    /// create a layout with fixed size chunks.
    ///
    /// Default values for the other fields are:
    ///
    /// - `margin`: 0, 0
    /// - `flex`: [`Flex::Start`]
    /// - `spacing`: 0
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Direction, Layout};
    ///
    /// Layout::new(
    ///     Direction::Horizontal,
    ///     [Constraint::Length(5), Constraint::Fill(1)],
    /// );
    ///
    /// Layout::new(
    ///     Direction::Vertical,
    ///     [1, 2, 3].iter().map(|&c| Constraint::Length(c)),
    /// );
    ///
    /// Layout::new(Direction::Horizontal, vec![1, 2]);
    /// ```
    pub fn new<I>(direction: Direction, constraints: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        Self {
            direction,
            constraints: constraints.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    /// Creates a new vertical layout with default values.
    ///
    /// The `constraints` parameter accepts any type that implements `IntoIterator<Item =
    /// Into<Constraint>>`. This includes arrays, slices, vectors, iterators, etc.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout};
    ///
    /// let layout = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]);
    /// ```
    pub fn vertical<I>(constraints: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        Self::new(Direction::Vertical, constraints.into_iter().map(Into::into))
    }

    /// Creates a new horizontal layout with default values.
    ///
    /// The `constraints` parameter accepts any type that implements `IntoIterator<Item =
    /// Into<Constraint>>`. This includes arrays, slices, vectors, iterators, etc.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout};
    ///
    /// let layout = Layout::horizontal([Constraint::Length(5), Constraint::Fill(1)]);
    /// ```
    pub fn horizontal<I>(constraints: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        Self::new(
            Direction::Horizontal,
            constraints.into_iter().map(Into::into),
        )
    }

    /// Initialize an empty cache with a custom size. The cache is keyed on the layout and area, so
    /// that subsequent calls with the same parameters are faster. The cache is a `LruCache`, and
    /// grows until `cache_size` is reached.
    ///
    /// By default, the cache size is [`Self::DEFAULT_CACHE_SIZE`].
    #[cfg(feature = "layout-cache")]
    pub fn init_cache(cache_size: NonZeroUsize) {
        LAYOUT_CACHE.with_borrow_mut(|cache| cache.resize(cache_size));
    }

    /// Set the direction of the layout.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Direction, Layout, Rect};
    ///
    /// let layout = Layout::default()
    ///     .direction(Direction::Horizontal)
    ///     .constraints([Constraint::Length(5), Constraint::Fill(1)])
    ///     .split(Rect::new(0, 0, 10, 10));
    /// assert_eq!(layout[..], [Rect::new(0, 0, 5, 10), Rect::new(5, 0, 5, 10)]);
    ///
    /// let layout = Layout::default()
    ///     .direction(Direction::Vertical)
    ///     .constraints([Constraint::Length(5), Constraint::Fill(1)])
    ///     .split(Rect::new(0, 0, 10, 10));
    /// assert_eq!(layout[..], [Rect::new(0, 0, 10, 5), Rect::new(0, 5, 10, 5)]);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Sets the constraints of the layout.
    ///
    /// The `constraints` parameter accepts any type that implements `IntoIterator<Item =
    /// Into<Constraint>>`. This includes arrays, slices, vectors, iterators. `Into<Constraint>` is
    /// implemented on u16, so you can pass an array or vec of u16 to this function to create a
    /// layout with fixed size chunks.
    ///
    /// Note that the constraints are applied to the whole area that is to be split, so using
    /// percentages and ratios with the other constraints may not have the desired effect of
    /// splitting the area up. (e.g. splitting 100 into [min 20, 50%, 50%], may not result in [20,
    /// 40, 40] but rather an indeterminate result between [20, 50, 30] and [20, 30, 50]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let layout = Layout::default()
    ///     .constraints([
    ///         Constraint::Percentage(20),
    ///         Constraint::Ratio(1, 5),
    ///         Constraint::Length(2),
    ///         Constraint::Min(2),
    ///         Constraint::Max(2),
    ///     ])
    ///     .split(Rect::new(0, 0, 10, 10));
    /// assert_eq!(
    ///     layout[..],
    ///     [
    ///         Rect::new(0, 0, 10, 2),
    ///         Rect::new(0, 2, 10, 2),
    ///         Rect::new(0, 4, 10, 2),
    ///         Rect::new(0, 6, 10, 2),
    ///         Rect::new(0, 8, 10, 2),
    ///     ]
    /// );
    ///
    /// Layout::default().constraints([Constraint::Fill(1)]);
    /// Layout::default().constraints(&[Constraint::Fill(1)]);
    /// Layout::default().constraints(vec![Constraint::Fill(1)]);
    /// Layout::default().constraints([Constraint::Fill(1)].iter().filter(|_| true));
    /// Layout::default().constraints([1, 2, 3].iter().map(|&c| Constraint::Length(c)));
    /// Layout::default().constraints([1, 2, 3]);
    /// Layout::default().constraints(vec![1, 2, 3]);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn constraints<I>(mut self, constraints: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        self.constraints = constraints.into_iter().map(Into::into).collect();
        self
    }

    /// Set the margin of the layout.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let layout = Layout::default()
    ///     .constraints([Constraint::Fill(1)])
    ///     .margin(2)
    ///     .split(Rect::new(0, 0, 10, 10));
    /// assert_eq!(layout[..], [Rect::new(2, 2, 6, 6)]);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn margin(mut self, margin: u16) -> Self {
        self.margin = Margin {
            horizontal: margin,
            vertical: margin,
        };
        self
    }

    /// Set the horizontal margin of the layout.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let layout = Layout::default()
    ///     .constraints([Constraint::Fill(1)])
    ///     .horizontal_margin(2)
    ///     .split(Rect::new(0, 0, 10, 10));
    /// assert_eq!(layout[..], [Rect::new(2, 0, 6, 10)]);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn horizontal_margin(mut self, horizontal: u16) -> Self {
        self.margin.horizontal = horizontal;
        self
    }

    /// Set the vertical margin of the layout.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let layout = Layout::default()
    ///     .constraints([Constraint::Fill(1)])
    ///     .vertical_margin(2)
    ///     .split(Rect::new(0, 0, 10, 10));
    /// assert_eq!(layout[..], [Rect::new(0, 2, 10, 6)]);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn vertical_margin(mut self, vertical: u16) -> Self {
        self.margin.vertical = vertical;
        self
    }

    /// The `flex` method  allows you to specify the flex behavior of the layout.
    ///
    /// # Arguments
    ///
    /// * `flex`: A [`Flex`] enum value that represents the flex behavior of the layout. It can be
    ///   one of the following:
    ///   - [`Flex::Legacy`]: The last item is stretched to fill the excess space.
    ///   - [`Flex::Start`]: The items are aligned to the start of the layout.
    ///   - [`Flex::Center`]: The items are aligned to the center of the layout.
    ///   - [`Flex::End`]: The items are aligned to the end of the layout.
    ///   - [`Flex::SpaceBetween`]: The items are evenly distributed with equal space between them.
    ///   - [`Flex::SpaceAround`]: The items are evenly distributed with equal space around them,
    ///     except the first and last items, which have half the space on their sides.
    ///   - [`Flex::SpaceEvenly`]: The items are evenly distributed with equal space around them.
    ///
    /// # Examples
    ///
    /// In this example, the items in the layout will be aligned to the start.
    ///
    /// ```rust
    /// use ratatui_core::layout::Constraint::*;
    /// use ratatui_core::layout::{Flex, Layout};
    ///
    /// let layout = Layout::horizontal([Length(20), Length(20), Length(20)]).flex(Flex::Start);
    /// ```
    ///
    /// In this example, the items in the layout will be stretched equally to fill the available
    /// space.
    ///
    /// ```rust
    /// use ratatui_core::layout::Constraint::*;
    /// use ratatui_core::layout::{Flex, Layout};
    ///
    /// let layout = Layout::horizontal([Length(20), Length(20), Length(20)]).flex(Flex::Legacy);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn flex(mut self, flex: Flex) -> Self {
        self.flex = flex;
        self
    }

    /// Sets the spacing between items in the layout.
    ///
    /// The `spacing` method sets the spacing between items in the layout. The spacing is applied
    /// evenly between all segments. The spacing value represents the number of cells between each
    /// item.
    ///
    /// Spacing can be positive integers, representing gaps between segments; or negative integers
    /// representing overlaps. Additionally, one of the variants of the [`Spacing`] enum can be
    /// passed to this function. See the documentation of the [`Spacing`] enum for more information.
    ///
    /// Note that if the layout has only one segment, the spacing will not be applied.
    /// Also, spacing will not be applied for [`Flex::SpaceAround`], [`Flex::SpaceEvenly`] and
    /// [`Flex::SpaceBetween`]
    ///
    /// # Examples
    ///
    /// In this example, the spacing between each item in the layout is set to 2 cells.
    ///
    /// ```rust
    /// use ratatui_core::layout::Constraint::*;
    /// use ratatui_core::layout::Layout;
    ///
    /// let layout = Layout::horizontal([Length(20), Length(20), Length(20)]).spacing(2);
    /// ```
    ///
    /// In this example, the spacing between each item in the layout is set to -1 cells, i.e. the
    /// three segments will have an overlapping border.
    ///
    /// ```rust
    /// use ratatui_core::layout::Constraint::*;
    /// use ratatui_core::layout::Layout;
    /// let layout = Layout::horizontal([Length(20), Length(20), Length(20)]).spacing(-1);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn spacing<T>(mut self, spacing: T) -> Self
    where
        T: Into<Spacing>,
    {
        self.spacing = spacing.into();
        self
    }

    /// Split the rect into a number of sub-rects according to the given [`Layout`].
    ///
    /// An ergonomic wrapper around [`Layout::split`] that returns an array of `Rect`s instead of
    /// `Rc<[Rect]>`.
    ///
    /// This method requires the number of constraints to be known at compile time. If you don't
    /// know the number of constraints at compile time, use [`Layout::split`] instead.
    ///
    /// # Panics
    ///
    /// Panics if the number of constraints is not equal to the length of the returned array.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let area = Rect::new(0, 0, 10, 10);
    /// let layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
    /// let [top, main] = layout.areas(area);
    ///
    /// // or explicitly specify the number of constraints:
    /// let areas = layout.areas::<2>(area);
    /// ```
    pub fn areas<const N: usize>(&self, area: Rect) -> [Rect; N] {
        let areas = self.split(area);
        areas.as_ref().try_into().unwrap_or_else(|_| {
            panic!(
                "invalid number of rects: expected {N}, found {}",
                areas.len()
            )
        })
    }

    /// Split the rect into a number of sub-rects according to the given [`Layout`].
    ///
    /// An ergonomic wrapper around [`Layout::split`] that returns an array of `Rect`s instead of
    /// `Rc<[Rect]>`.
    ///
    /// This method requires the number of constraints to be known at compile time. If you don't
    /// know the number of constraints at compile time, use [`Layout::split`] instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the number of constraints is not equal to the length of the returned
    /// array.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let area = Rect::new(0, 0, 10, 10);
    /// let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
    /// let [top, main] = layout.try_areas(area)?;
    ///
    /// // or explicitly specify the number of constraints:
    /// let areas = layout.try_areas::<2>(area)?;
    /// # Ok::<(), core::array::TryFromSliceError>(())
    /// ```
    pub fn try_areas<const N: usize>(&self, area: Rect) -> Result<[Rect; N], TryFromSliceError> {
        self.split(area).as_ref().try_into()
    }

    /// Split the rect into a number of sub-rects according to the given [`Layout`] and return just
    /// the spacers between the areas.
    ///
    /// This method requires the number of constraints to be known at compile time. If you don't
    /// know the number of constraints at compile time, use [`Layout::split_with_spacers`] instead.
    ///
    /// This method is similar to [`Layout::areas`], and can be called with the same parameters, but
    /// it returns just the spacers between the areas. The result of calling the `areas` method is
    /// cached, so this will generally not re-run the solver, but will just return the cached
    /// result.
    ///
    /// # Panics
    ///
    /// Panics if the number of constraints + 1 is not equal to the length of the returned array.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_core::layout::{Constraint, Layout, Rect};
    ///
    /// let area = Rect::new(0, 0, 10, 10);
    /// let layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
    /// let [top, main] = layout.areas(area);
    /// let [before, inbetween, after] = layout.spacers(area);
    ///
    /// // or explicitly specify the number of constraints:
    /// let spacers = layout.spacers::<3>(area);
    /// ```
    pub fn spacers<const N: usize>(&self, area: Rect) -> [Rect; N] {
        let (_, spacers) = self.split_with_spacers(area);
        spacers
            .as_ref()
            .try_into()
            .expect("invalid number of rects")
    }

    /// Wrapper function around the [`kasuari`] solver to be able to split a given area into
    /// smaller ones based on the preferred widths or heights and the direction.
    ///
    /// Note that the constraints are applied to the whole area that is to be split, so using
    /// percentages and ratios with the other constraints may not have the desired effect of
    /// splitting the area up. (e.g. splitting 100 into [min 20, 50%, 50%], may not result in [20,
    /// 40, 40] but rather an indeterminate result between [20, 50, 30] and [20, 30, 50]).
    ///
    /// This method stores the result of the computation in a thread-local cache keyed on the layout
    /// and area, so that subsequent calls with the same parameters are faster. The cache is a
    /// `LruCache`, and grows until [`Self::DEFAULT_CACHE_SIZE`] is reached by default. If the cache
    /// is initialized with [`Layout::init_cache()`], it grows until the initialized cache size.
    ///
    /// There is a helper method that can be used to split the whole area into smaller ones based on
    /// the layout: [`Layout::areas()`]. That method is a shortcut for calling this method. It
    /// allows you to destructure the result directly into variables, which is useful when you know
    /// at compile time the number of areas that will be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui_core::layout::{Constraint, Direction, Layout, Rect};
    /// let layout = Layout::default()
    ///     .direction(Direction::Vertical)
    ///     .constraints([Constraint::Length(5), Constraint::Fill(1)])
    ///     .split(Rect::new(2, 2, 10, 10));
    /// assert_eq!(layout[..], [Rect::new(2, 2, 10, 5), Rect::new(2, 7, 10, 5)]);
    ///
    /// let layout = Layout::default()
    ///     .direction(Direction::Horizontal)
    ///     .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)])
    ///     .split(Rect::new(0, 0, 9, 2));
    /// assert_eq!(layout[..], [Rect::new(0, 0, 3, 2), Rect::new(3, 0, 6, 2)]);
    /// ```
    pub fn split(&self, area: Rect) -> Rects {
        self.split_with_spacers(area).0
    }

    /// Wrapper function around the [`kasuari`] solver that splits the given area into smaller ones
    /// based on the preferred widths or heights and the direction, with the ability to include
    /// spacers between the areas.
    ///
    /// This method is similar to `split`, but it returns two sets of rectangles: one for the areas
    /// and one for the spacers.
    ///
    /// This method stores the result of the computation in a thread-local cache keyed on the layout
    /// and area, so that subsequent calls with the same parameters are faster. The cache is a
    /// `LruCache`, and grows until [`Self::DEFAULT_CACHE_SIZE`] is reached by default. If the cache
    /// is initialized with [`Layout::init_cache()`], it grows until the initialized cache size.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui_core::layout::{Constraint, Direction, Layout, Rect};
    ///
    /// let (areas, spacers) = Layout::default()
    ///     .direction(Direction::Vertical)
    ///     .constraints([Constraint::Length(5), Constraint::Fill(1)])
    ///     .split_with_spacers(Rect::new(2, 2, 10, 10));
    /// assert_eq!(areas[..], [Rect::new(2, 2, 10, 5), Rect::new(2, 7, 10, 5)]);
    /// assert_eq!(
    ///     spacers[..],
    ///     [
    ///         Rect::new(2, 2, 10, 0),
    ///         Rect::new(2, 7, 10, 0),
    ///         Rect::new(2, 12, 10, 0)
    ///     ]
    /// );
    ///
    /// let (areas, spacers) = Layout::default()
    ///     .direction(Direction::Horizontal)
    ///     .spacing(1)
    ///     .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)])
    ///     .split_with_spacers(Rect::new(0, 0, 10, 2));
    /// assert_eq!(areas[..], [Rect::new(0, 0, 3, 2), Rect::new(4, 0, 6, 2)]);
    /// assert_eq!(
    ///     spacers[..],
    ///     [
    ///         Rect::new(0, 0, 0, 2),
    ///         Rect::new(3, 0, 1, 2),
    ///         Rect::new(10, 0, 0, 2)
    ///     ]
    /// );
    /// ```
    pub fn split_with_spacers(&self, area: Rect) -> (Segments, Spacers) {
        let split = || self.try_split(area).expect("failed to split");

        #[cfg(feature = "layout-cache")]
        {
            LAYOUT_CACHE.with_borrow_mut(|cache| {
                let key = (area, self.clone());
                cache.get_or_insert(key, split).clone()
            })
        }

        #[cfg(not(feature = "layout-cache"))]
        split()
    }

    fn try_split(&self, area: Rect) -> Result<(Segments, Spacers), AddConstraintError> {
        // To take advantage of all of [`kasuari`] features, we would want to store the `Solver` in
        // one of the fields of the Layout struct. And we would want to set it up such that we could
        // add or remove constraints as and when needed.
        // The advantage of doing it as described above is that it would allow users to
        // incrementally add and remove constraints efficiently.
        // Solves will just one constraint different would not need to resolve the entire layout.
        //
        // The disadvantage of this approach is that it requires tracking which constraints were
        // added, and which variables they correspond to.
        // This will also require introducing and maintaining the API for users to do so.
        //
        // Currently we don't support that use case and do not intend to support it in the future,
        // and instead we require that the user re-solve the layout every time they call `split`.
        // To minimize the time it takes to solve the same problem over and over again, we
        // cache the `Layout` struct along with the results.
        //
        // `try_split` is the inner method in `split` that is called only when the LRU cache doesn't
        // match the key. So inside `try_split`, we create a new instance of the solver.
        //
        // This is equivalent to storing the solver in `Layout` and calling `solver.reset()` here.
        let mut solver = Solver::new();

        let inner_area = area.inner(self.margin);
        let (area_start, area_end) = match self.direction {
            Direction::Horizontal => (
                f64::from(inner_area.x) * FLOAT_PRECISION_MULTIPLIER,
                f64::from(inner_area.right()) * FLOAT_PRECISION_MULTIPLIER,
            ),
            Direction::Vertical => (
                f64::from(inner_area.y) * FLOAT_PRECISION_MULTIPLIER,
                f64::from(inner_area.bottom()) * FLOAT_PRECISION_MULTIPLIER,
            ),
        };

        // ```plain
        // <───────────────────────────────────area_size──────────────────────────────────>
        // ┌─area_start                                                          area_end─┐
        // V                                                                              V
        // ┌────┬───────────────────┬────┬─────variables─────┬────┬───────────────────┬────┐
        // │    │                   │    │                   │    │                   │    │
        // V    V                   V    V                   V    V                   V    V
        // ┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐
        //      │     Max(20)      │     │      Max(20)     │     │      Max(20)     │
        // └   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘
        // ^    ^                   ^    ^                   ^    ^                   ^    ^
        // │    │                   │    │                   │    │                   │    │
        // └─┬──┶━━━━━━━━━┳━━━━━━━━━┵─┬──┶━━━━━━━━━┳━━━━━━━━━┵─┬──┶━━━━━━━━━┳━━━━━━━━━┵─┬──┘
        //   │            ┃           │            ┃           │            ┃           │
        //   └────────────╂───────────┴────────────╂───────────┴────────────╂──Spacers──┘
        //                ┃                        ┃                        ┃
        //                ┗━━━━━━━━━━━━━━━━━━━━━━━━┻━━━━━━━━Segments━━━━━━━━┛
        // ```

        let variable_count = self.constraints.len() * 2 + 2;
        let variables = iter::repeat_with(Variable::new)
            .take(variable_count)
            .collect_vec();
        let spacers = variables
            .iter()
            .tuples()
            .map(|(a, b)| Element::from((*a, *b)))
            .collect_vec();
        let segments = variables
            .iter()
            .skip(1)
            .tuples()
            .map(|(a, b)| Element::from((*a, *b)))
            .collect_vec();

        let flex = self.flex;

        let spacing = match self.spacing {
            Spacing::Space(x) => x as i16,
            Spacing::Overlap(x) => -(x as i16),
        };

        let constraints = &self.constraints;

        let area_size = Element::from((*variables.first().unwrap(), *variables.last().unwrap()));
        configure_area(&mut solver, area_size, area_start, area_end)?;
        configure_variable_in_area_constraints(&mut solver, &variables, area_size)?;
        configure_variable_constraints(&mut solver, &variables)?;
        configure_flex_constraints(&mut solver, area_size, &spacers, flex, spacing)?;
        configure_constraints(&mut solver, area_size, &segments, constraints, flex)?;
        configure_fill_constraints(&mut solver, &segments, constraints, flex)?;

        if !flex.is_legacy() {
            for (left, right) in segments.iter().tuple_windows() {
                solver.add_constraint(left.has_size(right, ALL_SEGMENT_GROW))?;
            }
        }

        // `solver.fetch_changes()` can only be called once per solve
        let changes: HashMap<Variable, f64> = solver.fetch_changes().iter().copied().collect();
        // debug_elements(&segments, &changes);
        // debug_elements(&spacers, &changes);

        let segment_rects = changes_to_rects(&changes, &segments, inner_area, self.direction);
        let spacer_rects = changes_to_rects(&changes, &spacers, inner_area, self.direction);

        Ok((segment_rects, spacer_rects))
    }
}

fn configure_area(
    solver: &mut Solver,
    area: Element,
    area_start: f64,
    area_end: f64,
) -> Result<(), AddConstraintError> {
    solver.add_constraint(area.start | EQ(Strength::REQUIRED) | area_start)?;
    solver.add_constraint(area.end | EQ(Strength::REQUIRED) | area_end)?;
    Ok(())
}

fn configure_variable_in_area_constraints(
    solver: &mut Solver,
    variables: &[Variable],
    area: Element,
) -> Result<(), AddConstraintError> {
    // all variables are in the range [area.start, area.end]
    for &variable in variables {
        solver.add_constraint(variable | GE(Strength::REQUIRED) | area.start)?;
        solver.add_constraint(variable | LE(Strength::REQUIRED) | area.end)?;
    }

    Ok(())
}

fn configure_variable_constraints(
    solver: &mut Solver,
    variables: &[Variable],
) -> Result<(), AddConstraintError> {
    // ┌────┬───────────────────┬────┬─────variables─────┬────┬───────────────────┬────┐
    // │    │                   │    │                   │    │                   │    │
    // v    v                   v    v                   v    v                   v    v
    // ┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐
    //      │     Max(20)      │     │      Max(20)     │     │      Max(20)     │
    // └   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘
    // ^    ^                   ^    ^                   ^    ^                   ^    ^
    // └v0  └v1                 └v2  └v3                 └v4  └v5                 └v6  └v7

    for (&left, &right) in variables.iter().skip(1).tuples() {
        solver.add_constraint(left | LE(Strength::REQUIRED) | right)?;
    }
    Ok(())
}

fn configure_constraints(
    solver: &mut Solver,
    area: Element,
    segments: &[Element],
    constraints: &[Constraint],
    flex: Flex,
) -> Result<(), AddConstraintError> {
    for (&constraint, &segment) in constraints.iter().zip(segments.iter()) {
        match constraint {
            Constraint::Max(max) => {
                solver.add_constraint(segment.has_max_size(max, MAX_SIZE_LE))?;
                solver.add_constraint(segment.has_int_size(max, MAX_SIZE_EQ))?;
            }
            Constraint::Min(min) => {
                solver.add_constraint(segment.has_min_size(min as i16, MIN_SIZE_GE))?;
                if flex.is_legacy() {
                    solver.add_constraint(segment.has_int_size(min, MIN_SIZE_EQ))?;
                } else {
                    solver.add_constraint(segment.has_size(area, FILL_GROW))?;
                }
            }
            Constraint::Length(length) => {
                solver.add_constraint(segment.has_int_size(length, LENGTH_SIZE_EQ))?;
            }
            Constraint::Percentage(p) => {
                let size = area.size() * f64::from(p) / 100.00;
                solver.add_constraint(segment.has_size(size, PERCENTAGE_SIZE_EQ))?;
            }
            Constraint::Ratio(num, den) => {
                // avoid division by zero by using 1 when denominator is 0
                let size = area.size() * f64::from(num) / f64::from(den.max(1));
                solver.add_constraint(segment.has_size(size, RATIO_SIZE_EQ))?;
            }
            Constraint::Fill(_) => {
                // given no other constraints, this segment will grow as much as possible.
                solver.add_constraint(segment.has_size(area, FILL_GROW))?;
            }
        }
    }
    Ok(())
}

fn configure_flex_constraints(
    solver: &mut Solver,
    area: Element,
    spacers: &[Element],
    flex: Flex,
    spacing: i16,
) -> Result<(), AddConstraintError> {
    let spacers_except_first_and_last = spacers.get(1..spacers.len() - 1).unwrap_or(&[]);
    let spacing_f64 = f64::from(spacing) * FLOAT_PRECISION_MULTIPLIER;
    match flex {
        Flex::Legacy => {
            for spacer in spacers_except_first_and_last {
                solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
            }
            if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
                solver.add_constraint(first.is_empty())?;
                solver.add_constraint(last.is_empty())?;
            }
        }

        // All spacers excluding first and last are the same size and will grow to fill
        // any remaining space after the constraints are satisfied.
        // All spacers excluding first and last are also twice the size of the first and last
        // spacers
        Flex::SpaceAround => {
            if spacers.len() <= 2 {
                // If there are two or less spacers, fallback to Flex::SpaceEvenly
                for (left, right) in spacers.iter().tuple_combinations() {
                    solver.add_constraint(left.has_size(right, SPACER_SIZE_EQ))?;
                }
                for spacer in spacers {
                    solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
                    solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
                }
            } else {
                // Separate the first and last spacer from the middle ones
                let (first, rest) = spacers.split_first().unwrap();
                let (last, middle) = rest.split_last().unwrap();

                // All middle spacers should be equal in size
                for (left, right) in middle.iter().tuple_combinations() {
                    solver.add_constraint(left.has_size(right, SPACER_SIZE_EQ))?;
                }

                // First and last spacers should be half the size of any middle spacer
                if let Some(first_middle) = middle.first() {
                    solver.add_constraint(first_middle.has_double_size(first, SPACER_SIZE_EQ))?;
                    solver.add_constraint(first_middle.has_double_size(last, SPACER_SIZE_EQ))?;
                }

                // Apply minimum size and growth constraints
                for spacer in spacers {
                    solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
                    solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
                }
            }
        }

        // All spacers are the same size and will grow to fill any remaining space after the
        // constraints are satisfied
        Flex::SpaceEvenly => {
            for (left, right) in spacers.iter().tuple_combinations() {
                solver.add_constraint(left.has_size(right, SPACER_SIZE_EQ))?;
            }
            for spacer in spacers {
                solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
                solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
            }
        }

        // All spacers excluding first and last are the same size and will grow to fill
        // any remaining space after the constraints are satisfied.
        // The first and last spacers are zero size.
        Flex::SpaceBetween => {
            for (left, right) in spacers_except_first_and_last.iter().tuple_combinations() {
                solver.add_constraint(left.has_size(right.size(), SPACER_SIZE_EQ))?;
            }
            for spacer in spacers_except_first_and_last {
                solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
                solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
            }
            if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
                solver.add_constraint(first.is_empty())?;
                solver.add_constraint(last.is_empty())?;
            }
        }

        Flex::Start => {
            for spacer in spacers_except_first_and_last {
                solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
            }
            if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
                solver.add_constraint(first.is_empty())?;
                solver.add_constraint(last.has_size(area, GROW))?;
            }
        }
        Flex::Center => {
            for spacer in spacers_except_first_and_last {
                solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
            }
            if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
                solver.add_constraint(first.has_size(area, GROW))?;
                solver.add_constraint(last.has_size(area, GROW))?;
                solver.add_constraint(first.has_size(last, SPACER_SIZE_EQ))?;
            }
        }
        Flex::End => {
            for spacer in spacers_except_first_and_last {
                solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
            }
            if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
                solver.add_constraint(last.is_empty())?;
                solver.add_constraint(first.has_size(area, GROW))?;
            }
        }
    }
    Ok(())
}

/// Make every `Fill` constraint proportionally equal to each other
/// This will make it fill up empty spaces equally
///
/// [Fill(1), Fill(1)]
/// ┌──────┐┌──────┐
/// │abcdef││abcdef│
/// └──────┘└──────┘
///
/// [Fill(1), Fill(2)]
/// ┌──────┐┌────────────┐
/// │abcdef││abcdefabcdef│
/// └──────┘└────────────┘
///
/// `size == base_element * scaling_factor`
fn configure_fill_constraints(
    solver: &mut Solver,
    segments: &[Element],
    constraints: &[Constraint],
    flex: Flex,
) -> Result<(), AddConstraintError> {
    for ((&left_constraint, &left_segment), (&right_constraint, &right_segment)) in constraints
        .iter()
        .zip(segments.iter())
        .filter(|(c, _)| c.is_fill() || (!flex.is_legacy() && c.is_min()))
        .tuple_combinations()
    {
        let left_scaling_factor = match left_constraint {
            Constraint::Fill(scale) => f64::from(scale).max(1e-6),
            Constraint::Min(_) => 1.0,
            _ => unreachable!(),
        };
        let right_scaling_factor = match right_constraint {
            Constraint::Fill(scale) => f64::from(scale).max(1e-6),
            Constraint::Min(_) => 1.0,
            _ => unreachable!(),
        };
        solver.add_constraint(
            (right_scaling_factor * left_segment.size())
                | EQ(GROW)
                | (left_scaling_factor * right_segment.size()),
        )?;
    }
    Ok(())
}

// Used instead of `f64::round` directly, to provide fallback for `no_std`.
#[cfg(feature = "std")]
#[inline]
fn round(value: f64) -> f64 {
    value.round()
}

// A rounding fallback for `no_std` in pure rust.
#[cfg(not(feature = "std"))]
#[inline]
fn round(value: f64) -> f64 {
    (value + 0.5f64.copysign(value)) as i64 as f64
}

fn changes_to_rects(
    changes: &HashMap<Variable, f64>,
    elements: &[Element],
    area: Rect,
    direction: Direction,
) -> Rects {
    // convert to Rects
    elements
        .iter()
        .map(|element| {
            let start = changes.get(&element.start).unwrap_or(&0.0);
            let end = changes.get(&element.end).unwrap_or(&0.0);
            let start = round(round(*start) / FLOAT_PRECISION_MULTIPLIER) as u16;
            let end = round(round(*end) / FLOAT_PRECISION_MULTIPLIER) as u16;
            let size = end.saturating_sub(start);
            match direction {
                Direction::Horizontal => Rect {
                    x: start,
                    y: area.y,
                    width: size,
                    height: area.height,
                },
                Direction::Vertical => Rect {
                    x: area.x,
                    y: start,
                    width: area.width,
                    height: size,
                },
            }
        })
        .collect::<Rects>()
}

/// please leave this here as it's useful for debugging unit tests when we make any changes to
/// layout code - we should replace this with tracing in the future.
#[expect(dead_code)]
#[cfg(feature = "std")]
fn debug_elements(elements: &[Element], changes: &HashMap<Variable, f64>) {
    let variables = alloc::format!(
        "{:?}",
        elements
            .iter()
            .map(|e| (
                changes.get(&e.start).unwrap_or(&0.0) / FLOAT_PRECISION_MULTIPLIER,
                changes.get(&e.end).unwrap_or(&0.0) / FLOAT_PRECISION_MULTIPLIER,
            ))
            .collect::<Vec<(f64, f64)>>()
    );
    std::dbg!(variables);
}

/// A container used by the solver inside split
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct Element {
    start: Variable,
    end: Variable,
}

impl From<(Variable, Variable)> for Element {
    fn from((start, end): (Variable, Variable)) -> Self {
        Self { start, end }
    }
}

impl Element {
    #[expect(dead_code)]
    fn new() -> Self {
        Self {
            start: Variable::new(),
            end: Variable::new(),
        }
    }

    fn size(&self) -> Expression {
        self.end - self.start
    }

    fn has_max_size(&self, size: u16, strength: Strength) -> kasuari::Constraint {
        self.size() | LE(strength) | (f64::from(size) * FLOAT_PRECISION_MULTIPLIER)
    }

    fn has_min_size(&self, size: i16, strength: Strength) -> kasuari::Constraint {
        self.size() | GE(strength) | (f64::from(size) * FLOAT_PRECISION_MULTIPLIER)
    }

    fn has_int_size(&self, size: u16, strength: Strength) -> kasuari::Constraint {
        self.size() | EQ(strength) | (f64::from(size) * FLOAT_PRECISION_MULTIPLIER)
    }

    fn has_size<E: Into<Expression>>(&self, size: E, strength: Strength) -> kasuari::Constraint {
        self.size() | EQ(strength) | size.into()
    }

    fn has_double_size<E: Into<Expression>>(
        &self,
        size: E,
        strength: Strength,
    ) -> kasuari::Constraint {
        self.size() | EQ(strength) | (size.into() * 2.0)
    }

    fn is_empty(&self) -> kasuari::Constraint {
        self.size() | EQ(Strength::REQUIRED - Strength::WEAK) | 0.0
    }
}

/// allow the element to represent its own size in expressions
impl From<Element> for Expression {
    fn from(element: Element) -> Self {
        element.size()
    }
}

/// allow the element to represent its own size in expressions
impl From<&Element> for Expression {
    fn from(element: &Element) -> Self {
        element.size()
    }
}

mod strengths {
    use kasuari::Strength;

    /// The strength to apply to Spacers to ensure that their sizes are equal.
    ///
    /// ┌     ┐┌───┐┌     ┐┌───┐┌     ┐
    ///   ==x  │   │  ==x  │   │  ==x
    /// └     ┘└───┘└     ┘└───┘└     ┘
    pub const SPACER_SIZE_EQ: Strength = Strength::REQUIRED.div_f64(10.0);

    /// The strength to apply to Min inequality constraints.
    ///
    /// ┌────────┐
    /// │Min(>=x)│
    /// └────────┘
    pub const MIN_SIZE_GE: Strength = Strength::STRONG.mul_f64(100.0);

    /// The strength to apply to Max inequality constraints.
    ///
    /// ┌────────┐
    /// │Max(<=x)│
    /// └────────┘
    pub const MAX_SIZE_LE: Strength = Strength::STRONG.mul_f64(100.0);

    /// The strength to apply to Length constraints.
    ///
    /// ┌───────────┐
    /// │Length(==x)│
    /// └───────────┘
    pub const LENGTH_SIZE_EQ: Strength = Strength::STRONG.mul_f64(10.0);

    /// The strength to apply to Percentage constraints.
    ///
    /// ┌───────────────┐
    /// │Percentage(==x)│
    /// └───────────────┘
    pub const PERCENTAGE_SIZE_EQ: Strength = Strength::STRONG;

    /// The strength to apply to Ratio constraints.
    ///
    /// ┌────────────┐
    /// │Ratio(==x,y)│
    /// └────────────┘
    pub const RATIO_SIZE_EQ: Strength = Strength::STRONG.div_f64(10.0);

    /// The strength to apply to Min equality constraints.
    ///
    /// ┌────────┐
    /// │Min(==x)│
    /// └────────┘
    pub const MIN_SIZE_EQ: Strength = Strength::MEDIUM.mul_f64(10.0);

    /// The strength to apply to Max equality constraints.
    ///
    /// ┌────────┐
    /// │Max(==x)│
    /// └────────┘
    pub const MAX_SIZE_EQ: Strength = Strength::MEDIUM.mul_f64(10.0);

    /// The strength to apply to Fill growing constraints.
    ///
    /// ┌─────────────────────┐
    /// │<=     Fill(x)     =>│
    /// └─────────────────────┘
    pub const FILL_GROW: Strength = Strength::MEDIUM;

    /// The strength to apply to growing constraints.
    ///
    /// ┌────────────┐
    /// │<= Min(x) =>│
    /// └────────────┘
    pub const GROW: Strength = Strength::MEDIUM.div_f64(10.0);

    /// The strength to apply to Spacer growing constraints.
    ///
    /// ┌       ┐
    ///  <= x =>
    /// └       ┘
    pub const SPACE_GROW: Strength = Strength::WEAK.mul_f64(10.0);

    /// The strength to apply to growing the size of all segments equally.
    ///
    /// ┌───────┐
    /// │<= x =>│
    /// └───────┘
    pub const ALL_SEGMENT_GROW: Strength = Strength::WEAK;
}

#[cfg(test)]
mod tests {
    use alloc::borrow::ToOwned;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;

    #[test]
    // The compiler will optimize out the comparisons, but this ensures that the constants are
    // defined in the correct order of priority.
    pub fn strength_is_valid() {
        use strengths::*;
        assert!(SPACER_SIZE_EQ > MAX_SIZE_LE);
        assert!(MAX_SIZE_LE > MAX_SIZE_EQ);
        assert!(MIN_SIZE_GE == MAX_SIZE_LE);
        assert!(MAX_SIZE_LE > LENGTH_SIZE_EQ);
        assert!(LENGTH_SIZE_EQ > PERCENTAGE_SIZE_EQ);
        assert!(PERCENTAGE_SIZE_EQ > RATIO_SIZE_EQ);
        assert!(RATIO_SIZE_EQ > MAX_SIZE_EQ);
        assert!(MIN_SIZE_GE > FILL_GROW);
        assert!(FILL_GROW > GROW);
        assert!(GROW > SPACE_GROW);
        assert!(SPACE_GROW > ALL_SEGMENT_GROW);
    }

    #[test]
    #[cfg(feature = "layout-cache")]
    fn cache_size() {
        LAYOUT_CACHE.with_borrow(|cache| {
            assert_eq!(cache.cap().get(), Layout::DEFAULT_CACHE_SIZE);
        });

        Layout::init_cache(NonZeroUsize::new(10).unwrap());
        LAYOUT_CACHE.with_borrow(|cache| {
            assert_eq!(cache.cap().get(), 10);
        });
    }

    #[test]
    fn default() {
        assert_eq!(
            Layout::default(),
            Layout {
                direction: Direction::Vertical,
                margin: Margin::new(0, 0),
                constraints: vec![],
                flex: Flex::default(),
                spacing: Spacing::default(),
            }
        );
    }

    #[test]
    fn new() {
        // array
        let fixed_size_array = [Constraint::Min(0)];
        let layout = Layout::new(Direction::Horizontal, fixed_size_array);
        assert_eq!(layout.direction, Direction::Horizontal);
        assert_eq!(layout.constraints, [Constraint::Min(0)]);

        // array_ref
        #[expect(clippy::needless_borrows_for_generic_args)] // backwards compatibility test
        let layout = Layout::new(Direction::Horizontal, &[Constraint::Min(0)]);
        assert_eq!(layout.direction, Direction::Horizontal);
        assert_eq!(layout.constraints, [Constraint::Min(0)]);

        // vec
        let layout = Layout::new(Direction::Horizontal, vec![Constraint::Min(0)]);
        assert_eq!(layout.direction, Direction::Horizontal);
        assert_eq!(layout.constraints, [Constraint::Min(0)]);

        // vec_ref
        #[expect(clippy::needless_borrows_for_generic_args)] // backwards compatibility test
        let layout = Layout::new(Direction::Horizontal, &(vec![Constraint::Min(0)]));
        assert_eq!(layout.direction, Direction::Horizontal);
        assert_eq!(layout.constraints, [Constraint::Min(0)]);

        // iterator
        let layout = Layout::new(Direction::Horizontal, iter::once(Constraint::Min(0)));
        assert_eq!(layout.direction, Direction::Horizontal);
        assert_eq!(layout.constraints, [Constraint::Min(0)]);
    }

    #[test]
    fn vertical() {
        assert_eq!(
            Layout::vertical([Constraint::Min(0)]),
            Layout {
                direction: Direction::Vertical,
                margin: Margin::new(0, 0),
                constraints: vec![Constraint::Min(0)],
                flex: Flex::default(),
                spacing: Spacing::default(),
            }
        );
    }

    #[test]
    fn horizontal() {
        assert_eq!(
            Layout::horizontal([Constraint::Min(0)]),
            Layout {
                direction: Direction::Horizontal,
                margin: Margin::new(0, 0),
                constraints: vec![Constraint::Min(0)],
                flex: Flex::default(),
                spacing: Spacing::default(),
            }
        );
    }

    /// The purpose of this test is to ensure that layout can be constructed with any type that
    /// implements `IntoIterator<Item = AsRef<Constraint>>`.
    #[test]
    fn constraints() {
        const CONSTRAINTS: [Constraint; 2] = [Constraint::Min(0), Constraint::Max(10)];
        let fixed_size_array = CONSTRAINTS;
        assert_eq!(
            Layout::default().constraints(fixed_size_array).constraints,
            CONSTRAINTS,
            "constraints should be settable with an array"
        );

        let slice_of_fixed_size_array = &CONSTRAINTS;
        assert_eq!(
            Layout::default()
                .constraints(slice_of_fixed_size_array)
                .constraints,
            CONSTRAINTS,
            "constraints should be settable with a slice"
        );

        let vec = CONSTRAINTS.to_vec();
        let slice_of_vec = vec.as_slice();
        assert_eq!(
            Layout::default().constraints(slice_of_vec).constraints,
            CONSTRAINTS,
            "constraints should be settable with a slice"
        );

        assert_eq!(
            Layout::default().constraints(vec).constraints,
            CONSTRAINTS,
            "constraints should be settable with a Vec"
        );

        let iter = CONSTRAINTS.iter();
        assert_eq!(
            Layout::default().constraints(iter).constraints,
            CONSTRAINTS,
            "constraints should be settable with an iter"
        );

        let iterator = CONSTRAINTS.iter().map(ToOwned::to_owned);
        assert_eq!(
            Layout::default().constraints(iterator).constraints,
            CONSTRAINTS,
            "constraints should be settable with an iterator"
        );

        let iterator_ref = CONSTRAINTS.iter().map(AsRef::as_ref);
        assert_eq!(
            Layout::default().constraints(iterator_ref).constraints,
            CONSTRAINTS,
            "constraints should be settable with an iterator of refs"
        );
    }

    #[test]
    fn direction() {
        assert_eq!(
            Layout::default().direction(Direction::Horizontal).direction,
            Direction::Horizontal
        );
        assert_eq!(
            Layout::default().direction(Direction::Vertical).direction,
            Direction::Vertical
        );
    }

    #[test]
    fn margins() {
        assert_eq!(Layout::default().margin(10).margin, Margin::new(10, 10));
        assert_eq!(
            Layout::default().horizontal_margin(10).margin,
            Margin::new(10, 0)
        );
        assert_eq!(
            Layout::default().vertical_margin(10).margin,
            Margin::new(0, 10)
        );
        assert_eq!(
            Layout::default()
                .horizontal_margin(10)
                .vertical_margin(20)
                .margin,
            Margin::new(10, 20)
        );
    }

    #[test]
    fn flex() {
        assert_eq!(Layout::default().flex, Flex::Start);
        assert_eq!(Layout::default().flex(Flex::Center).flex, Flex::Center);
    }

    #[test]
    fn spacing() {
        assert_eq!(Layout::default().spacing(10).spacing, Spacing::Space(10));
        assert_eq!(Layout::default().spacing(0).spacing, Spacing::Space(0));
        assert_eq!(Layout::default().spacing(-10).spacing, Spacing::Overlap(10));
    }

    /// Tests for the `Layout::split()` function.
    ///
    /// There are many tests in this as the number of edge cases that are caused by the interaction
    /// between the constraints is quite large. The tests are split into sections based on the type
    /// of constraints that are used.
    ///
    /// These tests are characterization tests. This means that they are testing the way the code
    /// currently works, and not the way it should work. This is because the current behavior is not
    /// well defined, and it is not clear what the correct behavior should be. This means that if
    /// the behavior changes, these tests should be updated to match the new behavior.
    ///
    ///  EOL comments in each test are intended to communicate the purpose of each test and to make
    ///  it easy to see that the tests are as exhaustive as feasible:
    /// - zero: constraint is zero
    /// - exact: constraint is equal to the space
    /// - underflow: constraint is for less than the full space
    /// - overflow: constraint is for more than the full space
    mod split {
        use alloc::string::ToString;
        use core::ops::Range;

        use itertools::Itertools;
        use pretty_assertions::assert_eq;
        use rstest::rstest;

        use super::*;
        use crate::buffer::Buffer;
        use crate::layout::Constraint::{self, *};
        use crate::layout::{Direction, Flex, Layout, Rect};
        use crate::text::Text;
        use crate::widgets::Widget;

        /// Test that the given constraints applied to the given area result in the expected layout.
        /// Each chunk is filled with a letter repeated as many times as the width of the chunk. The
        /// resulting buffer is compared to the expected string.
        ///
        /// This approach is used rather than testing the resulting rects directly because it is
        /// easier to visualize the result, and it leads to more concise tests that are easier to
        /// compare against each other. E.g. `"abc"` is much more concise than `[Rect::new(0, 0, 1,
        /// 1), Rect::new(1, 0, 1, 1), Rect::new(2, 0, 1, 1)]`.
        #[track_caller]
        fn letters(flex: Flex, constraints: &[Constraint], width: u16, expected: &str) {
            let area = Rect::new(0, 0, width, 1);
            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .flex(flex)
                .split(area);
            let mut buffer = Buffer::empty(area);
            for (c, &area) in ('a'..='z').take(constraints.len()).zip(layout.iter()) {
                let s = c.to_string().repeat(area.width as usize);
                Text::from(s).render(area, &mut buffer);
            }
            assert_eq!(buffer, Buffer::with_lines([expected]));
        }

        #[rstest]
        // flex, width, lengths, expected
        #[case(Flex::Legacy, 1, &[Length(0)], "a")] // zero
        #[case(Flex::Legacy, 1, &[Length(1)], "a")] // exact
        #[case(Flex::Legacy, 1, &[Length(2)], "a")] // overflow
        #[case(Flex::Legacy, 2, &[Length(0)], "aa")] // zero
        #[case(Flex::Legacy, 2, &[Length(1)], "aa")] // underflow
        #[case(Flex::Legacy, 2, &[Length(2)], "aa")] // exact
        #[case(Flex::Legacy, 2, &[Length(3)], "aa")] // overflow
        #[case(Flex::Legacy, 1, &[Length(0), Length(0)], "b")] // zero, zero
        #[case(Flex::Legacy, 1, &[Length(0), Length(1)], "b")] // zero, exact
        #[case(Flex::Legacy, 1, &[Length(0), Length(2)], "b")] // zero, overflow
        #[case(Flex::Legacy, 1, &[Length(1), Length(0)], "a")] // exact, zero
        #[case(Flex::Legacy, 1, &[Length(1), Length(1)], "a")] // exact, exact
        #[case(Flex::Legacy, 1, &[Length(1), Length(2)], "a")] // exact, overflow
        #[case(Flex::Legacy, 1, &[Length(2), Length(0)], "a")] // overflow, zero
        #[case(Flex::Legacy, 1, &[Length(2), Length(1)], "a")] // overflow, exact
        #[case(Flex::Legacy, 1, &[Length(2), Length(2)], "a")] // overflow, overflow
        #[case(Flex::Legacy, 2, &[Length(0), Length(0)], "bb")] // zero, zero
        #[case(Flex::Legacy, 2, &[Length(0), Length(1)], "bb")] // zero, underflow
        #[case(Flex::Legacy, 2, &[Length(0), Length(2)], "bb")] // zero, exact
        #[case(Flex::Legacy, 2, &[Length(0), Length(3)], "bb")] // zero, overflow
        #[case(Flex::Legacy, 2, &[Length(1), Length(0)], "ab")] // underflow, zero
        #[case(Flex::Legacy, 2, &[Length(1), Length(1)], "ab")] // underflow, underflow
        #[case(Flex::Legacy, 2, &[Length(1), Length(2)], "ab")] // underflow, exact
        #[case(Flex::Legacy, 2, &[Length(1), Length(3)], "ab")] // underflow, overflow
        #[case(Flex::Legacy, 2, &[Length(2), Length(0)], "aa")] // exact, zero
        #[case(Flex::Legacy, 2, &[Length(2), Length(1)], "aa")] // exact, underflow
        #[case(Flex::Legacy, 2, &[Length(2), Length(2)], "aa")] // exact, exact
        #[case(Flex::Legacy, 2, &[Length(2), Length(3)], "aa")] // exact, overflow
        #[case(Flex::Legacy, 2, &[Length(3), Length(0)], "aa")] // overflow, zero
        #[case(Flex::Legacy, 2, &[Length(3), Length(1)], "aa")] // overflow, underflow
        #[case(Flex::Legacy, 2, &[Length(3), Length(2)], "aa")] // overflow, exact
        #[case(Flex::Legacy, 2, &[Length(3), Length(3)], "aa")] // overflow, overflow
        #[case(Flex::Legacy, 3, &[Length(2), Length(2)], "aab")] // with stretchlast
        fn length(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        #[case(Flex::Legacy, 1, &[Max(0)], "a")] // zero
        #[case(Flex::Legacy, 1, &[Max(1)], "a")] // exact
        #[case(Flex::Legacy, 1, &[Max(2)], "a")] // overflow
        #[case(Flex::Legacy, 2, &[Max(0)], "aa")] // zero
        #[case(Flex::Legacy, 2, &[Max(1)], "aa")] // underflow
        #[case(Flex::Legacy, 2, &[Max(2)], "aa")] // exact
        #[case(Flex::Legacy, 2, &[Max(3)], "aa")] // overflow
        #[case(Flex::Legacy, 1, &[Max(0), Max(0)], "b")] // zero, zero
        #[case(Flex::Legacy, 1, &[Max(0), Max(1)], "b")] // zero, exact
        #[case(Flex::Legacy, 1, &[Max(0), Max(2)], "b")] // zero, overflow
        #[case(Flex::Legacy, 1, &[Max(1), Max(0)], "a")] // exact, zero
        #[case(Flex::Legacy, 1, &[Max(1), Max(1)], "a")] // exact, exact
        #[case(Flex::Legacy, 1, &[Max(1), Max(2)], "a")] // exact, overflow
        #[case(Flex::Legacy, 1, &[Max(2), Max(0)], "a")] // overflow, zero
        #[case(Flex::Legacy, 1, &[Max(2), Max(1)], "a")] // overflow, exact
        #[case(Flex::Legacy, 1, &[Max(2), Max(2)], "a")] // overflow, overflow
        #[case(Flex::Legacy, 2, &[Max(0), Max(0)], "bb")] // zero, zero
        #[case(Flex::Legacy, 2, &[Max(0), Max(1)], "bb")] // zero, underflow
        #[case(Flex::Legacy, 2, &[Max(0), Max(2)], "bb")] // zero, exact
        #[case(Flex::Legacy, 2, &[Max(0), Max(3)], "bb")] // zero, overflow
        #[case(Flex::Legacy, 2, &[Max(1), Max(0)], "ab")] // underflow, zero
        #[case(Flex::Legacy, 2, &[Max(1), Max(1)], "ab")] // underflow, underflow
        #[case(Flex::Legacy, 2, &[Max(1), Max(2)], "ab")] // underflow, exact
        #[case(Flex::Legacy, 2, &[Max(1), Max(3)], "ab")] // underflow, overflow
        #[case(Flex::Legacy, 2, &[Max(2), Max(0)], "aa")] // exact, zero
        #[case(Flex::Legacy, 2, &[Max(2), Max(1)], "aa")] // exact, underflow
        #[case(Flex::Legacy, 2, &[Max(2), Max(2)], "aa")] // exact, exact
        #[case(Flex::Legacy, 2, &[Max(2), Max(3)], "aa")] // exact, overflow
        #[case(Flex::Legacy, 2, &[Max(3), Max(0)], "aa")] // overflow, zero
        #[case(Flex::Legacy, 2, &[Max(3), Max(1)], "aa")] // overflow, underflow
        #[case(Flex::Legacy, 2, &[Max(3), Max(2)], "aa")] // overflow, exact
        #[case(Flex::Legacy, 2, &[Max(3), Max(3)], "aa")] // overflow, overflow
        #[case(Flex::Legacy, 3, &[Max(2), Max(2)], "aab")]
        fn max(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        #[case(Flex::Legacy, 1, &[Min(0), Min(0)], "b")] // zero, zero
        #[case(Flex::Legacy, 1, &[Min(0), Min(1)], "b")] // zero, exact
        #[case(Flex::Legacy, 1, &[Min(0), Min(2)], "b")] // zero, overflow
        #[case(Flex::Legacy, 1, &[Min(1), Min(0)], "a")] // exact, zero
        #[case(Flex::Legacy, 1, &[Min(1), Min(1)], "a")] // exact, exact
        #[case(Flex::Legacy, 1, &[Min(1), Min(2)], "a")] // exact, overflow
        #[case(Flex::Legacy, 1, &[Min(2), Min(0)], "a")] // overflow, zero
        #[case(Flex::Legacy, 1, &[Min(2), Min(1)], "a")] // overflow, exact
        #[case(Flex::Legacy, 1, &[Min(2), Min(2)], "a")] // overflow, overflow
        #[case(Flex::Legacy, 2, &[Min(0), Min(0)], "bb")] // zero, zero
        #[case(Flex::Legacy, 2, &[Min(0), Min(1)], "bb")] // zero, underflow
        #[case(Flex::Legacy, 2, &[Min(0), Min(2)], "bb")] // zero, exact
        #[case(Flex::Legacy, 2, &[Min(0), Min(3)], "bb")] // zero, overflow
        #[case(Flex::Legacy, 2, &[Min(1), Min(0)], "ab")] // underflow, zero
        #[case(Flex::Legacy, 2, &[Min(1), Min(1)], "ab")] // underflow, underflow
        #[case(Flex::Legacy, 2, &[Min(1), Min(2)], "ab")] // underflow, exact
        #[case(Flex::Legacy, 2, &[Min(1), Min(3)], "ab")] // underflow, overflow
        #[case(Flex::Legacy, 2, &[Min(2), Min(0)], "aa")] // exact, zero
        #[case(Flex::Legacy, 2, &[Min(2), Min(1)], "aa")] // exact, underflow
        #[case(Flex::Legacy, 2, &[Min(2), Min(2)], "aa")] // exact, exact
        #[case(Flex::Legacy, 2, &[Min(2), Min(3)], "aa")] // exact, overflow
        #[case(Flex::Legacy, 2, &[Min(3), Min(0)], "aa")] // overflow, zero
        #[case(Flex::Legacy, 2, &[Min(3), Min(1)], "aa")] // overflow, underflow
        #[case(Flex::Legacy, 2, &[Min(3), Min(2)], "aa")] // overflow, exact
        #[case(Flex::Legacy, 2, &[Min(3), Min(3)], "aa")] // overflow, overflow
        #[case(Flex::Legacy, 3, &[Min(2), Min(2)], "aab")]
        fn min(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest] // flex, width, lengths, expected
        // One constraint will take all the space (width = 1)
        #[case(Flex::Legacy, 1, &[Percentage(0)],   "a")]
        #[case(Flex::Legacy, 1, &[Percentage(25)],  "a")]
        #[case(Flex::Legacy, 1, &[Percentage(50)],  "a")]
        #[case(Flex::Legacy, 1, &[Percentage(90)],  "a")]
        #[case(Flex::Legacy, 1, &[Percentage(100)], "a")]
        #[case(Flex::Legacy, 1, &[Percentage(200)], "a")]
        // One constraint will take all the space (width = 2)
        #[case(Flex::Legacy, 2, &[Percentage(0)],   "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(10)],  "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(25)],  "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(50)],  "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(66)],  "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(100)], "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(200)], "aa")]
        // One constraint will take all the space (width = 3)
        #[case(Flex::Legacy, 10, &[Percentage(0)],   "aaaaaaaaaa")]
        #[case(Flex::Legacy, 10, &[Percentage(10)],  "aaaaaaaaaa")]
        #[case(Flex::Legacy, 10, &[Percentage(25)],  "aaaaaaaaaa")]
        #[case(Flex::Legacy, 10, &[Percentage(50)],  "aaaaaaaaaa")]
        #[case(Flex::Legacy, 10, &[Percentage(66)],  "aaaaaaaaaa")]
        #[case(Flex::Legacy, 10, &[Percentage(100)], "aaaaaaaaaa")]
        #[case(Flex::Legacy, 10, &[Percentage(200)], "aaaaaaaaaa")]
        // 0%/any allocates all the space to the second constraint
        #[case(Flex::Legacy, 1, &[Percentage(0), Percentage(0)],   "b")]
        #[case(Flex::Legacy, 1, &[Percentage(0), Percentage(10)],  "b")]
        #[case(Flex::Legacy, 1, &[Percentage(0), Percentage(50)],  "b")]
        #[case(Flex::Legacy, 1, &[Percentage(0), Percentage(90)],  "b")]
        #[case(Flex::Legacy, 1, &[Percentage(0), Percentage(100)], "b")]
        #[case(Flex::Legacy, 1, &[Percentage(0), Percentage(200)], "b")]
        // 10%/any allocates all the space to the second constraint (even if it is 0)
        #[case(Flex::Legacy, 1, &[Percentage(10), Percentage(0)],   "b")]
        #[case(Flex::Legacy, 1, &[Percentage(10), Percentage(10)],  "b")]
        #[case(Flex::Legacy, 1, &[Percentage(10), Percentage(50)],  "b")]
        #[case(Flex::Legacy, 1, &[Percentage(10), Percentage(90)],  "b")]
        #[case(Flex::Legacy, 1, &[Percentage(10), Percentage(100)], "b")]
        #[case(Flex::Legacy, 1, &[Percentage(10), Percentage(200)], "b")]
        // 50%/any allocates all the space to the first constraint
        #[case(Flex::Legacy, 1, &[Percentage(50), Percentage(0)],   "a")]
        #[case(Flex::Legacy, 1, &[Percentage(50), Percentage(50)],  "a")]
        #[case(Flex::Legacy, 1, &[Percentage(50), Percentage(100)], "a")]
        #[case(Flex::Legacy, 1, &[Percentage(50), Percentage(200)], "a")]
        // 90%/any allocates all the space to the first constraint
        #[case(Flex::Legacy, 1, &[Percentage(90), Percentage(0)],   "a")]
        #[case(Flex::Legacy, 1, &[Percentage(90), Percentage(50)],  "a")]
        #[case(Flex::Legacy, 1, &[Percentage(90), Percentage(100)], "a")]
        #[case(Flex::Legacy, 1, &[Percentage(90), Percentage(200)], "a")]
        // 100%/any allocates all the space to the first constraint
        #[case(Flex::Legacy, 1, &[Percentage(100), Percentage(0)],   "a")]
        #[case(Flex::Legacy, 1, &[Percentage(100), Percentage(50)],  "a")]
        #[case(Flex::Legacy, 1, &[Percentage(100), Percentage(100)], "a")]
        #[case(Flex::Legacy, 1, &[Percentage(100), Percentage(200)], "a")]
        // 0%/any allocates all the space to the second constraint
        #[case(Flex::Legacy, 2, &[Percentage(0), Percentage(0)],   "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(0), Percentage(25)],  "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(0), Percentage(50)],  "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(0), Percentage(100)], "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(0), Percentage(200)], "bb")]
        // 10%/any allocates all the space to the second constraint
        #[case(Flex::Legacy, 2, &[Percentage(10), Percentage(0)],   "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(10), Percentage(25)],  "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(10), Percentage(50)],  "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(10), Percentage(100)], "bb")]
        #[case(Flex::Legacy, 2, &[Percentage(10), Percentage(200)], "bb")]
        // 25% * 2 = 0.5, which rounds up to 1, so the first constraint gets 1
        #[case(Flex::Legacy, 2, &[Percentage(25), Percentage(0)],   "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(25), Percentage(25)],  "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(25), Percentage(50)],  "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(25), Percentage(100)], "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(25), Percentage(200)], "ab")]
        // 33% * 2 = 0.66, so the first constraint gets 1
        #[case(Flex::Legacy, 2, &[Percentage(33), Percentage(0)],   "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(33), Percentage(25)],  "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(33), Percentage(50)],  "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(33), Percentage(100)], "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(33), Percentage(200)], "ab")]
        // 50% * 2 = 1, so the first constraint gets 1
        #[case(Flex::Legacy, 2, &[Percentage(50), Percentage(0)],   "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(50), Percentage(50)],  "ab")]
        #[case(Flex::Legacy, 2, &[Percentage(50), Percentage(100)], "ab")]
        // 100%/any allocates all the space to the first constraint
        // This is probably not the correct behavior, but it is the current behavior
        #[case(Flex::Legacy, 2, &[Percentage(100), Percentage(0)],   "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(100), Percentage(50)],  "aa")]
        #[case(Flex::Legacy, 2, &[Percentage(100), Percentage(100)], "aa")]
        // 33%/any allocates 1 to the first constraint the rest to the second
        #[case(Flex::Legacy, 3, &[Percentage(33), Percentage(33)], "abb")]
        #[case(Flex::Legacy, 3, &[Percentage(33), Percentage(66)], "abb")]
        // 33%/any allocates 1.33 = 1 to the first constraint the rest to the second
        #[case(Flex::Legacy, 4, &[Percentage(33), Percentage(33)], "abbb")]
        #[case(Flex::Legacy, 4, &[Percentage(33), Percentage(66)], "abbb")]
        // Longer tests zero allocates everything to the second constraint
        #[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(0)],   "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(25)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(50)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(100)], "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(0),   Percentage(200)], "bbbbbbbbbb" )]
        // 10% allocates a single character to the first constraint
        #[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(0)],   "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(25)],  "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(50)],  "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(100)], "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(10),  Percentage(200)], "abbbbbbbbb" )]
        // 25% allocates 2.5 = 3 characters to the first constraint
        #[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(0)],   "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(25)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(50)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(100)], "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(25),  Percentage(200)], "aaabbbbbbb" )]
        // 33% allocates 3.3 = 3 characters to the first constraint
        #[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(0)],   "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(25)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(50)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(100)], "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(33),  Percentage(200)], "aaabbbbbbb" )]
        // 50% allocates 5 characters to the first constraint
        #[case(Flex::Legacy, 10, &[Percentage(50),  Percentage(0)],   "aaaaabbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(50),  Percentage(50)],  "aaaaabbbbb" )]
        #[case(Flex::Legacy, 10, &[Percentage(50),  Percentage(100)], "aaaaabbbbb" )]
        // 100% allocates everything to the first constraint
        #[case(Flex::Legacy, 10, &[Percentage(100), Percentage(0)],   "aaaaaaaaaa" )]
        #[case(Flex::Legacy, 10, &[Percentage(100), Percentage(50)],  "aaaaaaaaaa" )]
        #[case(Flex::Legacy, 10, &[Percentage(100), Percentage(100)], "aaaaaaaaaa" )]
        fn percentage(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        #[case(Flex::Start, 10, &[Percentage(0),   Percentage(0)],    "          " )]
        #[case(Flex::Start, 10, &[Percentage(0),   Percentage(25)],  "bbb       " )]
        #[case(Flex::Start, 10, &[Percentage(0),   Percentage(50)],  "bbbbb     " )]
        #[case(Flex::Start, 10, &[Percentage(0),   Percentage(100)], "bbbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(0),   Percentage(200)], "bbbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(10),  Percentage(0)],   "a         " )]
        #[case(Flex::Start, 10, &[Percentage(10),  Percentage(25)],  "abbb      " )]
        #[case(Flex::Start, 10, &[Percentage(10),  Percentage(50)],  "abbbbb    " )]
        #[case(Flex::Start, 10, &[Percentage(10),  Percentage(100)], "abbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(10),  Percentage(200)], "abbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(25),  Percentage(0)],   "aaa       " )]
        #[case(Flex::Start, 10, &[Percentage(25),  Percentage(25)],  "aaabb     " )]
        #[case(Flex::Start, 10, &[Percentage(25),  Percentage(50)],  "aaabbbbb  " )]
        #[case(Flex::Start, 10, &[Percentage(25),  Percentage(100)], "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(25),  Percentage(200)], "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(33),  Percentage(0)],   "aaa       " )]
        #[case(Flex::Start, 10, &[Percentage(33),  Percentage(25)],  "aaabbb    " )]
        #[case(Flex::Start, 10, &[Percentage(33),  Percentage(50)],  "aaabbbbb  " )]
        #[case(Flex::Start, 10, &[Percentage(33),  Percentage(100)], "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(33),  Percentage(200)], "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(50),  Percentage(0)],   "aaaaa     " )]
        #[case(Flex::Start, 10, &[Percentage(50),  Percentage(50)],  "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(50),  Percentage(100)], "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(100), Percentage(0)],   "aaaaaaaaaa" )]
        #[case(Flex::Start, 10, &[Percentage(100), Percentage(50)],  "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(100), Percentage(100)], "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Percentage(100), Percentage(200)], "aaaaabbbbb" )]
        fn percentage_start(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        #[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(0)],   "          " )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(25)],  "        bb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(50)],  "     bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(100)], "bbbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(0),   Percentage(200)], "bbbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(0)],   "a         " )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(25)],  "a       bb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(50)],  "a    bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(100)], "abbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(10),  Percentage(200)], "abbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(0)],   "aaa       " )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(25)],  "aaa     bb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(50)],  "aaa  bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(100)], "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(25),  Percentage(200)], "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(0)],   "aaa       " )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(25)],  "aaa     bb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(50)],  "aaa  bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(100)], "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(33),  Percentage(200)], "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(50),  Percentage(0)],   "aaaaa     " )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(50),  Percentage(50)],  "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(50),  Percentage(100)], "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(0)],   "aaaaaaaaaa" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(50)],  "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(100)], "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Percentage(100), Percentage(200)], "aaaaabbbbb" )]
        fn percentage_spacebetween(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        // flex, width, ratios, expected
        // Just one ratio takes up the whole space
        #[case(Flex::Legacy, 1, &[Ratio(0, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 4)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 2)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(9, 10)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(2, 1)], "a")]
        #[case(Flex::Legacy, 2, &[Ratio(0, 1)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 10)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 4)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 2)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(2, 3)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 1)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(2, 1)], "aa")]
        #[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(0, 1)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(1, 10)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(1, 2)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(9, 10)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(1, 1)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(0, 1), Ratio(2, 1)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(0, 1)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(1, 10)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(1, 2)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(9, 10)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(1, 1)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 10), Ratio(2, 1)], "b")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(0, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(1, 2)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(1, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 2), Ratio(2, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(0, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(1, 2)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(1, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(9, 10), Ratio(2, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(0, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(1, 2)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(1, 1)], "a")]
        #[case(Flex::Legacy, 1, &[Ratio(1, 1), Ratio(2, 1)], "a")]
        #[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(0, 1)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(1, 4)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(1, 2)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(1, 1)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(0, 1), Ratio(2, 1)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(0, 1)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(1, 4)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(1, 2)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(1, 1)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 10), Ratio(2, 1)], "bb")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(0, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(1, 4)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(1, 2)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(1, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 4), Ratio(2, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(0, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(1, 4)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(1, 2)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(1, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 3), Ratio(2, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 2), Ratio(0, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 2), Ratio(1, 2)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 2), Ratio(1, 1)], "ab")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 1), Ratio(0, 1)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 1), Ratio(1, 2)], "aa")]
        #[case(Flex::Legacy, 2, &[Ratio(1, 1), Ratio(1, 1)], "aa")]
        #[case(Flex::Legacy, 3, &[Ratio(1, 3), Ratio(1, 3)], "abb")]
        #[case(Flex::Legacy, 3, &[Ratio(1, 3), Ratio(2,3)], "abb")]
        #[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(0, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(1, 4)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(1, 2)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(1, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(0, 1), Ratio(2, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(0, 1)], "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(1, 4)], "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(1, 2)], "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(1, 1)], "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 10), Ratio(2, 1)], "abbbbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(0, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(1, 4)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(1, 2)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(1, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 4), Ratio(2, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(0, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(1, 4)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(1, 2)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(1, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 3), Ratio(2, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 2), Ratio(0, 1)],  "aaaaabbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 2), Ratio(1, 2)],  "aaaaabbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 2), Ratio(1, 1)],  "aaaaabbbbb" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 1), Ratio(0, 1)],  "aaaaaaaaaa" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 1), Ratio(1, 2)],  "aaaaaaaaaa" )]
        #[case(Flex::Legacy, 10, &[Ratio(1, 1), Ratio(1, 1)],  "aaaaaaaaaa" )]
        fn ratio(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        #[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(0, 1)],   "          " )]
        #[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(1, 4)],  "bbb       " )]
        #[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(1, 2)],  "bbbbb     " )]
        #[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(1, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(0, 1), Ratio(2, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(0, 1)], "a         " )]
        #[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(1, 4)], "abbb      " )]
        #[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(1, 2)], "abbbbb    " )]
        #[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(1, 1)], "abbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 10), Ratio(2, 1)], "abbbbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(0, 1)],  "aaa       " )]
        #[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(1, 4)],  "aaabb     " )]
        #[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(1, 2)],  "aaabbbbb  " )]
        #[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(1, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 4), Ratio(2, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(0, 1)],  "aaa       " )]
        #[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(1, 4)],  "aaabbb    " )]
        #[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(1, 2)],  "aaabbbbb  " )]
        #[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(1, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 3), Ratio(2, 1)],  "aaabbbbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 2), Ratio(0, 1)],  "aaaaa     " )]
        #[case(Flex::Start, 10, &[Ratio(1, 2), Ratio(1, 2)],  "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 2), Ratio(1, 1)],  "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(0, 1)],  "aaaaaaaaaa" )]
        #[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(1, 2)],  "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(1, 1)],  "aaaaabbbbb" )]
        #[case(Flex::Start, 10, &[Ratio(1, 1), Ratio(2, 1)],  "aaaaabbbbb" )]
        fn ratio_start(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[rstest]
        #[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(0, 1)],  "          " )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(1, 4)],  "        bb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(1, 2)],  "     bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(1, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(0, 1), Ratio(2, 1)],  "bbbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(0, 1)], "a         " )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(1, 4)], "a       bb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(1, 2)], "a    bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(1, 1)], "abbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 10), Ratio(2, 1)], "abbbbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(0, 1)],  "aaa       " )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(1, 4)],  "aaa     bb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(1, 2)],  "aaa  bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(1, 1)],  "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 4), Ratio(2, 1)],  "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(0, 1)],  "aaa       " )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(1, 4)],  "aaa     bb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(1, 2)],  "aaa  bbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(1, 1)],  "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 3), Ratio(2, 1)],  "aaabbbbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 2), Ratio(0, 1)],  "aaaaa     " )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 2), Ratio(1, 2)],  "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 2), Ratio(1, 1)],  "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(0, 1)],  "aaaaaaaaaa" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(1, 2)],  "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(1, 1)],  "aaaaabbbbb" )]
        #[case(Flex::SpaceBetween, 10, &[Ratio(1, 1), Ratio(2, 1)],  "aaaaabbbbb" )]
        fn ratio_spacebetween(
            #[case] flex: Flex,
            #[case] width: u16,
            #[case] constraints: &[Constraint],
            #[case] expected: &str,
        ) {
            letters(flex, constraints, width, expected);
        }

        #[test]
        fn vertical_split_by_height() {
            let target = Rect {
                x: 2,
                y: 2,
                width: 10,
                height: 10,
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(10),
                    Constraint::Max(5),
                    Constraint::Min(1),
                ])
                .split(target);

            assert_eq!(chunks.iter().map(|r| r.height).sum::<u16>(), target.height);
            chunks.windows(2).for_each(|w| assert!(w[0].y <= w[1].y));
        }

        #[test]
        fn edge_cases() {
            // stretches into last
            let layout = Layout::default()
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                    Constraint::Min(0),
                ])
                .split(Rect::new(0, 0, 1, 1));
            assert_eq!(
                layout[..],
                [
                    Rect::new(0, 0, 1, 1),
                    Rect::new(0, 1, 1, 0),
                    Rect::new(0, 1, 1, 0)
                ]
            );

            // stretches into last
            let layout = Layout::default()
                .constraints([
                    Constraint::Max(1),
                    Constraint::Percentage(99),
                    Constraint::Min(0),
                ])
                .split(Rect::new(0, 0, 1, 1));
            assert_eq!(
                layout[..],
                [
                    Rect::new(0, 0, 1, 0),
                    Rect::new(0, 0, 1, 1),
                    Rect::new(0, 1, 1, 0)
                ]
            );

            // minimal bug from
            // https://github.com/ratatui/ratatui/pull/404#issuecomment-1681850644
            // TODO: check if this bug is now resolved?
            let layout = Layout::default()
                .constraints([Min(1), Length(0), Min(1)])
                .direction(Direction::Horizontal)
                .split(Rect::new(0, 0, 1, 1));
            assert_eq!(
                layout[..],
                [
                    Rect::new(0, 0, 1, 1),
                    Rect::new(1, 0, 0, 1),
                    Rect::new(1, 0, 0, 1),
                ]
            );

            // This stretches the 2nd last length instead of the last min based on ranking
            let layout = Layout::default()
                .constraints([Length(3), Min(4), Length(1), Min(4)])
                .direction(Direction::Horizontal)
                .split(Rect::new(0, 0, 7, 1));
            assert_eq!(
                layout[..],
                [
                    Rect::new(0, 0, 0, 1),
                    Rect::new(0, 0, 4, 1),
                    Rect::new(4, 0, 0, 1),
                    Rect::new(4, 0, 3, 1),
                ]
            );
        }

        #[rstest]
        #[case::len_min1(vec![Length(25), Min(100)], vec![0..0,  0..100])]
        #[case::len_min2(vec![Length(25), Min(0)], vec![0..25, 25..100])]
        #[case::len_max1(vec![Length(25), Max(0)], vec![0..100, 100..100])]
        #[case::len_max2(vec![Length(25), Max(100)], vec![0..25, 25..100])]
        #[case::len_perc(vec![Length(25), Percentage(25)], vec![0..25, 25..100])]
        #[case::perc_len(vec![Percentage(25), Length(25)], vec![0..75, 75..100])]
        #[case::len_ratio(vec![Length(25), Ratio(1, 4)], vec![0..25, 25..100])]
        #[case::ratio_len(vec![Ratio(1, 4), Length(25)], vec![0..75, 75..100])]
        #[case::len_len(vec![Length(25), Length(25)], vec![0..25, 25..100])]
        #[case::len1(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
        #[case::len2(vec![Length(15), Length(35), Length(25)], vec![0..15, 15..50, 50..100])]
        #[case::len3(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
        fn constraint_length(
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<Range<u16>>,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case(7, vec![Length(4), Length(4)], vec![0..3, 4..7])]
        #[case(4, vec![Length(4), Length(4)], vec![0..2, 3..4])]
        fn table_length(
            #[case] width: u16,
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<Range<u16>>,
        ) {
            let rect = Rect::new(0, 0, width, 1);
            let ranges = Layout::horizontal(constraints)
                .spacing(1)
                .flex(Flex::Start)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect::<Vec<Range<u16>>>();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::min_len_max(vec![Min(25), Length(25), Max(25)], vec![0..50, 50..75, 75..100])]
        #[case::max_len_min(vec![Max(25), Length(25), Min(25)], vec![0..25, 25..50, 50..100])]
        #[case::len_len_len(vec![Length(33), Length(33), Length(33)], vec![0..33, 33..66, 66..100])]
        #[case::len_len_len_25(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
        #[case::perc_len_ratio(vec![Percentage(25), Length(25), Ratio(1, 4)], vec![0..25, 25..50, 50..100])]
        #[case::len_ratio_perc(vec![Length(25), Ratio(1, 4), Percentage(25)], vec![0..25, 25..75, 75..100])]
        #[case::ratio_len_perc(vec![Ratio(1, 4), Length(25), Percentage(25)], vec![0..50, 50..75, 75..100])]
        #[case::ratio_perc_len(vec![Ratio(1, 4), Percentage(25), Length(25)], vec![0..50, 50..75, 75..100])]
        #[case::len_len_min(vec![Length(100), Length(1), Min(20)], vec![0..80, 80..80, 80..100])]
        #[case::min_len_len(vec![Min(20), Length(1), Length(100)], vec![0..20, 20..21, 21..100])]
        #[case::fill_len_fill(vec![Fill(1), Length(10), Fill(1)], vec![0..45, 45..55, 55..100])]
        #[case::fill_len_fill_2(vec![Fill(1), Length(10), Fill(2)], vec![0..30, 30..40, 40..100])]
        #[case::fill_len_fill_4(vec![Fill(1), Length(10), Fill(4)], vec![0..18, 18..28, 28..100])]
        #[case::fill_len_fill_5(vec![Fill(1), Length(10), Fill(5)], vec![0..15, 15..25, 25..100])]
        #[case::len_len_len_25(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
        #[case::unstable_test(vec![Length(25), Length(25), Length(25)], vec![0..25, 25..50, 50..100])]
        fn length_is_higher_priority(
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<Range<u16>>,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::min_len_max(vec![Min(25), Length(25), Max(25)], vec![50, 25, 25])]
        #[case::max_len_min(vec![Max(25), Length(25), Min(25)], vec![25, 25, 50])]
        #[case::len_len_len1(vec![Length(33), Length(33), Length(33)], vec![33, 33, 33])]
        #[case::len_len_len2(vec![Length(25), Length(25), Length(25)], vec![25, 25, 25])]
        #[case::perc_len_ratio(vec![Percentage(25), Length(25), Ratio(1, 4)], vec![25, 25, 25])]
        #[case::len_ratio_perc(vec![Length(25), Ratio(1, 4), Percentage(25)], vec![25, 25, 25])]
        #[case::ratio_len_perc(vec![Ratio(1, 4), Length(25), Percentage(25)], vec![25, 25, 25])]
        #[case::ratio_perc_len(vec![Ratio(1, 4), Percentage(25), Length(25)], vec![25, 25, 25])]
        #[case::len_len_min(vec![Length(100), Length(1), Min(20)], vec![79, 1, 20])]
        #[case::min_len_len(vec![Min(20), Length(1), Length(100)], vec![20, 1, 79])]
        #[case::fill_len_fill1(vec![Fill(1), Length(10), Fill(1)], vec![45, 10, 45])]
        #[case::fill_len_fill2(vec![Fill(1), Length(10), Fill(2)], vec![30, 10, 60])]
        #[case::fill_len_fill4(vec![Fill(1), Length(10), Fill(4)], vec![18, 10, 72])]
        #[case::fill_len_fill5(vec![Fill(1), Length(10), Fill(5)], vec![15, 10, 75])]
        #[case::len_len_len3(vec![Length(25), Length(25), Length(25)], vec![25, 25, 25])]
        fn length_is_higher_priority_in_flex(
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<u16>,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            for flex in [
                Flex::Start,
                Flex::End,
                Flex::Center,
                Flex::SpaceAround,
                Flex::SpaceEvenly,
                Flex::SpaceBetween,
            ] {
                let widths = Layout::horizontal(&constraints)
                    .flex(flex)
                    .split(rect)
                    .iter()
                    .map(|r| r.width)
                    .collect_vec();
                assert_eq!(widths, expected);
            }
        }

        #[rstest]
        #[case::fill_len_fill(vec![Fill(1), Length(10), Fill(2)], vec![0..13, 13..23, 23..50])]
        #[case::len_fill_fill(vec![Length(10), Fill(2), Fill(1)], vec![0..10, 10..37, 37..50])] // might be unstable?
        fn fixed_with_50_width(
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<Range<u16>>,
        ) {
            let rect = Rect::new(0, 0, 50, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::same_fill(vec![Fill(1), Fill(2), Fill(1), Fill(1)], vec![0..20, 20..60, 60..80, 80..100])]
        #[case::inc_fill(vec![Fill(1), Fill(2), Fill(3), Fill(4)], vec![0..10, 10..30, 30..60, 60..100])]
        #[case::dec_fill(vec![Fill(4), Fill(3), Fill(2), Fill(1)], vec![0..40, 40..70, 70..90, 90..100])]
        #[case::rand_fill1(vec![Fill(1), Fill(3), Fill(2), Fill(4)], vec![0..10, 10..40, 40..60, 60..100])]
        #[case::rand_fill2(vec![Fill(1), Fill(3), Length(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
        #[case::rand_fill3(vec![Fill(1), Fill(3), Percentage(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
        #[case::rand_fill4(vec![Fill(1), Fill(3), Min(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
        #[case::rand_fill5(vec![Fill(1), Fill(3), Max(50), Fill(2), Fill(4)], vec![0..5, 5..20, 20..70, 70..80, 80..100])]
        #[case::zero_fill1(vec![Fill(0), Fill(1), Fill(0)], vec![0..0, 0..100, 100..100])]
        #[case::zero_fill2(vec![Fill(0), Length(1), Fill(0)], vec![0..50, 50..51, 51..100])]
        #[case::zero_fill3(vec![Fill(0), Percentage(1), Fill(0)], vec![0..50, 50..51, 51..100])]
        #[case::zero_fill4(vec![Fill(0), Min(1), Fill(0)], vec![0..50, 50..51, 51..100])]
        #[case::zero_fill5(vec![Fill(0), Max(1), Fill(0)], vec![0..50, 50..51, 51..100])]
        #[case::zero_fill6(vec![Fill(0), Fill(2), Fill(0), Fill(1)], vec![0..0, 0..67, 67..67, 67..100])]
        #[case::space_fill1(vec![Fill(0), Fill(2), Percentage(20)], vec![0..0, 0..80, 80..100])]
        #[case::space_fill2(vec![Fill(0), Fill(0), Percentage(20)], vec![0..40, 40..80, 80..100])]
        #[case::space_fill3(vec![Fill(0), Ratio(1, 5)], vec![0..80, 80..100])]
        #[case::space_fill4(vec![Fill(0), Fill(u16::MAX)], vec![0..0, 0..100])]
        #[case::space_fill5(vec![Fill(u16::MAX), Fill(0)], vec![0..100, 100..100])]
        #[case::space_fill6(vec![Fill(0), Percentage(20)], vec![0..80, 80..100])]
        #[case::space_fill7(vec![Fill(1), Percentage(20)], vec![0..80, 80..100])]
        #[case::space_fill8(vec![Fill(u16::MAX), Percentage(20)], vec![0..80, 80..100])]
        #[case::space_fill9(vec![Fill(u16::MAX), Fill(0), Percentage(20)], vec![0..80, 80..80, 80..100])]
        #[case::space_fill10(vec![Fill(0), Length(20)], vec![0..80, 80..100])]
        #[case::space_fill11(vec![Fill(0), Min(20)], vec![0..80, 80..100])]
        #[case::space_fill12(vec![Fill(0), Max(20)], vec![0..80, 80..100])]
        #[case::fill_collapse1(vec![Fill(1), Fill(1), Fill(1), Min(30), Length(50)], vec![0..7, 7..13, 13..20, 20..50, 50..100])]
        #[case::fill_collapse2(vec![Fill(1), Fill(1), Fill(1), Length(50), Length(50)], vec![0..0, 0..0, 0..0, 0..50, 50..100])]
        #[case::fill_collapse3(vec![Fill(1), Fill(1), Fill(1), Length(75), Length(50)], vec![0..0, 0..0, 0..0, 0..75, 75..100])]
        #[case::fill_collapse4(vec![Fill(1), Fill(1), Fill(1), Min(50), Max(50)], vec![0..0, 0..0, 0..0, 0..50, 50..100])]
        #[case::fill_collapse5(vec![Fill(1), Fill(1), Fill(1), Ratio(1, 1)], vec![0..0, 0..0, 0..0, 0..100])]
        #[case::fill_collapse6(vec![Fill(1), Fill(1), Fill(1), Percentage(100)], vec![0..0, 0..0, 0..0, 0..100])]
        fn fill(#[case] constraints: Vec<Constraint>, #[case] expected: Vec<Range<u16>>) {
            let rect = Rect::new(0, 0, 100, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::min_percentage(vec![Min(0), Percentage(20)], vec![0..80, 80..100])]
        #[case::max_percentage(vec![Max(0), Percentage(20)], vec![0..0, 0..100])]
        fn percentage_parameterized(
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<Range<u16>>,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::max_min(vec![Max(100), Min(0)], vec![0..100, 100..100])]
        #[case::min_max(vec![Min(0), Max(100)], vec![0..0, 0..100])]
        #[case::length_min(vec![Length(u16::MAX), Min(10)], vec![0..90, 90..100])]
        #[case::min_length(vec![Min(10), Length(u16::MAX)], vec![0..10, 10..100])]
        #[case::length_max(vec![Length(0), Max(10)], vec![0..90, 90..100])]
        #[case::max_length(vec![Max(10), Length(0)], vec![0..10, 10..100])]
        fn min_max(#[case] constraints: Vec<Constraint>, #[case] expected: Vec<Range<u16>>) {
            let rect = Rect::new(0, 0, 100, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::length_legacy(vec![Length(50)], vec![0..100], Flex::Legacy)]
        #[case::length_start(vec![Length(50)], vec![0..50], Flex::Start)]
        #[case::length_end(vec![Length(50)], vec![50..100], Flex::End)]
        #[case::length_center(vec![Length(50)], vec![25..75], Flex::Center)]
        #[case::ratio_legacy(vec![Ratio(1, 2)], vec![0..100], Flex::Legacy)]
        #[case::ratio_start(vec![Ratio(1, 2)], vec![0..50], Flex::Start)]
        #[case::ratio_end(vec![Ratio(1, 2)], vec![50..100], Flex::End)]
        #[case::ratio_center(vec![Ratio(1, 2)], vec![25..75], Flex::Center)]
        #[case::percent_legacy(vec![Percentage(50)], vec![0..100], Flex::Legacy)]
        #[case::percent_start(vec![Percentage(50)], vec![0..50], Flex::Start)]
        #[case::percent_end(vec![Percentage(50)], vec![50..100], Flex::End)]
        #[case::percent_center(vec![Percentage(50)], vec![25..75], Flex::Center)]
        #[case::min_legacy(vec![Min(50)], vec![0..100], Flex::Legacy)]
        #[case::min_start(vec![Min(50)], vec![0..100], Flex::Start)]
        #[case::min_end(vec![Min(50)], vec![0..100], Flex::End)]
        #[case::min_center(vec![Min(50)], vec![0..100], Flex::Center)]
        #[case::max_legacy(vec![Max(50)], vec![0..100], Flex::Legacy)]
        #[case::max_start(vec![Max(50)], vec![0..50], Flex::Start)]
        #[case::max_end(vec![Max(50)], vec![50..100], Flex::End)]
        #[case::max_center(vec![Max(50)], vec![25..75], Flex::Center)]
        #[case::spacebetween_becomes_stretch1(vec![Min(1)], vec![0..100], Flex::SpaceBetween)]
        #[case::spacebetween_becomes_stretch2(vec![Max(20)], vec![0..100], Flex::SpaceBetween)]
        #[case::spacebetween_becomes_stretch3(vec![Length(20)], vec![0..100], Flex::SpaceBetween)]
        #[case::length_legacy2(vec![Length(25), Length(25)], vec![0..25, 25..100], Flex::Legacy)]
        #[case::length_start2(vec![Length(25), Length(25)], vec![0..25, 25..50], Flex::Start)]
        #[case::length_center2(vec![Length(25), Length(25)], vec![25..50, 50..75], Flex::Center)]
        #[case::length_end2(vec![Length(25), Length(25)], vec![50..75, 75..100], Flex::End)]
        #[case::length_spacebetween(vec![Length(25), Length(25)], vec![0..25, 75..100], Flex::SpaceBetween)]
        #[case::length_spaceevenly(vec![Length(25), Length(25)], vec![17..42, 58..83], Flex::SpaceEvenly)]
        #[case::length_spacearound(vec![Length(25), Length(25)], vec![13..38, 63..88], Flex::SpaceAround)]
        #[case::percentage_legacy(vec![Percentage(25), Percentage(25)], vec![0..25, 25..100], Flex::Legacy)]
        #[case::percentage_start(vec![Percentage(25), Percentage(25)], vec![0..25, 25..50], Flex::Start)]
        #[case::percentage_center(vec![Percentage(25), Percentage(25)], vec![25..50, 50..75], Flex::Center)]
        #[case::percentage_end(vec![Percentage(25), Percentage(25)], vec![50..75, 75..100], Flex::End)]
        #[case::percentage_spacebetween(vec![Percentage(25), Percentage(25)], vec![0..25, 75..100], Flex::SpaceBetween)]
        #[case::percentage_spaceevenly(vec![Percentage(25), Percentage(25)], vec![17..42, 58..83], Flex::SpaceEvenly)]
        #[case::percentage_spacearound(vec![Percentage(25), Percentage(25)], vec![13..38, 63..88], Flex::SpaceAround)]
        #[case::min_legacy2(vec![Min(25), Min(25)], vec![0..25, 25..100], Flex::Legacy)]
        #[case::min_start2(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::Start)]
        #[case::min_center2(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::Center)]
        #[case::min_end2(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::End)]
        #[case::min_spacebetween(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::SpaceBetween)]
        #[case::min_spaceevenly(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::SpaceEvenly)]
        #[case::min_spacearound(vec![Min(25), Min(25)], vec![0..50, 50..100], Flex::SpaceAround)]
        #[case::max_legacy2(vec![Max(25), Max(25)], vec![0..25, 25..100], Flex::Legacy)]
        #[case::max_start2(vec![Max(25), Max(25)], vec![0..25, 25..50], Flex::Start)]
        #[case::max_center2(vec![Max(25), Max(25)], vec![25..50, 50..75], Flex::Center)]
        #[case::max_end2(vec![Max(25), Max(25)], vec![50..75, 75..100], Flex::End)]
        #[case::max_spacebetween(vec![Max(25), Max(25)], vec![0..25, 75..100], Flex::SpaceBetween)]
        #[case::max_spaceevenly(vec![Max(25), Max(25)], vec![17..42, 58..83], Flex::SpaceEvenly)]
        #[case::max_spacearound(vec![Max(25), Max(25)], vec![13..38, 63..88], Flex::SpaceAround)]
        #[case::length_spaced_around(vec![Length(25), Length(25), Length(25)], vec![0..25, 38..63, 75..100], Flex::SpaceBetween)]
        #[case::one_segment_legacy(vec![Length(50)], vec![0..100], Flex::Legacy)]
        #[case::one_segment_start(vec![Length(50)], vec![0..50], Flex::Start)]
        #[case::one_segment_end(vec![Length(50)], vec![50..100], Flex::End)]
        #[case::one_segment_center(vec![Length(50)], vec![25..75], Flex::Center)]
        #[case::one_segment_spacebetween(vec![Length(50)], vec![0..100], Flex::SpaceBetween)]
        #[case::one_segment_spaceevenly(vec![Length(50)], vec![25..75], Flex::SpaceEvenly)]
        #[case::one_segment_spacearound(vec![Length(50)], vec![25..75], Flex::SpaceAround)]
        fn flex_constraint(
            #[case] constraints: Vec<Constraint>,
            #[case] expected: Vec<Range<u16>>,
            #[case] flex: Flex,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let ranges = Layout::horizontal(constraints)
                .flex(flex)
                .split(rect)
                .iter()
                .map(|r| r.left()..r.right())
                .collect_vec();
            assert_eq!(ranges, expected);
        }

        #[rstest]
        #[case::length_overlap1(vec![(0  , 20) , (20 , 20) , (40 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Start        , 0)]
        #[case::length_overlap2(vec![(0  , 20) , (19 , 20) , (38 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Start        , -1)]
        #[case::length_overlap3(vec![(21 , 20) , (40 , 20) , (59 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Center       , -1)]
        #[case::length_overlap4(vec![(42 , 20) , (61 , 20) , (80 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::End          , -1)]
        #[case::length_overlap5(vec![(0  , 20) , (19 , 20) , (38 , 62)] , vec![Length(20) , Length(20) , Length(20)] , Flex::Legacy       , -1)]
        #[case::length_overlap6(vec![(0  , 20) , (40 , 20) , (80 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::SpaceBetween , -1)]
        #[case::length_overlap7(vec![(10 , 20) , (40 , 20) , (70 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::SpaceEvenly  , -1)]
        #[case::length_overlap7(vec![(7  , 20) , (40 , 20) , (73 , 20)] , vec![Length(20) , Length(20) , Length(20)] , Flex::SpaceAround  , -1)]
        fn flex_overlap(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .flex(flex)
                .spacing(spacing)
                .split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();

            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::length_spacing(vec![(0 , 20), (20, 20) , (40, 20)], vec![Length(20), Length(20), Length(20)], Flex::Start      , 0)]
        #[case::length_spacing(vec![(0 , 20), (22, 20) , (44, 20)], vec![Length(20), Length(20), Length(20)], Flex::Start      , 2)]
        #[case::length_spacing(vec![(18, 20), (40, 20) , (62, 20)], vec![Length(20), Length(20), Length(20)], Flex::Center     , 2)]
        #[case::length_spacing(vec![(36, 20), (58, 20) , (80, 20)], vec![Length(20), Length(20), Length(20)], Flex::End        , 2)]
        #[case::length_spacing(vec![(0 , 20), (22, 20) , (44, 56)], vec![Length(20), Length(20), Length(20)], Flex::Legacy     , 2)]
        #[case::length_spacing(vec![(0 , 20), (40, 20) , (80, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceBetween, 2)]
        #[case::length_spacing(vec![(10, 20), (40, 20) , (70, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceEvenly, 2)]
        #[case::length_spacing(vec![(7, 20), (40, 20) , (73, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceAround, 2)]
        fn flex_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .flex(flex)
                .spacing(spacing)
                .split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::a(vec![(0, 25), (25, 75)], vec![Length(25), Length(25)])]
        #[case::b(vec![(0, 25), (25, 75)], vec![Length(25), Percentage(25)])]
        #[case::c(vec![(0, 75), (75, 25)], vec![Percentage(25), Length(25)])]
        #[case::d(vec![(0, 75), (75, 25)], vec![Min(25), Percentage(25)])]
        #[case::e(vec![(0, 25), (25, 75)], vec![Percentage(25), Min(25)])]
        #[case::f(vec![(0, 25), (25, 75)], vec![Min(25), Percentage(100)])]
        #[case::g(vec![(0, 75), (75, 25)], vec![Percentage(100), Min(25)])]
        #[case::h(vec![(0, 25), (25, 75)], vec![Max(75), Percentage(75)])]
        #[case::i(vec![(0, 75), (75, 25)], vec![Percentage(75), Max(75)])]
        #[case::j(vec![(0, 25), (25, 75)], vec![Max(25), Percentage(25)])]
        #[case::k(vec![(0, 75), (75, 25)], vec![Percentage(25), Max(25)])]
        #[case::l(vec![(0, 25), (25, 75)], vec![Length(25), Ratio(1, 4)])]
        #[case::m(vec![(0, 75), (75, 25)], vec![Ratio(1, 4), Length(25)])]
        #[case::n(vec![(0, 25), (25, 75)], vec![Percentage(25), Ratio(1, 4)])]
        #[case::o(vec![(0, 75), (75, 25)], vec![Ratio(1, 4), Percentage(25)])]
        #[case::p(vec![(0, 25), (25, 75)], vec![Ratio(1, 4), Fill(25)])]
        #[case::q(vec![(0, 75), (75, 25)], vec![Fill(25), Ratio(1, 4)])]
        fn constraint_specification_tests_for_priority(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .flex(Flex::Legacy)
                .split(rect)
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(r, expected);
        }

        #[rstest]
        #[case::a(vec![(0, 20), (20, 20), (40, 20)], vec![Length(20), Length(20), Length(20)], Flex::Start, 0)]
        #[case::b(vec![(18, 20), (40, 20), (62, 20)], vec![Length(20), Length(20), Length(20)], Flex::Center, 2)]
        #[case::c(vec![(36, 20), (58, 20), (80, 20)], vec![Length(20), Length(20), Length(20)], Flex::End, 2)]
        #[case::d(vec![(0, 20), (22, 20), (44, 56)], vec![Length(20), Length(20), Length(20)], Flex::Legacy, 2)]
        #[case::e(vec![(0, 20), (22, 20), (44, 56)], vec![Length(20), Length(20), Length(20)], Flex::Legacy, 2)]
        #[case::f(vec![(10, 20), (40, 20), (70, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceEvenly, 2)]
        #[case::f(vec![(7, 20), (40, 20), (73, 20)], vec![Length(20), Length(20), Length(20)], Flex::SpaceAround, 2)]
        fn constraint_specification_tests_for_priority_with_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .spacing(spacing)
                .flex(flex)
                .split(rect)
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(r, expected);
        }

        #[rstest]
        #[case::prop(vec![(0 , 10), (10, 80), (90 , 10)] , vec![Length(10), Fill(1), Length(10)], Flex::Legacy)]
        #[case::flex(vec![(0 , 10), (90 , 10)] , vec![Length(10), Length(10)], Flex::SpaceBetween)]
        #[case::prop(vec![(0 , 27), (27, 10), (37, 26), (63, 10), (73, 27)] , vec![Fill(1), Length(10), Fill(1), Length(10), Fill(1)], Flex::Legacy)]
        #[case::flex(vec![(27 , 10), (63, 10)] , vec![Length(10), Length(10)], Flex::SpaceEvenly)]
        #[case::prop(vec![(0 , 10), (10, 10), (20 , 80)] , vec![Length(10), Length(10), Fill(1)], Flex::Legacy)]
        #[case::flex(vec![(0 , 10), (10, 10)] , vec![Length(10), Length(10)], Flex::Start)]
        #[case::prop(vec![(0 , 80), (80 , 10), (90, 10)] , vec![Fill(1), Length(10), Length(10)], Flex::Legacy)]
        #[case::flex(vec![(80 , 10), (90, 10)] , vec![Length(10), Length(10)], Flex::End)]
        #[case::prop(vec![(0 , 40), (40, 10), (50, 10), (60, 40)] , vec![Fill(1), Length(10), Length(10), Fill(1)], Flex::Legacy)]
        #[case::flex(vec![(40 , 10), (50, 10)] , vec![Length(10), Length(10)], Flex::Center)]
        #[case::flex(vec![(20 , 10), (70, 10)] , vec![Length(10), Length(10)], Flex::SpaceAround)]
        fn fill_vs_flex(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints).flex(flex).split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Legacy , 0)]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , 0)]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , 0)]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , 0)]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Start , 0)]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Center , 0)]
        #[case::flex0(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::End , 0)]
        #[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::Legacy , 10)]
        #[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::Start , 10)]
        #[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::Center , 10)]
        #[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::End , 10)]
        #[case::flex10(vec![(10 , 35), (55 , 35)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , 10)]
        #[case::flex10(vec![(0 , 45), (55 , 45)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , 10)]
        #[case::flex10(vec![(10 , 30), (60 , 30)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , 10)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , 0)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , 0)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , 0)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , 0)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , 0)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , 0)]
        #[case::flex_length0(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , 0)]
        #[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , 10)]
        #[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , 10)]
        #[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , 10)]
        #[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , 10)]
        #[case::flex_length10(vec![(10 , 25), (45, 10), (65 , 25)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , 10)]
        #[case::flex_length10(vec![(0 , 35), (45, 10), (65 , 35)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , 10)]
        #[case::flex_length10(vec![(10 , 15), (45, 10), (75 , 15)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , 10)]
        fn fill_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .flex(flex)
                .spacing(spacing)
                .split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(expected, result);
        }

        #[rstest]
        #[case::flex0_1(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::Legacy , -10)]
        #[case::flex0_2(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , -10)]
        #[case::flex0_3(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , -10)]
        #[case::flex0_4(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::Start , -10)]
        #[case::flex0_5(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::Center , -10)]
        #[case::flex0_6(vec![(0 , 55), (45 , 55)] , vec![Fill(1), Fill(1)], Flex::End , -10)]
        #[case::flex0_7(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , -10)]
        #[case::flex10_1(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Legacy , -1)]
        #[case::flex10_2(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Start , -1)]
        #[case::flex10_3(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::Center , -1)]
        #[case::flex10_4(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::End , -1)]
        #[case::flex10_5(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceAround , -1)]
        #[case::flex10_6(vec![(0 , 51), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceBetween , -1)]
        #[case::flex10_7(vec![(0 , 50), (50 , 50)] , vec![Fill(1), Fill(1)], Flex::SpaceEvenly , -1)]
        #[case::flex_length0_1(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , -10)]
        #[case::flex_length0_2(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , -10)]
        #[case::flex_length0_3(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , -10)]
        #[case::flex_length0_4(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , -10)]
        #[case::flex_length0_5(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , -10)]
        #[case::flex_length0_6(vec![(0 , 55), (45, 10), (45 , 55)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , -10)]
        #[case::flex_length0_7(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , -10)]
        #[case::flex_length10_1(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::Legacy , -1)]
        #[case::flex_length10_2(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::Start , -1)]
        #[case::flex_length10_3(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::Center , -1)]
        #[case::flex_length10_4(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::End , -1)]
        #[case::flex_length10_5(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceAround , -1)]
        #[case::flex_length10_6(vec![(0 , 46), (45, 10), (54 , 46)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceBetween , -1)]
        #[case::flex_length10_7(vec![(0 , 45), (45, 10), (55 , 45)] , vec![Fill(1), Length(10), Fill(1)], Flex::SpaceEvenly , -1)]
        fn fill_overlap(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .flex(flex)
                .spacing(spacing)
                .split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::flex_length10(vec![(0, 10), (90, 10)], vec![Length(10), Length(10)], Flex::Center, 80)]
        fn flex_spacing_lower_priority_than_user_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints)
                .flex(flex)
                .spacing(spacing)
                .split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::spacers(vec![(0, 0), (10, 0), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy)]
        #[case::spacers(vec![(0, 0), (10, 80), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween)]
        #[case::spacers(vec![(0, 27), (37, 26), (73, 27)], vec![Length(10), Length(10)], Flex::SpaceEvenly)]
        #[case::spacers(vec![(0, 20), (30, 40), (80, 20)], vec![Length(10), Length(10)], Flex::SpaceAround)]
        #[case::spacers(vec![(0, 0), (10, 0), (20, 80)], vec![Length(10), Length(10)], Flex::Start)]
        #[case::spacers(vec![(0, 40), (50, 0), (60, 40)], vec![Length(10), Length(10)], Flex::Center)]
        #[case::spacers(vec![(0, 80), (90, 0), (100, 0)], vec![Length(10), Length(10)], Flex::End)]
        fn split_with_spacers_no_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let (_, s) = Layout::horizontal(&constraints)
                .flex(flex)
                .split_with_spacers(rect);
            assert_eq!(s.len(), constraints.len() + 1);
            let result = s
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::spacers(vec![(0, 0), (10, 5), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy, 5)]
        #[case::spacers(vec![(0, 0), (10, 80), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween, 5)]
        #[case::spacers(vec![(0, 27), (37, 26), (73, 27)], vec![Length(10), Length(10)], Flex::SpaceEvenly, 5)]
        #[case::spacers(vec![(0, 20), (30, 40), (80, 20)], vec![Length(10), Length(10)], Flex::SpaceAround, 5)]
        #[case::spacers(vec![(0, 0), (10, 5), (25, 75)], vec![Length(10), Length(10)], Flex::Start, 5)]
        #[case::spacers(vec![(0, 38), (48, 5), (63, 37)], vec![Length(10), Length(10)], Flex::Center, 5)]
        #[case::spacers(vec![(0, 75), (85, 5), (100, 0)], vec![Length(10), Length(10)], Flex::End, 5)]
        fn split_with_spacers_and_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let (_, s) = Layout::horizontal(&constraints)
                .flex(flex)
                .spacing(spacing)
                .split_with_spacers(rect);
            assert_eq!(s.len(), constraints.len() + 1);
            let result = s
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(expected, result);
        }

        #[rstest]
        #[case::spacers_1(vec![(0, 0), (10, 0), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy, -1)]
        #[case::spacers_2(vec![(0, 0), (10, 80), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween, -1)]
        #[case::spacers_3(vec![(0, 27), (37, 26), (73, 27)], vec![Length(10), Length(10)], Flex::SpaceEvenly, -1)]
        #[case::spacers_3(vec![(0, 20), (30, 40), (80, 20)], vec![Length(10), Length(10)], Flex::SpaceAround, -1)]
        #[case::spacers_4(vec![(0, 0), (10, 0), (19, 81)], vec![Length(10), Length(10)], Flex::Start, -1)]
        #[case::spacers_5(vec![(0, 41), (51, 0), (60, 40)], vec![Length(10), Length(10)], Flex::Center, -1)]
        #[case::spacers_6(vec![(0, 81), (91, 0), (100, 0)], vec![Length(10), Length(10)], Flex::End, -1)]
        fn split_with_spacers_and_overlap(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let (_, s) = Layout::horizontal(&constraints)
                .flex(flex)
                .spacing(spacing)
                .split_with_spacers(rect);
            assert_eq!(s.len(), constraints.len() + 1);
            let result = s
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::Legacy, 200)]
        #[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::SpaceBetween, 200)]
        #[case::spacers(vec![(0, 33), (33, 34), (67, 33)], vec![Length(10), Length(10)], Flex::SpaceEvenly, 200)]
        #[case::spacers(vec![(0, 25), (25, 50), (75, 25)], vec![Length(10), Length(10)], Flex::SpaceAround, 200)]
        #[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::Start, 200)]
        #[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::Center, 200)]
        #[case::spacers(vec![(0, 0), (0, 100), (100, 0)], vec![Length(10), Length(10)], Flex::End, 200)]
        fn split_with_spacers_and_too_much_spacing(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
            #[case] spacing: i16,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let (_, s) = Layout::horizontal(&constraints)
                .flex(flex)
                .spacing(spacing)
                .split_with_spacers(rect);
            assert_eq!(s.len(), constraints.len() + 1);
            let result = s
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }

        #[rstest]
        #[case::compare(vec![(0, 90), (90, 10)], vec![Min(10), Length(10)], Flex::Legacy)]
        #[case::compare(vec![(0, 90), (90, 10)], vec![Min(10), Length(10)], Flex::Start)]
        #[case::compare(vec![(0, 10), (10, 90)], vec![Min(10), Percentage(100)], Flex::Legacy)]
        #[case::compare(vec![(0, 10), (10, 90)], vec![Min(10), Percentage(100)], Flex::Start)]
        #[case::compare(vec![(0, 50), (50, 50)], vec![Percentage(50), Percentage(50)], Flex::Legacy)]
        #[case::compare(vec![(0, 50), (50, 50)], vec![Percentage(50), Percentage(50)], Flex::Start)]
        fn legacy_vs_default(
            #[case] expected: Vec<(u16, u16)>,
            #[case] constraints: Vec<Constraint>,
            #[case] flex: Flex,
        ) {
            let rect = Rect::new(0, 0, 100, 1);
            let r = Layout::horizontal(constraints).flex(flex).split(rect);
            let result = r
                .iter()
                .map(|r| (r.x, r.width))
                .collect::<Vec<(u16, u16)>>();
            assert_eq!(result, expected);
        }
    }
}

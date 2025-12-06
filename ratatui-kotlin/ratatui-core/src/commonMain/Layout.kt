#![warn(clippy::missing_const_for_fn)]
//! Layout and positioning in terminal user interfaces.
//!
//! This module provides a comprehensive set of types and traits for working with layout and
//! positioning in terminal applications. It implements a flexible layout system that allows you to
//! divide the terminal screen into different areas using constraints, manage positioning and
//! sizing, and handle complex UI arrangements.
//!
//! The layout system in Ratatui is based on the Cassowary constraint solver algorithm, implemented
//! through the [`kasuari`] crate. This allows for sophisticated constraint-based layouts where
//! multiple requirements can be satisfied simultaneously, with priorities determining which
//! constraints take precedence when conflicts arise.
//!
//! [`kasuari`]: https://crates.io/crates/kasuari
//!
//! # Core Concepts
//!
//! ## Coordinate System
//!
//! The coordinate system runs left to right, top to bottom, with the origin `(0, 0)` in the top
//! left corner of the terminal. The x and y coordinates are represented by `u16` values.
//!
//! ```text
//!      x (columns)
//!   ┌─────────────→
//! y │ (0,0)
//!   │
//! (rows)
//!   ↓
//! ```
//!
//! ## Layout Fundamentals
//!
//! Layouts form the structural foundation of your terminal UI. The [`Layout`] struct divides
//! available screen space into rectangular areas using a constraint-based approach. You define
//! multiple constraints for how space should be allocated, and the Cassowary solver determines
//! the optimal layout that satisfies as many constraints as possible. These areas can then be
//! used to render widgets or nested layouts.
//!
//! Note that the [`Layout`] struct is not required to create layouts - you can also manually
//! calculate and create [`Rect`] areas using simple mathematics to divide up the terminal space
//! if you prefer direct control over positioning and sizing.
//!
//! ## Rectangular Areas
//!
//! All layout operations work with rectangular areas represented by the [`Rect`] type. A [`Rect`]
//! defines a position and size in the terminal, specified by its top-left corner coordinates and
//! dimensions.
//!
//! # Available Types
//!
//! ## Core Layout Types
//!
//! - [`Layout`] - The primary layout engine that divides space using constraints and direction
//! - [`Rect`] - Represents a rectangular area with position and dimensions
//! - [`Constraint`] - Defines how space should be allocated (length, percentage, ratio, etc.)
//! - [`Direction`] - Specifies layout orientation (horizontal or vertical)
//! - [`Flex`] - Controls space distribution when constraints are satisfied
//!
//! ## Positioning and Sizing
//!
//! - [`Position`] - Represents a point in the terminal coordinate system
//! - [`Size`] - Represents dimensions (width and height)
//! - [`Margin`] - Defines spacing around rectangular areas
//! - [`Offset`] - Represents relative movement in the coordinate system
//! - [`Spacing`] - Controls spacing or overlap between layout segments
//!
//! ## Alignment
//!
//! - [`Alignment`] (alias for [`HorizontalAlignment`]) - Horizontal text/content alignment
//! - [`HorizontalAlignment`] - Horizontal alignment options (left, center, right)
//! - [`VerticalAlignment`] - Vertical alignment options (top, center, bottom)
//!
//! ## Iteration Support
//!
//! - [`Rows`] - Iterator over horizontal rows within a rectangular area
//! - [`Columns`] - Iterator over vertical columns within a rectangular area
//! - [`Positions`] - Iterator over all positions within a rectangular area
//!
//! # Quick Start
//!
//! Here's a simple example of creating a basic layout using the [`Layout`] struct:
//!
//! ```rust
//! use ratatui_core::layout::{Constraint, Direction, Layout, Rect};
//!
//! // Create a terminal area
//! let area = Rect::new(0, 0, 80, 24);
//!
//! // Divide it vertically into two equal parts using Layout
//! let layout = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);
//! let [top, bottom] = layout.areas(area);
//!
//! // Now you have two areas: top and bottom
//! ```
//!
//! **Note**: When the number of layout areas is known at compile time, use destructuring
//! assignment with descriptive variable names for better readability:
//!
//! ```rust
//! use ratatui_core::layout::{Constraint, Layout, Rect};
//!
//! let area = Rect::new(0, 0, 80, 24);
//! let [header, content, footer] = Layout::vertical([
//!     Constraint::Length(3),
//!     Constraint::Fill(1),
//!     Constraint::Length(1),
//! ])
//! .areas(area);
//! ```
//!
//! Use [`Layout::split`] when the number of areas is only known at runtime.
//!
//! Alternatively, you can create layouts manually using mathematics:
//!
//! ```rust
//! use ratatui_core::layout::Rect;
//!
//! // Create a terminal area
//! let area = Rect::new(0, 0, 80, 24);
//!
//! // Manually divide into two equal parts
//! let top_half = Rect::new(area.x, area.y, area.width, area.height / 2);
//! let bottom_half = Rect::new(
//!     area.x,
//!     area.y + area.height / 2,
//!     area.width,
//!     area.height / 2,
//! );
//! ```
//!
//! # Layout Examples
//!
//! ## Basic Vertical Split
//!
//! ```rust
//! use ratatui_core::layout::{Constraint, Layout, Rect};
//!
//! let area = Rect::new(0, 0, 80, 24);
//! let [header, content, footer] = Layout::vertical([
//!     Constraint::Length(3), // Header: fixed height
//!     Constraint::Fill(1),   // Content: flexible
//!     Constraint::Length(1), // Footer: fixed height
//! ])
//! .areas(area);
//! ```
//!
//! ## Horizontal Sidebar Layout
//!
//! ```rust
//! use ratatui_core::layout::{Constraint, Layout, Rect};
//!
//! let area = Rect::new(0, 0, 80, 24);
//! let [sidebar, main] = Layout::horizontal([
//!     Constraint::Length(20), // Sidebar: fixed width
//!     Constraint::Fill(1),    // Main content: flexible
//! ])
//! .areas(area);
//! ```
//!
//! ## Complex Nested Layout
//!
//! ```rust
//! use ratatui_core::layout::{Constraint, Layout, Rect};
//!
//! fn create_complex_layout(area: Rect) -> [Rect; 4] {
//!     // First, split vertically
//!     let [header, body, footer] = Layout::vertical([
//!         Constraint::Length(3), // Header
//!         Constraint::Fill(1),   // Body
//!         Constraint::Length(1), // Footer
//!     ])
//!     .areas(area);
//!
//!     // Then split the body horizontally
//!     let [sidebar, main] = Layout::horizontal([
//!         Constraint::Length(20), // Sidebar
//!         Constraint::Fill(1),    // Main
//!     ])
//!     .areas(body);
//!
//!     [header, sidebar, main, footer]
//! }
//! ```
//!
//! # Working with Constraints
//!
//! [`Constraint`]s define how space is allocated within a layout using the Cassowary constraint
//! solver algorithm. The constraint solver attempts to satisfy all constraints simultaneously,
//! with priorities determining which constraints take precedence when conflicts arise. Different
//! constraint types serve different purposes:
//!
//! - [`Constraint::Min`] - Minimum size constraint
//! - [`Constraint::Max`] - Maximum size constraint
//! - [`Constraint::Length`] - Fixed size in character cells
//! - [`Constraint::Percentage`] - Relative size as a percentage of available space
//! - [`Constraint::Ratio`] - Proportional size using ratios
//! - [`Constraint::Fill`] - Proportional fill of remaining space
//!
//! Constraints are resolved in priority order, with [`Constraint::Min`] having the highest
//! priority and [`Constraint::Fill`] having the lowest. The constraint solver will satisfy as
//! many constraints as possible while respecting these priorities.
//!
//! # Flexible Space Distribution
//!
//! The [`Flex`] enum controls how extra space is distributed when constraints are satisfied:
//!
//! - [`Flex::Start`] - Align content to the start, leaving excess space at the end
//! - [`Flex::End`] - Align content to the end, leaving excess space at the start
//! - [`Flex::Center`] - Center content, distributing excess space equally on both sides
//! - [`Flex::SpaceBetween`] - Distribute excess space evenly *between* elements, none at the ends
//! - [`Flex::SpaceAround`] - Distribute space *around* elements: equal padding on both sides of
//!   each element; gaps between elements are twice the edge spacing
//! - [`Flex::SpaceEvenly`] - Distribute space *evenly*: equal spacing between all elements,
//!   including before the first and after the last.
//! - [`Flex::Legacy`] - Legacy behavior (puts excess space in the last element)
//!
//! # Positioning and Alignment
//!
//! Use [`Position`] to represent specific points in the terminal, [`Size`] for dimensions, and the
//! alignment types for controlling content positioning within areas:
//!
//! ```rust
//! use ratatui_core::layout::{Alignment, Position, Rect, Size};
//!
//! let pos = Position::new(10, 5);
//! let size = Size::new(80, 24);
//! let rect = Rect::new(pos.x, pos.y, size.width, size.height);
//!
//! // Alignment for content within areas
//! let center = Alignment::Center;
//! ```
//!
//! # Advanced Features
//!
//! ## Margins and Spacing
//!
//! Add spacing around areas using uniform margins or between layout segments using [`Spacing`]:
//!
//! ```rust
//! use ratatui_core::layout::{Constraint, Layout, Margin, Rect, Spacing};
//!
//! let layout = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
//!     .margin(2) // 2-cell margin on all sides
//!     .spacing(Spacing::Space(1)); // 1-cell spacing between segments
//!
//! // For asymmetric margins, use the Rect inner method directly
//! let area = Rect::new(0, 0, 80, 24).inner(Margin::new(2, 1));
//! ```
//!
//! ## Area Iteration
//!
//! Iterate over rows, columns, or all positions within a rectangular area. The `rows()` and
//! `columns()` iterators return full [`Rect`] regions that can be used to render widgets or
//! passed to other layout methods for more complex nested layouts. The `positions()` iterator
//! returns [`Position`] values representing individual cell coordinates:
//!
//! ```rust
//! use ratatui_core::buffer::Buffer;
//! use ratatui_core::layout::{Constraint, Layout, Rect};
//! use ratatui_core::widgets::Widget;
//!
//! let area = Rect::new(0, 0, 20, 10);
//! let mut buffer = Buffer::empty(area);
//!
//! // Renders "Row 0", "Row 1", etc. in each horizontal row
//! for (i, row) in area.rows().enumerate() {
//!     format!("Row {i}").render(row, &mut buffer);
//! }
//!
//! // Renders column indices (0-9 repeating) in each vertical column
//! for (i, col) in area.columns().enumerate() {
//!     format!("{}", i % 10).render(col, &mut buffer);
//! }
//!
//! // Renders position indices (0-9 repeating) at each cell position
//! for (i, pos) in area.positions().enumerate() {
//!     buffer[pos].set_symbol(&format!("{}", i % 10));
//! }
//! ```
//!
//! # Performance Considerations
//!
//! The layout system includes optional caching to improve performance for repeated layout
//! calculations. Layout caching is enabled by default in the main `ratatui` crate, but requires
//! explicitly enabling the `layout-cache` feature when using `ratatui-core` directly. When
//! enabled, layout results are cached based on the area and layout configuration.
//!
//! # Related Documentation
//!
//! For more detailed information and practical examples:
//!
//! - [Layout Concepts](https://ratatui.rs/concepts/layout/) - Comprehensive guide to layout
//!   concepts
//! - [Layout Recipes](https://ratatui.rs/recipes/layout/) - Practical layout examples and patterns
//! - [Grid Layout Recipe](https://ratatui.rs/recipes/layout/grid/) - Creating grid-based layouts
//! - [Center a Widget Recipe](https://ratatui.rs/recipes/layout/center-a-widget/) - Centering
//!   content
//! - [Dynamic Layouts Recipe](https://ratatui.rs/recipes/layout/dynamic/) - Creating responsive
//!   layouts
//!
//! # Examples
//!
//! See the Ratatui repository for complete examples:
//!
//! - [`constraints`](https://github.com/ratatui/ratatui/blob/main/examples/apps/constraints/) -
//!   Demonstrates different constraint types
//! - [`flex`](https://github.com/ratatui/ratatui/blob/main/examples/apps/flex/) - Shows flex space
//!   distribution
//! - [`layout`](https://github.com/ratatui/ratatui/blob/main/examples/apps/layout/) - Basic layout
//!   examples

mod alignment;
mod constraint;
mod direction;
mod flex;
mod layout;
mod margin;
mod offset;
mod position;
mod rect;
mod size;

pub use alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
pub use constraint::Constraint;
pub use direction::Direction;
pub use flex::Flex;
pub use layout::{Layout, Spacing};
pub use margin::Margin;
pub use offset::Offset;
pub use position::Position;
pub use rect::{Columns, Positions, Rect, Rows};
pub use size::Size;

#![no_std]
// show the feature flags in the generated documentation
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/logo.png",
    html_favicon_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/favicon.ico"
)]
#![warn(missing_docs)]
//! **ratatui-widgets** contains all the widgets that were previously part of the [Ratatui] crate.
//! It is meant to be used in conjunction with `ratatui`, which provides the core functionality for
//! building terminal user interfaces.
//!
//! [Ratatui]: https://crates.io/crates/ratatui
//!
//! Most applications shouldn't need to depend directly on `ratatui-widgets`, `ratatui` crate
//! re-exports all the widgets from this crate. However, if you are building a widget library that
//! internally uses these widgets, or if you prefer finer grained dependencies, you may want to
//! depend on this crate rather than transitively through the `ratatui` crate.
//!
//! Previously, a crate named `ratatui-widgets` was published with some formative ideas about an
//! eventual Ratatui framework. That crate has now moved to [tui-framework-experiment], pending a
//! new name.
//!
//! [tui-framework-experiment]: https://crates.io/crates/tui-framework-experiment
//!
//! # Installation
//!
//! Run the following command to add this crate to your project:
//!
//! ```sh
//! cargo add ratatui-widgets
//! ```
//!
//! # Available Widgets
//!
//! - [`BarChart`]: displays multiple datasets as bars with optional grouping.
//! - [`Block`]: a basic widget that draws a block with optional borders, titles, and styles.
//! - [`calendar::Monthly`]: displays a single month.
//! - [`Canvas`]: draws arbitrary shapes using drawing characters.
//! - [`Chart`]: displays multiple datasets as lines or scatter graphs.
//! - [`Clear`]: clears the area it occupies. Useful to render over previously drawn widgets.
//! - [`Gauge`]: displays progress percentage using block characters.
//! - [`LineGauge`]: displays progress as a line.
//! - [`List`]: displays a list of items and allows selection.
//! - [`RatatuiLogo`]: displays the Ratatui logo.
//! - [`RatatuiMascot`]: displays the Ratatui mascot.
//! - [`Paragraph`]: displays a paragraph of optionally styled and wrapped text.
//! - [`Scrollbar`]: displays a scrollbar.
//! - [`Sparkline`]: displays a single dataset as a sparkline.
//! - [`Table`]: displays multiple rows and columns in a grid and allows selection.
//! - [`Tabs`]: displays a tab bar and allows selection.
//!
//! [`BarChart`]: crate::barchart::BarChart
//! [`Block`]: crate::block::Block
//! [`calendar::Monthly`]: crate::calendar::Monthly
//! [`Canvas`]: crate::canvas::Canvas
//! [`Chart`]: crate::chart::Chart
//! [`Clear`]: crate::clear::Clear
//! [`Gauge`]: crate::gauge::Gauge
//! [`LineGauge`]: crate::gauge::LineGauge
//! [`List`]: crate::list::List
//! [`RatatuiLogo`]: crate::logo::RatatuiLogo
//! [`RatatuiMascot`]: crate::mascot::RatatuiMascot
//! [`Paragraph`]: crate::paragraph::Paragraph
//! [`Scrollbar`]: crate::scrollbar::Scrollbar
//! [`Sparkline`]: crate::sparkline::Sparkline
//! [`Table`]: crate::table::Table
//! [`Tabs`]: crate::tabs::Tabs
//!
//! All these widgets are re-exported directly under `ratatui::widgets` in the `ratatui` crate.
//!
//! # Crate Organization
//!
//! `ratatui-widgets` is part of the Ratatui workspace that was modularized in version 0.30.0.
//! This crate contains all the built-in widget implementations that were previously part of the
//! main `ratatui` crate.
//!
//! **When to use `ratatui-widgets`:**
//!
//! - Building widget libraries that need to compose with built-in widgets
//! - You want finer-grained dependencies and only need specific widgets
//! - Creating custom widgets that extend or wrap the built-in ones
//!
//! **When to use the main [`ratatui`] crate:**
//!
//! - Building applications (recommended - includes everything you need)
//! - You want the convenience of having all widgets available
//!
//! For detailed information about the workspace organization, see [ARCHITECTURE.md].
//!
//! [`ratatui`]: https://crates.io/crates/ratatui
//! [ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md
#![cfg_attr(feature = "document-features", doc = "\n## Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
//!
//! # Contributing
//!
//! Contributions are welcome! Please open an issue or submit a pull request on GitHub. For more
//! details on contributing, please see the [CONTRIBUTING](CONTRIBUTING.md) document.
//!
//! # License
//!
//! This project is licensed under the MIT License. See the [LICENSE](../LICENSE) file for details.

#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::alloc_instead_of_core)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod barchart;
pub mod block;
pub mod borders;
pub mod canvas;
pub mod chart;
pub mod clear;
pub mod gauge;
pub mod list;
pub mod logo;
pub mod mascot;
pub mod paragraph;
pub mod scrollbar;
pub mod sparkline;
pub mod table;
pub mod tabs;

#[cfg(not(feature = "std"))]
mod polyfills;
mod reflow;

#[cfg(feature = "calendar")]
pub mod calendar;

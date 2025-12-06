//! Primitives for styled text.
//!
//! A terminal UI is at its root a lot of strings. In order to make it accessible and stylish, those
//! strings may be associated to a set of styles. `ratatui` has three ways to represent them:
//! - A single line string where all graphemes have the same style is represented by a [`Span`].
//! - A single line string where each grapheme may have its own style is represented by [`Line`].
//! - A multiple line string where each grapheme may have its own style is represented by a
//!   [`Text`].
//!
//! These types form a hierarchy: [`Line`] is a collection of [`Span`] and each line of [`Text`] is
//! a [`Line`].
//!
//! Keep it mind that a lot of widgets will use those types to advertise what kind of string is
//! supported for their properties. Moreover, `ratatui` provides convenient `From` implementations
//! so that you can start by using simple `String` or `&str` and then promote them to the previous
//! primitives when you need additional styling capabilities.
//!
//! For example, for the `Block` widget, all the following calls are valid to set its `title`
//! property (which is a [`Line`] under the hood):
//!
//! ```rust,ignore
//! use ratatui_core::{
//!     style::{Color, Style},
//!     text::{Line, Span},
//!     widgets::Block,
//! };
//!
//! // A simple string with no styling.
//! // Converted to Line(vec![
//! //   Span { content: Cow::Borrowed("My title"), style: Style { .. } }
//! // ])
//! let block = Block::new().title("My title");
//!
//! // A simple string with a unique style.
//! // Converted to Line(vec![
//! //   Span { content: Cow::Borrowed("My title"), style: Style { fg: Some(Color::Yellow), .. }
//! // ])
//! let block = Block::new().title(Span::styled("My title", Style::default().fg(Color::Yellow)));
//!
//! // A string with multiple styles.
//! // Converted to Line(vec![
//! //   Span { content: Cow::Borrowed("My"), style: Style { fg: Some(Color::Yellow), .. } },
//! //   Span { content: Cow::Borrowed(" title"), .. }
//! // ])
//! let block = Block::new().title(vec![
//!     Span::styled("My", Style::default().fg(Color::Yellow)),
//!     Span::raw(" title"),
//! ]);
//! ```

mod grapheme;
pub use grapheme::StyledGrapheme;

mod line;
pub use line::{Line, ToLine};

mod masked;
pub use masked::Masked;

mod span;
pub use span::{Span, ToSpan};

mod text;
pub use text::{Text, ToText};

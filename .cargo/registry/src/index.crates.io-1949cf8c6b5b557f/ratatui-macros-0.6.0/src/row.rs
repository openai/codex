/// A macro for creating a [`Row`] using vec! syntax.
///
/// `row!` is similar to the [`vec!`] macro, but it returns a [`Row`] instead of a `Vec`.
///
/// # Examples
///
/// * Create a [`Row`] containing a vector of [`Cell`]s:
///
/// ```rust
/// # use ratatui::prelude::*;
/// use ratatui_macros::row;
///
/// let row = row!["hello", "world"];
/// let row = row!["hello".red(), "world".red().bold()];
/// ```
///
/// * Create an empty [`Row`]:
///
/// ```rust
/// # use ratatui::prelude::*;
/// # use ratatui_macros::row;
/// let empty_row = row![];
/// ```
///
/// * Create a [`Row`] from a given [`Cell`] repeated some amount of times:
///
/// ```rust
/// # use ratatui::prelude::*;
/// # use ratatui_macros::row;
/// let row = row!["hello"; 2];
/// ```
///
/// * Use [`text!`], [`line!`] or [`span!`] macro inside [`row!`] macro.
///
/// ```rust
/// # use ratatui::prelude::*;
/// use ratatui_macros::{row, line, text, span};
///
/// let row = row![
///     line!["hello", "world"], span!(Modifier::BOLD; "goodbye {}", "world"),
///     text!["hello", "world"],
/// ];
/// ```
///
/// [`Row`]: crate::widgets::Row
/// [`Cell`]: crate::widgets::Cell
#[macro_export]
macro_rules! row {
    () => {
        ::ratatui::widgets::Row::default()
    };
    ($cell:expr; $n:expr) => {
        ::ratatui::widgets::Row::new(vec![::ratatui::widgets::Cell::from($cell); $n])
    };
    ($($cell:expr),+ $(,)?) => {{
        ::ratatui::widgets::Row::new(vec![
        $(
            ::ratatui::widgets::Cell::from($cell),
        )+
        ])
    }};
}

#[cfg(test)]
mod tests {

    use ratatui::{
        text::Text,
        widgets::{Cell, Row},
    };

    #[test]
    fn row_literal() {
        let row = row!["hello", "world"];
        assert_eq!(
            row,
            Row::new(vec![Cell::from("hello"), Cell::from("world")])
        );
    }

    #[test]
    fn row_empty() {
        let row = row![];
        assert_eq!(row, Row::default());
    }

    #[test]
    fn row_single_cell() {
        let row = row![Cell::from("foo")];
        assert_eq!(row, Row::new(vec![Cell::from("foo")]));
    }

    #[test]
    fn row_repeated_cell() {
        let row = row![Cell::from("foo"); 2];
        assert_eq!(row, Row::new(vec![Cell::from("foo"), Cell::from("foo")]));
    }

    #[test]
    fn row_explicit_use_of_span_and_line() {
        let row = row![crate::line!("hello"), crate::span!["world"]];
        assert_eq!(
            row,
            Row::new(vec![Cell::from("hello"), Cell::from("world")])
        );
    }

    #[test]
    fn row_vec_count_syntax() {
        let row = row!["hello"; 2];
        assert_eq!(
            row,
            Row::new(vec![Cell::from("hello"), Cell::from("hello")])
        );
    }

    #[test]
    fn multiple_rows() {
        use crate::text;
        let rows = [
            row!["Find File", text!["ctrl+f"].right_aligned()],
            row!["Open recent", text!["ctrl+r"].right_aligned()],
            row!["Open config", text!["ctrl+k"].right_aligned()],
        ];
        assert_eq!(
            rows,
            [
                Row::new([
                    Cell::from("Find File"),
                    Cell::from(Text::raw("ctrl+f").alignment(ratatui::layout::Alignment::Right)),
                ]),
                Row::new([
                    Cell::from("Open recent"),
                    Cell::from(Text::raw("ctrl+r").alignment(ratatui::layout::Alignment::Right)),
                ]),
                Row::new([
                    Cell::from("Open config"),
                    Cell::from(Text::raw("ctrl+k").alignment(ratatui::layout::Alignment::Right)),
                ]),
            ]
        );
    }
}

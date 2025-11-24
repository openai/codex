/// A macro for creating a [`Text`] using vec! syntax.
///
/// `text!` is similar to the [`vec!`] macro, but it returns a [`Text`] instead of a `Vec`.
///
/// # Examples
///
/// * Create a [`Text`] containing a vector of [`Line`]s:
///
/// ```rust
/// # use ratatui::prelude::*;
/// use ratatui_macros::text;
///
/// let text = text!["hello", "world"];
/// let text = text!["hello".red(), "world".red().bold()];
/// ```
///
/// * Create a [`text`] from a given [`Line`] repeated some amount of times:
///
/// ```rust
/// # use ratatui::prelude::*;
/// # use ratatui_macros::text;
/// let text = text!["hello"; 2];
/// ```
///
/// * Use [`line!`] or [`span!`] macro inside [`text!`] macro.
///
/// ```rust
/// # use ratatui::prelude::*;
/// use ratatui_macros::{line, text, span};
///
/// let text = text![line!["hello", "world"], span!(Modifier::BOLD; "goodbye {}", "world")];
/// ```
///
/// [`Text`]: crate::text::Text
/// [`Line`]: crate::text::Line
/// [`Span`]: crate::text::Span
#[macro_export]
macro_rules! text {
    () => {
        ratatui::text::Text::default()
    };
    ($line:expr; $n:expr) => {
        ratatui::text::Text::from(vec![$line.into(); $n])
    };
    ($($line:expr),+ $(,)?) => {{
        ratatui::text::Text::from(vec![
        $(
            $line.into(),
        )+
        ])
    }};
}

#[cfg(test)]
mod tests {
    use ratatui::prelude::*;

    #[test]
    fn text() {
        // literal
        let text = text!["hello", "world"];
        assert_eq!(text, Text::from(vec!["hello".into(), "world".into()]));

        // explicit use of span and line
        let text = text![crate::line!("hello"), crate::span!["world"]];
        assert_eq!(text, Text::from(vec!["hello".into(), "world".into()]));

        // vec count syntax
        let text = text!["hello"; 2];
        assert_eq!(text, Text::from(vec!["hello".into(), "hello".into()]));
    }
}

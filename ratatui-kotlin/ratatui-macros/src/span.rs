/// A macro for creating a [`Span`] using formatting syntax.
///
/// `span!` is similar to the [`format!`] macro, but it returns a [`Span`] instead of a `String`. In
/// addition, it also accepts an expression for the first argument, which will be converted to a
/// string using the [`format!`] macro.
///
/// If semicolon follows the first argument, then the first argument is a [`Style`] and a styled
/// [`Span`] will be created. Otherwise, the [`Span`] will be created as a raw span (i.e. with style
/// set to `Style::default()`).
///
/// # Examples
///
/// ```rust
/// # use ratatui_core::style::{Color, Modifier, Style, Stylize};
/// use ratatui_macros::span;
///
/// let content = "content";
///
/// // expression
/// let span = span!(content);
///
/// // format string
/// let span = span!("test content");
/// let span = span!("test {}", "content");
/// let span = span!("{} {}", "test", "content");
/// let span = span!("test {content}");
/// let span = span!("test {content}", content = "content");
///
/// // with format specifiers
/// let span = span!("test {:4}", 123);
/// let span = span!("test {:04}", 123);
///
/// let style = Style::new().green();
///
/// // styled expression
/// let span = span!(style; content);
///
/// // styled format string
/// let span = span!(style; "test content");
/// let span = span!(style; "test {}", "content");
/// let span = span!(style; "{} {}", "test", "content");
/// let span = span!(style; "test {content}");
/// let span = span!(style; "test {content}", content = "content");
///
/// // accepts any type that is convertible to Style
/// let span = span!(Style::new().green(); "test {content}");
/// let span = span!(Color::Green; "test {content}");
/// let span = span!(Modifier::BOLD; "test {content}");
///
/// // with format specifiers
/// let span = span!(style; "test {:4}", 123);
/// let span = span!(style; "test {:04}", 123);
/// ```
///
/// # Note
///
/// The first parameter must be a formatting specifier followed by a comma OR anything that can be
/// converted into a [`Style`] followed by a semicolon.
///
/// For example, the following will fail to compile:
///
/// ```compile_fail
/// # use ratatui::prelude::*;
/// # use ratatui_macros::span;
/// let span = span!(Modifier::BOLD, "hello world");
/// ```
///
/// But this will work:
///
/// ```rust
/// # use ratatui_core::style::{Modifier};
/// # use ratatui_macros::span;
/// let span = span!(Modifier::BOLD; "hello world");
/// ```
///
/// The following will fail to compile:
///
/// ```compile_fail
/// # use ratatui::prelude::*;
/// # use ratatui_macros::span;
/// let span = span!("hello", "world");
/// ```
///
/// But this will work:
///
/// ```rust
/// # use ratatui_macros::span;
/// let span = span!("hello {}", "world");
/// ```
///
/// [`Color`]: ratatui_core::style::Color
/// [`Span`]: ratatui_core::text::Span
/// [`Style`]: ratatui_core::style::Style
/// [`format!`]: alloc::format!
#[macro_export]
macro_rules! span {
    ($string:literal) => {
        $crate::ratatui_core::text::Span::raw($crate::format!($string))
    };
    ($string:literal, $($arg:tt)*) => {
        $crate::ratatui_core::text::Span::raw($crate::format!($string, $($arg)*))
    };
    ($expr:expr) => {
        $crate::ratatui_core::text::Span::raw($crate::format!("{}", $expr))
    };
    ($style:expr, $($arg:tt)*) => {
        compile_error!("first parameter must be a formatting specifier followed by a comma OR a `Style` followed by a semicolon")
    };
    ($style:expr; $string:literal) => {
        $crate::ratatui_core::text::Span::styled($crate::format!($string), $style)
    };
    ($style:expr; $string:literal, $($arg:tt)*) => {
        $crate::ratatui_core::text::Span::styled($crate::format!($string, $($arg)*), $style)
    };
    ($style:expr; $expr:expr) => {
        $crate::ratatui_core::text::Span::styled($crate::format!("{}", $expr), $style)
    };
}

#[cfg(test)]
mod tests {
    use ratatui_core::{
        style::{Color, Modifier, Style},
        text::Span,
    };

    #[test]
    fn raw() {
        let test = "test";
        let content = "content";
        let number = 123;

        // literal
        let span = span!("test content");
        assert_eq!(span, Span::raw("test content"));

        // string
        let span = span!("test {}", "content");
        assert_eq!(span, Span::raw("test content"));

        // string variable
        let span = span!("test {}", content);
        assert_eq!(span, Span::raw("test content"));

        // string variable in the format string
        let span = span!("test {content}");
        assert_eq!(span, Span::raw("test content"));

        // named variable
        let span = span!("test {content}", content = "content");
        assert_eq!(span, Span::raw("test content"));

        // named variable pointing at a local variable
        let span = span!("test {content}", content = content);
        assert_eq!(span, Span::raw("test content"));

        // two strings
        let span = span!("{} {}", "test", "content");
        assert_eq!(span, Span::raw("test content"));

        // two string variables
        let span = span!("{test} {content}");
        assert_eq!(span, Span::raw("test content"));

        // a number
        let span = span!("test {number}");
        assert_eq!(span, Span::raw("test 123"));

        // a number with a format specifier
        let span = span!("test {number:04}");
        assert_eq!(span, Span::raw("test 0123"));

        // directly pass a number expression
        let span = span!(number);
        assert_eq!(span, Span::raw("123"));

        // directly pass a string expression
        let span = span!(test);
        assert_eq!(span, Span::raw("test"));
    }

    #[test]
    fn styled() {
        const STYLE: Style = Style::new().fg(Color::Green);

        let test = "test";
        let content = "content";
        let number = 123;

        // literal
        let span = span!(STYLE; "test content");
        assert_eq!(span, Span::styled("test content", STYLE));

        // string
        let span = span!(STYLE; "test {}", "content");
        assert_eq!(span, Span::styled("test content", STYLE));

        // string variable
        let span = span!(STYLE; "test {}", content);
        assert_eq!(span, Span::styled("test content", STYLE));

        // string variable in the format string
        let span = span!(STYLE; "test {content}");
        assert_eq!(span, Span::styled("test content", STYLE));

        // named variable
        let span = span!(STYLE; "test {content}", content = "content");
        assert_eq!(span, Span::styled("test content", STYLE));

        // named variable pointing at a local variable
        let span = span!(STYLE; "test {content}", content = content);
        assert_eq!(span, Span::styled("test content", STYLE));

        // two strings
        let span = span!(STYLE; "{} {}", "test", "content");
        assert_eq!(span, Span::styled("test content", STYLE));

        // two string variables
        let span = span!(STYLE; "{test} {content}");
        assert_eq!(span, Span::styled("test content", STYLE));

        // a number
        let span = span!(STYLE; "test {number}");
        assert_eq!(span, Span::styled("test 123", STYLE));

        // a number with a format specifier
        let span = span!(STYLE; "test {number:04}");
        assert_eq!(span, Span::styled("test 0123", STYLE));

        // accepts any type that is convertible to Style
        let span = span!(Color::Green; "test {content}");
        assert_eq!(span, Span::styled("test content", STYLE));

        let span = span!(Modifier::BOLD; "test {content}");
        assert_eq!(span, Span::styled("test content", Style::new().bold()));

        // directly pass a number expression
        let span = span!(STYLE; number);
        assert_eq!(span, Span::styled("123", STYLE));

        // directly pass a string expression
        let span = span!(STYLE; test);
        assert_eq!(span, Span::styled("test", STYLE));
    }
}

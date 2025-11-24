//! Terminal [`Styles`] for help and error output

pub use anstyle::*;

/// Terminal styling definitions
///
/// See also [`Command::styles`][crate::Command::styles].
///
/// # Example
///
/// clap v3 styling
/// ```rust
/// # use clap_builder as clap;
/// # use clap::builder::styling::*;
/// let styles = Styles::styled()
///     .header(AnsiColor::Yellow.on_default())
///     .usage(AnsiColor::Green.on_default())
///     .literal(AnsiColor::Green.on_default())
///     .placeholder(AnsiColor::Green.on_default());
/// ```
#[derive(Clone, Debug)]
#[allow(missing_copy_implementations)] // Large enough type that I want an explicit `clone()` for now
pub struct Styles {
    header: Style,
    error: Style,
    usage: Style,
    literal: Style,
    placeholder: Style,
    valid: Style,
    invalid: Style,
    context: Style,
    context_value: Option<Style>,
}

impl Styles {
    /// No terminal styling
    pub const fn plain() -> Self {
        Self {
            header: Style::new(),
            error: Style::new(),
            usage: Style::new(),
            literal: Style::new(),
            placeholder: Style::new(),
            valid: Style::new(),
            invalid: Style::new(),
            context: Style::new(),
            context_value: None,
        }
    }

    /// Default terminal styling
    pub const fn styled() -> Self {
        #[cfg(feature = "color")]
        {
            Self {
                header: Style::new().bold().underline(),
                error: Style::new()
                    .fg_color(Some(Color::Ansi(AnsiColor::Red)))
                    .bold(),
                usage: Style::new().bold().underline(),
                literal: Style::new().bold(),
                placeholder: Style::new(),
                valid: Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))),
                invalid: Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
                context: Style::new(),
                context_value: None,
            }
        }
        #[cfg(not(feature = "color"))]
        {
            Self::plain()
        }
    }

    /// General Heading style, e.g. [`help_heading`][crate::Arg::help_heading]
    #[inline]
    pub const fn header(mut self, style: Style) -> Self {
        self.header = style;
        self
    }

    /// Error heading
    #[inline]
    pub const fn error(mut self, style: Style) -> Self {
        self.error = style;
        self
    }

    /// Usage heading
    #[inline]
    pub const fn usage(mut self, style: Style) -> Self {
        self.usage = style;
        self
    }

    /// Literal command-line syntax, e.g. `--help`
    #[inline]
    pub const fn literal(mut self, style: Style) -> Self {
        self.literal = style;
        self
    }

    /// Descriptions within command-line syntax, e.g. [`value_name`][crate::Arg::value_name]
    #[inline]
    pub const fn placeholder(mut self, style: Style) -> Self {
        self.placeholder = style;
        self
    }

    /// Highlight suggested usage
    #[inline]
    pub const fn valid(mut self, style: Style) -> Self {
        self.valid = style;
        self
    }

    /// Highlight invalid usage
    #[inline]
    pub const fn invalid(mut self, style: Style) -> Self {
        self.invalid = style;
        self
    }

    /// Highlight all specified contexts, e.g. `[default: false]`
    ///
    /// To specialize the style of the value within the context, see [`Styles::context_value`]
    #[inline]
    pub const fn context(mut self, style: Style) -> Self {
        self.context = style;
        self
    }

    /// Highlight values within all of the context, e.g. the `false` in `[default: false]`
    ///
    /// If not explicitly set, falls back to `context`'s style.
    #[inline]
    pub const fn context_value(mut self, style: Style) -> Self {
        self.context_value = Some(style);
        self
    }
}

/// Reflection
impl Styles {
    /// General Heading style, e.g. [`help_heading`][crate::Arg::help_heading]
    #[inline(always)]
    pub const fn get_header(&self) -> &Style {
        &self.header
    }

    /// Error heading
    #[inline(always)]
    pub const fn get_error(&self) -> &Style {
        &self.error
    }

    /// Usage heading
    #[inline(always)]
    pub const fn get_usage(&self) -> &Style {
        &self.usage
    }

    /// Literal command-line syntax, e.g. `--help`
    #[inline(always)]
    pub const fn get_literal(&self) -> &Style {
        &self.literal
    }

    /// Descriptions within command-line syntax, e.g. [`value_name`][crate::Arg::value_name]
    #[inline(always)]
    pub const fn get_placeholder(&self) -> &Style {
        &self.placeholder
    }

    /// Highlight suggested usage
    #[inline(always)]
    pub const fn get_valid(&self) -> &Style {
        &self.valid
    }

    /// Highlight invalid usage
    #[inline(always)]
    pub const fn get_invalid(&self) -> &Style {
        &self.invalid
    }

    /// Highlight all specified contexts, e.g. `[default: false]`
    ///
    /// To specialize the style of the value within the context, see [`Styles::context_value`]
    #[inline(always)]
    pub const fn get_context(&self) -> &Style {
        &self.context
    }

    /// Highlight values within all of the context, e.g. the `false` in `[default: false]`
    ///
    /// If not explicitly set, falls back to `context`'s style.
    #[inline(always)]
    pub const fn get_context_value(&self) -> &Style {
        match &self.context_value {
            Some(s) => s,
            None => &self.context,
        }
    }
}

impl super::AppExt for Styles {}

impl Default for Styles {
    fn default() -> Self {
        Self::styled()
    }
}

impl Default for &'_ Styles {
    fn default() -> Self {
        const STYLES: Styles = Styles::styled();
        &STYLES
    }
}

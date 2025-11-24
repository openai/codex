use crate::{AnsiColors, Color, DynColor, DynColors};
use core::fmt;

#[cfg(doc)]
use crate::OwoColorize;

/// A runtime-configurable text effect for use with [`Style`]
#[allow(missing_docs)]
#[derive(Debug, Copy, Clone)]
pub enum Effect {
    Bold,
    Dimmed,
    Italic,
    Underline,
    Blink,
    BlinkFast,
    Reversed,
    Hidden,
    Strikethrough,
}

macro_rules! color_methods {
    ($(
        #[$fg_meta:meta] #[$bg_meta:meta] $color:ident $fg_method:ident $bg_method:ident
    ),* $(,)?) => {
        $(
            #[$fg_meta]
            #[must_use]
            pub const fn $fg_method(mut self) -> Self {
                self.fg = Some(DynColors::Ansi(AnsiColors::$color));
                self
            }

            #[$fg_meta]
            #[must_use]
            pub const fn $bg_method(mut self) -> Self {
                self.bg = Some(DynColors::Ansi(AnsiColors::$color));
                self
            }
         )*
    };
}

macro_rules! style_methods {
    ($(#[$meta:meta] ($name:ident, $set_name:ident)),* $(,)?) => {
        $(
            #[$meta]
            #[must_use]
            pub const fn $name(mut self) -> Self {
                self.style_flags = self.style_flags.$set_name(true);
                self
            }
        )*
    };
}

const _: () = (); // workaround for syntax highlighting bug

/// A wrapper type which applies a [`Style`] when displaying the inner type
pub struct Styled<T> {
    /// The target value to be styled
    pub(crate) target: T,
    /// The style to apply to target
    pub style: Style,
}

/// A pre-computed style that can be applied to a struct using [`OwoColorize::style`].
///
/// Its interface mimics that of [`OwoColorize`], but instead of chaining methods on your
/// object, you instead chain them on the `Style` object before applying it.
///
/// ```rust
/// use owo_colors::{OwoColorize, Style};
///
/// let my_style = Style::new()
///     .red()
///     .on_white()
///     .strikethrough();
///
/// println!("{}", "red text, white background, struck through".style(my_style));
/// ```
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Style {
    pub(crate) fg: Option<DynColors>,
    pub(crate) bg: Option<DynColors>,
    pub(crate) bold: bool,
    pub(crate) style_flags: StyleFlags,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) struct StyleFlags(pub(crate) u8);

impl StyleFlags {
    #[must_use]
    #[inline]
    const fn is_plain(&self) -> bool {
        self.0 == 0
    }
}

const DIMMED_SHIFT: u8 = 0;
const ITALIC_SHIFT: u8 = 1;
const UNDERLINE_SHIFT: u8 = 2;
const BLINK_SHIFT: u8 = 3;
const BLINK_FAST_SHIFT: u8 = 4;
const REVERSED_SHIFT: u8 = 5;
const HIDDEN_SHIFT: u8 = 6;
const STRIKETHROUGH_SHIFT: u8 = 7;

macro_rules! style_flags_methods {
    ($(($shift:ident, $name:ident, $set_name:ident)),* $(,)?) => {
        $(
            #[must_use]
            const fn $name(&self) -> bool {
                ((self.0 >> $shift) & 1) != 0
            }

            #[must_use]
            const fn $set_name(mut self, $name: bool) -> Self {
                self.0 = (self.0 & !(1 << $shift)) | (($name as u8) << $shift);
                self
            }
        )*
    };
}

impl StyleFlags {
    const fn new() -> Self {
        Self(0)
    }

    style_flags_methods! {
        (DIMMED_SHIFT, dimmed, set_dimmed),
        (ITALIC_SHIFT, italic, set_italic),
        (UNDERLINE_SHIFT, underline, set_underline),
        (BLINK_SHIFT, blink, set_blink),
        (BLINK_FAST_SHIFT, blink_fast, set_blink_fast),
        (REVERSED_SHIFT, reversed, set_reversed),
        (HIDDEN_SHIFT, hidden, set_hidden),
        (STRIKETHROUGH_SHIFT, strikethrough, set_strikethrough),
    }
}

impl Default for StyleFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl Style {
    /// Create a new style to be applied later
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            style_flags: StyleFlags::new(),
        }
    }

    /// Apply the style to a given struct to output.
    ///
    /// # Example
    ///
    /// Usage in const contexts:
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, Style, Styled};
    ///
    /// const STYLED_TEXT: Styled<&'static str> = Style::new().bold().italic().style("bold and italic text");
    ///
    /// println!("{}", STYLED_TEXT);
    /// # assert_eq!(format!("{}", STYLED_TEXT), "\u{1b}[1;3mbold and italic text\u{1b}[0m");
    /// ```
    pub const fn style<T>(&self, target: T) -> Styled<T> {
        Styled {
            target,
            style: *self,
        }
    }

    /// Set the foreground color generically
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, colors::*};
    ///
    /// println!("{}", "red foreground".fg::<Red>());
    /// ```
    #[must_use]
    pub const fn fg<C: Color>(mut self) -> Self {
        self.fg = Some(C::DYN_COLORS_EQUIVALENT);
        self
    }

    /// Set the background color generically.
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, colors::*};
    ///
    /// println!("{}", "black background".bg::<Black>());
    /// ```
    #[must_use]
    pub const fn bg<C: Color>(mut self) -> Self {
        self.bg = Some(C::DYN_COLORS_EQUIVALENT);
        self
    }

    /// Removes the foreground color from the style. Note that this does not apply
    /// the default color, but rather represents not changing the current terminal color.
    ///
    /// If you wish to actively change the terminal color back to the default, see
    /// [`Style::default_color`].
    #[must_use]
    pub const fn remove_fg(mut self) -> Self {
        self.fg = None;
        self
    }

    /// Removes the background color from the style. Note that this does not apply
    /// the default color, but rather represents not changing the current terminal color.
    ///
    /// If you wish to actively change the terminal color back to the default, see
    /// [`Style::on_default_color`].
    #[must_use]
    pub const fn remove_bg(mut self) -> Self {
        self.bg = None;
        self
    }

    color_methods! {
        /// Change the foreground color to black
        /// Change the background color to black
        Black    black    on_black,
        /// Change the foreground color to red
        /// Change the background color to red
        Red      red      on_red,
        /// Change the foreground color to green
        /// Change the background color to green
        Green    green    on_green,
        /// Change the foreground color to yellow
        /// Change the background color to yellow
        Yellow   yellow   on_yellow,
        /// Change the foreground color to blue
        /// Change the background color to blue
        Blue     blue     on_blue,
        /// Change the foreground color to magenta
        /// Change the background color to magenta
        Magenta  magenta  on_magenta,
        /// Change the foreground color to purple
        /// Change the background color to purple
        Magenta  purple   on_purple,
        /// Change the foreground color to cyan
        /// Change the background color to cyan
        Cyan     cyan     on_cyan,
        /// Change the foreground color to white
        /// Change the background color to white
        White    white    on_white,

        /// Change the foreground color to the terminal default
        /// Change the background color to the terminal default
        Default default_color on_default_color,

        /// Change the foreground color to bright black
        /// Change the background color to bright black
        BrightBlack    bright_black    on_bright_black,
        /// Change the foreground color to bright red
        /// Change the background color to bright red
        BrightRed      bright_red      on_bright_red,
        /// Change the foreground color to bright green
        /// Change the background color to bright green
        BrightGreen    bright_green    on_bright_green,
        /// Change the foreground color to bright yellow
        /// Change the background color to bright yellow
        BrightYellow   bright_yellow   on_bright_yellow,
        /// Change the foreground color to bright blue
        /// Change the background color to bright blue
        BrightBlue     bright_blue     on_bright_blue,
        /// Change the foreground color to bright magenta
        /// Change the background color to bright magenta
        BrightMagenta  bright_magenta  on_bright_magenta,
        /// Change the foreground color to bright purple
        /// Change the background color to bright purple
        BrightMagenta  bright_purple   on_bright_purple,
        /// Change the foreground color to bright cyan
        /// Change the background color to bright cyan
        BrightCyan     bright_cyan     on_bright_cyan,
        /// Change the foreground color to bright white
        /// Change the background color to bright white
        BrightWhite    bright_white    on_bright_white,
    }

    /// Make the text bold
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    style_methods! {
        /// Make the text dim
        (dimmed, set_dimmed),
        /// Make the text italicized
        (italic, set_italic),
        /// Make the text underlined
        (underline, set_underline),
        /// Make the text blink
        (blink, set_blink),
        /// Make the text blink (but fast!)
        (blink_fast, set_blink_fast),
        /// Swap the foreground and background colors
        (reversed, set_reversed),
        /// Hide the text
        (hidden, set_hidden),
        /// Cross out the text
        (strikethrough, set_strikethrough),
    }

    #[must_use]
    const fn set_effect(mut self, effect: Effect, to: bool) -> Self {
        use Effect::*;
        match effect {
            Bold => {
                self.bold = to;
            }
            Dimmed => {
                // This somewhat contorted construction is required because const fns can't take
                // mutable refs as of Rust 1.81.
                self.style_flags = self.style_flags.set_dimmed(to);
            }
            Italic => {
                self.style_flags = self.style_flags.set_italic(to);
            }
            Underline => {
                self.style_flags = self.style_flags.set_underline(to);
            }
            Blink => {
                self.style_flags = self.style_flags.set_blink(to);
            }
            BlinkFast => {
                self.style_flags = self.style_flags.set_blink_fast(to);
            }
            Reversed => {
                self.style_flags = self.style_flags.set_reversed(to);
            }
            Hidden => {
                self.style_flags = self.style_flags.set_hidden(to);
            }
            Strikethrough => {
                self.style_flags = self.style_flags.set_strikethrough(to);
            }
        }
        self
    }

    #[must_use]
    const fn set_effects(mut self, mut effects: &[Effect], to: bool) -> Self {
        // This is basically a for loop that also works in const contexts.
        while let [first, rest @ ..] = effects {
            self = self.set_effect(*first, to);
            effects = rest;
        }
        self
    }

    /// Apply a given effect from the style
    #[must_use]
    pub const fn effect(self, effect: Effect) -> Self {
        self.set_effect(effect, true)
    }

    /// Remove a given effect from the style
    #[must_use]
    pub const fn remove_effect(self, effect: Effect) -> Self {
        self.set_effect(effect, false)
    }

    /// Apply a given set of effects to the style
    #[must_use]
    pub const fn effects(self, effects: &[Effect]) -> Self {
        self.set_effects(effects, true)
    }

    /// Remove a given set of effects from the style
    #[must_use]
    pub const fn remove_effects(self, effects: &[Effect]) -> Self {
        self.set_effects(effects, false)
    }

    /// Disables all the given effects from the style
    #[must_use]
    pub const fn remove_all_effects(mut self) -> Self {
        self.bold = false;
        self.style_flags = StyleFlags::new();
        self
    }

    /// Set the foreground color at runtime. Only use if you do not know which color will be used at
    /// compile-time. If the color is constant, use either [`OwoColorize::fg`](crate::OwoColorize::fg) or
    /// a color-specific method, such as [`OwoColorize::green`](crate::OwoColorize::green),
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "green".color(AnsiColors::Green));
    /// ```
    #[must_use]
    pub fn color<Color: DynColor>(mut self, color: Color) -> Self {
        // Can't be const because `get_dyncolors_fg` is a trait method.
        self.fg = Some(color.get_dyncolors_fg());
        self
    }

    /// Set the background color at runtime. Only use if you do not know what color to use at
    /// compile-time. If the color is constant, use either [`OwoColorize::bg`](crate::OwoColorize::bg) or
    /// a color-specific method, such as [`OwoColorize::on_yellow`](crate::OwoColorize::on_yellow),
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
    /// ```
    #[must_use]
    pub fn on_color<Color: DynColor>(mut self, color: Color) -> Self {
        // Can't be const because `get_dyncolors_bg` is a trait method.
        self.bg = Some(color.get_dyncolors_bg());
        self
    }

    /// Set the foreground color to a specific RGB value.
    #[must_use]
    pub const fn fg_rgb<const R: u8, const G: u8, const B: u8>(mut self) -> Self {
        self.fg = Some(DynColors::Rgb(R, G, B));

        self
    }

    /// Set the background color to a specific RGB value.
    #[must_use]
    pub const fn bg_rgb<const R: u8, const G: u8, const B: u8>(mut self) -> Self {
        self.bg = Some(DynColors::Rgb(R, G, B));

        self
    }

    /// Sets the foreground color to an RGB value.
    #[must_use]
    pub const fn truecolor(mut self, r: u8, g: u8, b: u8) -> Self {
        self.fg = Some(DynColors::Rgb(r, g, b));
        self
    }

    /// Sets the background color to an RGB value.
    #[must_use]
    pub const fn on_truecolor(mut self, r: u8, g: u8, b: u8) -> Self {
        self.bg = Some(DynColors::Rgb(r, g, b));
        self
    }

    /// Returns true if the style does not apply any formatting.
    #[must_use]
    #[inline]
    pub const fn is_plain(&self) -> bool {
        let s = &self;
        !(s.fg.is_some() || s.bg.is_some() || s.bold) && s.style_flags.is_plain()
    }

    /// Returns a formatter for the style's ANSI prefix.
    ///
    /// This can be used to separate out the prefix and suffix of a style.
    ///
    /// # Example
    ///
    /// ```
    /// use owo_colors::Style;
    /// use std::fmt::Write;
    ///
    /// let style = Style::new().red().on_blue();
    /// let prefix = style.prefix_formatter();
    /// let suffix = style.suffix_formatter();
    ///
    /// // Write the prefix and suffix separately.
    /// let mut output = String::new();
    /// write!(output, "{}", prefix);
    /// output.push_str("Hello");
    /// write!(output, "{}", suffix);
    ///
    /// assert_eq!(output, "\x1b[31;44mHello\x1b[0m");
    /// ```
    pub const fn prefix_formatter(&self) -> StylePrefixFormatter {
        StylePrefixFormatter(*self)
    }

    /// Returns a formatter for the style's ANSI suffix.
    ///
    /// This can be used to separate out the prefix and suffix of a style.
    ///
    /// # Example
    ///
    /// See [`Style::prefix_formatter`].
    pub const fn suffix_formatter(&self) -> StyleSuffixFormatter {
        StyleSuffixFormatter(*self)
    }

    /// Applies the ANSI-prefix for this style to the given formatter
    #[inline]
    #[allow(unused_assignments)]
    pub fn fmt_prefix(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self;
        let format_less_important_effects = s.style_flags != StyleFlags::default();
        let format_effect = s.bold || format_less_important_effects;
        let format_any = !self.is_plain();

        let mut semicolon = false;

        if format_any {
            f.write_str("\x1b[")?;
        }

        if let Some(fg) = s.fg {
            <DynColors as DynColor>::fmt_raw_ansi_fg(&fg, f)?;
            semicolon = true;
        }

        if let Some(bg) = s.bg {
            if s.fg.is_some() {
                f.write_str(";")?;
            }
            <DynColors as DynColor>::fmt_raw_ansi_bg(&bg, f)?;
            semicolon = true;
        }

        if format_effect {
            if s.bold {
                if semicolon {
                    f.write_str(";")?;
                }

                f.write_str("1")?;

                semicolon = true;
            }

            macro_rules! text_effect_fmt {
                ($style:ident, $formatter:ident, $semicolon:ident, $(($attr:ident, $value:literal)),* $(,)?) => {
                    $(
                        if $style.style_flags.$attr() {
                            if $semicolon {
                                $formatter.write_str(";")?;
                            }
                            $formatter.write_str($value)?;

                            $semicolon = true;
                        }
                    )+
                }
            }

            if format_less_important_effects {
                text_effect_fmt! {
                    s, f, semicolon,
                    (dimmed,        "2"),
                    (italic,        "3"),
                    (underline,     "4"),
                    (blink,         "5"),
                    (blink_fast,    "6"),
                    (reversed,      "7"),
                    (hidden,        "8"),
                    (strikethrough, "9"),
                }
            }
        }

        if format_any {
            f.write_str("m")?;
        }
        Ok(())
    }

    /// Applies the ANSI-suffix for this style to the given formatter
    #[inline]
    pub fn fmt_suffix(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.is_plain() {
            f.write_str("\x1b[0m")?;
        }
        Ok(())
    }
}

/// Formatter for the prefix of a [`Style`].
///
/// This is used to get the ANSI escape codes for the style without
/// the suffix, which is useful for formatting the prefix separately.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use = "this formatter does nothing unless displayed"]
pub struct StylePrefixFormatter(Style);

impl fmt::Display for StylePrefixFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_prefix(f)
    }
}

/// Formatter for the suffix of a [`Style`].
///
/// This is used to get the ANSI escape codes for the style without
/// the prefix, which is useful for formatting the suffix separately.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use = "this formatter does nothing unless displayed"]
pub struct StyleSuffixFormatter(Style);

impl fmt::Display for StyleSuffixFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_suffix(f)
    }
}

impl Default for Style {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create [`Style`]s more ergonomically
pub const fn style() -> Style {
    Style::new()
}

impl<T> Styled<T> {
    /// Returns a reference to the inner value to be styled
    pub const fn inner(&self) -> &T {
        &self.target
    }

    /// Returns a mutable reference to the inner value to be styled.
    ///
    /// *This method is const on Rust 1.83+.*
    #[cfg(const_mut_refs)]
    pub const fn inner_mut(&mut self) -> &mut T {
        &mut self.target
    }

    /// Returns a mutable reference to the inner value to be styled.
    ///
    /// *This method is const on Rust 1.83+.*
    #[cfg(not(const_mut_refs))]
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.target
    }
}

macro_rules! impl_fmt {
    ($($trait:path),* $(,)?) => {
        $(
            impl<T: $trait> $trait for Styled<T> {
                #[allow(unused_assignments)]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.style.fmt_prefix(f)?;
                    <T as $trait>::fmt(&self.target, f)?;
                    self.style.fmt_suffix(f)
                }
            }
        )*
    };
}

impl_fmt! {
    fmt::Display,
    fmt::Debug,
    fmt::UpperHex,
    fmt::LowerHex,
    fmt::Binary,
    fmt::UpperExp,
    fmt::LowerExp,
    fmt::Octal,
    fmt::Pointer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnsiColors, OwoColorize};

    #[test]
    fn size_of() {
        let size = std::mem::size_of::<Style>();
        assert_eq!(size, 10, "size of Style should be 10 bytes");
    }

    #[test]
    fn test_it() {
        let style = Style::new()
            .bright_white()
            .on_blue()
            .bold()
            .dimmed()
            .italic()
            .underline()
            .blink()
            //.blink_fast()
            //.reversed()
            //.hidden()
            .strikethrough();
        let s = style.style("TEST");
        let s2 = format!("{}", &s);
        println!("{}", &s2);
        assert_eq!(&s2, "\u{1b}[97;44;1;2;3;4;5;9mTEST\u{1b}[0m");

        let prefix = format!("{}", style.prefix_formatter());
        assert_eq!(&prefix, "\u{1b}[97;44;1;2;3;4;5;9m");

        let suffix = format!("{}", style.suffix_formatter());
        assert_eq!(&suffix, "\u{1b}[0m");
    }

    #[test]
    fn test_effects() {
        use Effect::*;
        let style = Style::new().effects(&[Strikethrough, Underline]);

        let s = style.style("TEST");
        let s2 = format!("{}", &s);
        println!("{}", &s2);
        assert_eq!(&s2, "\u{1b}[4;9mTEST\u{1b}[0m");
    }

    #[test]
    fn test_color() {
        let style = Style::new()
            .color(AnsiColors::White)
            .on_color(AnsiColors::Black);

        let s = style.style("TEST");
        let s2 = format!("{}", &s);
        println!("{}", &s2);
        assert_eq!(&s2, "\u{1b}[37;40mTEST\u{1b}[0m");
    }

    #[test]
    fn test_truecolor() {
        let style = Style::new().truecolor(255, 255, 255).on_truecolor(0, 0, 0);

        let s = style.style("TEST");
        let s2 = format!("{}", &s);
        println!("{}", &s2);
        assert_eq!(&s2, "\u{1b}[38;2;255;255;255;48;2;0;0;0mTEST\u{1b}[0m");
    }

    #[test]
    fn test_string_reference() {
        let style = Style::new().truecolor(255, 255, 255).on_truecolor(0, 0, 0);

        let string = String::from("TEST");
        let s = style.style(&string);
        let s2 = format!("{}", &s);
        println!("{}", &s2);
        assert_eq!(&s2, "\u{1b}[38;2;255;255;255;48;2;0;0;0mTEST\u{1b}[0m");
    }

    #[test]
    fn test_owocolorize() {
        let style = Style::new().bright_white().on_blue();

        let s = "TEST".style(style);
        let s2 = format!("{}", &s);
        println!("{}", &s2);
        assert_eq!(&s2, "\u{1b}[97;44mTEST\u{1b}[0m");
    }

    #[test]
    fn test_is_plain() {
        let style = Style::new().bright_white().on_blue();

        assert!(!style.is_plain());
        assert!(Style::default().is_plain());

        let string = String::from("TEST");
        let s = Style::default().style(&string);
        let s2 = format!("{}", &s);

        assert_eq!(string, s2)
    }

    #[test]
    fn test_inner() {
        let style = Style::default();

        let mut s = "TEST".style(style);

        assert_eq!(&&"TEST", s.inner());

        *s.inner_mut() = &"changed";
        assert_eq!(&&"changed", s.inner());
        assert_eq!("changed", format!("{}", s));
    }
}

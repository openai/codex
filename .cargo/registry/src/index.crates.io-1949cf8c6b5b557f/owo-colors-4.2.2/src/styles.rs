//! Different display styles (strikethrough, bold, etc.)
use core::fmt;

#[allow(unused_imports)]
use crate::OwoColorize;
use crate::{Style, Styled};

macro_rules! impl_fmt_for_style {
    ($(($ty:ident, $trait:path, $ansi:literal)),* $(,)?) => {
        $(
            impl<'a, T: ?Sized + $trait> $trait for $ty<'a, T> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str($ansi)?;
                    <_ as $trait>::fmt(&self.0, f)?;
                    f.write_str("\x1b[0m")
                }
            }
        )*
    };
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// with the addition of boldening it. Recommended to be constructed using
/// [`OwoColorize`](OwoColorize::bold).
#[repr(transparent)]
pub struct BoldDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> BoldDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_bold() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_bold() {
    ///     "hello".bold().into_styled()
    /// } else {
    ///     "hello".dimmed().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[1mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().bold();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// with the addition of dimming it. Recommended to be constructed using
/// [`OwoColorize`](OwoColorize::dimmed).
#[repr(transparent)]
pub struct DimDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> DimDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_dimmed() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_dimmed() {
    ///     "hello".dimmed().into_styled()
    /// } else {
    ///     "hello".bold().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[2mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().dimmed();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// with the addition of italics. Recommended to be constructed using
/// [`OwoColorize`](OwoColorize::italic).
#[repr(transparent)]
pub struct ItalicDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> ItalicDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_italic() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_italic() {
    ///     "hello".italic().into_styled()
    /// } else {
    ///     "hello".underline().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[3mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().italic();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// while underlining it. Recommended to be constructed using
/// [`OwoColorize`](OwoColorize::underline).
#[repr(transparent)]
pub struct UnderlineDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> UnderlineDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_underline() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_underline() {
    ///     "hello".underline().into_styled()
    /// } else {
    ///     "hello".italic().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[4mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().underline();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// while blinking. Recommended to be constructed using
/// [`OwoColorize`](OwoColorize::blink).
#[repr(transparent)]
pub struct BlinkDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> BlinkDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_blink() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_blink() {
    ///     "hello".blink().into_styled()
    /// } else {
    ///     "hello".hidden().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[5mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().blink();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// with the addition of making it blink fast. Use [`OwoColorize`](OwoColorize::blink_fast)
#[repr(transparent)]
pub struct BlinkFastDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> BlinkFastDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_blink_fast() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_blink_fast() {
    ///     "hello".blink_fast().into_styled()
    /// } else {
    ///     "hello".reversed().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[6mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().blink_fast();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// with the addition of swapping fg and bg colors. Use [`OwoColorize`](OwoColorize::reversed)
#[repr(transparent)]
pub struct ReversedDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> ReversedDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_reversed() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_reversed() {
    ///     "hello".reversed().into_styled()
    /// } else {
    ///     "hello".blink_fast().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[7mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().reversed();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// with the addition of hiding the text. Use [`OwoColorize`](OwoColorize::hidden).
#[repr(transparent)]
pub struct HiddenDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> HiddenDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_hidden() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_hidden() {
    ///     "hello".hidden().into_styled()
    /// } else {
    ///     "hello".blink().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[8mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().hidden();
        Styled {
            style,
            target: self.0,
        }
    }
}

/// Transparent wrapper around a type which implements all the formatters the wrapped type does,
/// crossed out. Recommended to be constructed using
/// [`OwoColorize`](OwoColorize::strikethrough).
#[repr(transparent)]
pub struct StrikeThroughDisplay<'a, T: ?Sized>(pub &'a T);

impl<'a, T: ?Sized> StrikeThroughDisplay<'a, T> {
    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::OwoColorize;
    ///
    /// fn is_strike_through() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_strike_through() {
    ///     "hello".strikethrough().into_styled()
    /// } else {
    ///     "hello".hidden().into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(styled_str.to_string(), "\x1b[9mhello\x1b[0m");
    /// ```
    pub const fn into_styled(self) -> Styled<&'a T> {
        let style = Style::new().strikethrough();
        Styled {
            style,
            target: self.0,
        }
    }
}

impl_fmt_for_style! {
    // Bold
    (BoldDisplay, fmt::Display,  "\x1b[1m"),
    (BoldDisplay, fmt::Debug,    "\x1b[1m"),
    (BoldDisplay, fmt::UpperHex, "\x1b[1m"),
    (BoldDisplay, fmt::LowerHex, "\x1b[1m"),
    (BoldDisplay, fmt::Binary,   "\x1b[1m"),
    (BoldDisplay, fmt::UpperExp, "\x1b[1m"),
    (BoldDisplay, fmt::LowerExp, "\x1b[1m"),
    (BoldDisplay, fmt::Octal,    "\x1b[1m"),
    (BoldDisplay, fmt::Pointer,  "\x1b[1m"),

    // Dim
    (DimDisplay, fmt::Display,  "\x1b[2m"),
    (DimDisplay, fmt::Debug,    "\x1b[2m"),
    (DimDisplay, fmt::UpperHex, "\x1b[2m"),
    (DimDisplay, fmt::LowerHex, "\x1b[2m"),
    (DimDisplay, fmt::Binary,   "\x1b[2m"),
    (DimDisplay, fmt::UpperExp, "\x1b[2m"),
    (DimDisplay, fmt::LowerExp, "\x1b[2m"),
    (DimDisplay, fmt::Octal,    "\x1b[2m"),
    (DimDisplay, fmt::Pointer,  "\x1b[2m"),

    // Italic
    (ItalicDisplay, fmt::Display,  "\x1b[3m"),
    (ItalicDisplay, fmt::Debug,    "\x1b[3m"),
    (ItalicDisplay, fmt::UpperHex, "\x1b[3m"),
    (ItalicDisplay, fmt::LowerHex, "\x1b[3m"),
    (ItalicDisplay, fmt::Binary,   "\x1b[3m"),
    (ItalicDisplay, fmt::UpperExp, "\x1b[3m"),
    (ItalicDisplay, fmt::LowerExp, "\x1b[3m"),
    (ItalicDisplay, fmt::Octal,    "\x1b[3m"),
    (ItalicDisplay, fmt::Pointer,  "\x1b[3m"),

    // Underline
    (UnderlineDisplay, fmt::Display,  "\x1b[4m"),
    (UnderlineDisplay, fmt::Debug,    "\x1b[4m"),
    (UnderlineDisplay, fmt::UpperHex, "\x1b[4m"),
    (UnderlineDisplay, fmt::LowerHex, "\x1b[4m"),
    (UnderlineDisplay, fmt::Binary,   "\x1b[4m"),
    (UnderlineDisplay, fmt::UpperExp, "\x1b[4m"),
    (UnderlineDisplay, fmt::LowerExp, "\x1b[4m"),
    (UnderlineDisplay, fmt::Octal,    "\x1b[4m"),
    (UnderlineDisplay, fmt::Pointer,  "\x1b[4m"),

    // Blink
    (BlinkDisplay, fmt::Display,  "\x1b[5m"),
    (BlinkDisplay, fmt::Debug,    "\x1b[5m"),
    (BlinkDisplay, fmt::UpperHex, "\x1b[5m"),
    (BlinkDisplay, fmt::LowerHex, "\x1b[5m"),
    (BlinkDisplay, fmt::Binary,   "\x1b[5m"),
    (BlinkDisplay, fmt::UpperExp, "\x1b[5m"),
    (BlinkDisplay, fmt::LowerExp, "\x1b[5m"),
    (BlinkDisplay, fmt::Octal,    "\x1b[5m"),
    (BlinkDisplay, fmt::Pointer,  "\x1b[5m"),

    // Blink fast
    (BlinkFastDisplay, fmt::Display,  "\x1b[6m"),
    (BlinkFastDisplay, fmt::Debug,    "\x1b[6m"),
    (BlinkFastDisplay, fmt::UpperHex, "\x1b[6m"),
    (BlinkFastDisplay, fmt::LowerHex, "\x1b[6m"),
    (BlinkFastDisplay, fmt::Binary,   "\x1b[6m"),
    (BlinkFastDisplay, fmt::UpperExp, "\x1b[6m"),
    (BlinkFastDisplay, fmt::LowerExp, "\x1b[6m"),
    (BlinkFastDisplay, fmt::Octal,    "\x1b[6m"),
    (BlinkFastDisplay, fmt::Pointer,  "\x1b[6m"),

    // Reverse video
    (ReversedDisplay, fmt::Display,  "\x1b[7m"),
    (ReversedDisplay, fmt::Debug,    "\x1b[7m"),
    (ReversedDisplay, fmt::UpperHex, "\x1b[7m"),
    (ReversedDisplay, fmt::LowerHex, "\x1b[7m"),
    (ReversedDisplay, fmt::Binary,   "\x1b[7m"),
    (ReversedDisplay, fmt::UpperExp, "\x1b[7m"),
    (ReversedDisplay, fmt::LowerExp, "\x1b[7m"),
    (ReversedDisplay, fmt::Octal,    "\x1b[7m"),
    (ReversedDisplay, fmt::Pointer,  "\x1b[7m"),

    // Hide the text
    (HiddenDisplay, fmt::Display,  "\x1b[8m"),
    (HiddenDisplay, fmt::Debug,    "\x1b[8m"),
    (HiddenDisplay, fmt::UpperHex, "\x1b[8m"),
    (HiddenDisplay, fmt::LowerHex, "\x1b[8m"),
    (HiddenDisplay, fmt::Binary,   "\x1b[8m"),
    (HiddenDisplay, fmt::UpperExp, "\x1b[8m"),
    (HiddenDisplay, fmt::LowerExp, "\x1b[8m"),
    (HiddenDisplay, fmt::Octal,    "\x1b[8m"),
    (HiddenDisplay, fmt::Pointer,  "\x1b[8m"),

    // StrikeThrough
    (StrikeThroughDisplay, fmt::Display,  "\x1b[9m"),
    (StrikeThroughDisplay, fmt::Debug,    "\x1b[9m"),
    (StrikeThroughDisplay, fmt::UpperHex, "\x1b[9m"),
    (StrikeThroughDisplay, fmt::LowerHex, "\x1b[9m"),
    (StrikeThroughDisplay, fmt::Binary,   "\x1b[9m"),
    (StrikeThroughDisplay, fmt::UpperExp, "\x1b[9m"),
    (StrikeThroughDisplay, fmt::LowerExp, "\x1b[9m"),
    (StrikeThroughDisplay, fmt::Octal,    "\x1b[9m"),
    (StrikeThroughDisplay, fmt::Pointer,  "\x1b[9m"),
}

use crate::{BgColorDisplay, Color, FgColorDisplay};
use crate::{BgDynColorDisplay, DynColor, FgDynColorDisplay, Style, Styled, colors};

use core::fmt;
use core::marker::PhantomData;

#[cfg(doc)]
use crate::OwoColorize;

/// A wrapper type which applies both a foreground and background color
pub struct ComboColorDisplay<'a, Fg: Color, Bg: Color, T: ?Sized>(&'a T, PhantomData<(Fg, Bg)>);

/// Wrapper around a type which implements all the formatters the wrapped type does, with the
/// addition of changing the foreground and background color.
///
/// If compile-time coloring is an option, consider using [`ComboColorDisplay`] instead.
pub struct ComboDynColorDisplay<'a, Fg: DynColor, Bg: DynColor, T: ?Sized>(&'a T, Fg, Bg);

macro_rules! impl_fmt_for_combo {
    ($($trait:path),* $(,)?) => {
        $(
            impl<'a, Fg: Color, Bg: Color, T: ?Sized + $trait> $trait for ComboColorDisplay<'a, Fg, Bg, T> {
                #[inline(always)]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("\x1b[")?;
                    f.write_str(Fg::RAW_ANSI_FG)?;
                    f.write_str(";")?;
                    f.write_str(Bg::RAW_ANSI_BG)?;
                    f.write_str("m")?;
                    <T as $trait>::fmt(&self.0, f)?;
                    f.write_str("\x1b[0m")
                }
            }
        )*

        $(
            impl<'a, Fg: DynColor, Bg: DynColor, T: ?Sized + $trait> $trait for ComboDynColorDisplay<'a, Fg, Bg, T> {
                #[inline(always)]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("\x1b[")?;
                    self.1.fmt_raw_ansi_fg(f)?;
                    f.write_str(";")?;
                    self.2.fmt_raw_ansi_bg(f)?;
                    f.write_str("m")?;
                    <T as $trait>::fmt(&self.0, f)?;
                    f.write_str("\x1b[0m")
                }
            }
        )*
    };
}

impl_fmt_for_combo! {
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

/// implement specialized color methods for FgColorDisplay BgColorDisplay, ComboColorDisplay
macro_rules! color_methods {
    ($(
        #[$fg_meta:meta] #[$bg_meta:meta] $color:ident $fg_method:ident $bg_method:ident
    ),* $(,)?) => {
        const _: () = (); // workaround for syntax highlighting bug

        impl<'a, Fg, T: ?Sized> FgColorDisplay<'a, Fg, T>
        where
            Fg: Color,
        {
            /// Create a new [`FgColorDisplay`], from a reference to a type which implements
            /// [`Color`].
            ///
            /// This is a const function: in non-const contexts, [`OwoColorize::fg`] or one of the
            /// other methods on it may be more convenient.
            ///
            /// # Example
            ///
            /// Usage in const contexts:
            ///
            /// ```rust
            /// use owo_colors::{colors::Green, FgColorDisplay};
            ///
            /// const GREEN_TEXT: FgColorDisplay<Green, str> = FgColorDisplay::new("green");
            ///
            /// println!("{}", GREEN_TEXT);
            /// # assert_eq!(format!("{}", GREEN_TEXT), "\x1b[32mgreen\x1b[39m");
            /// ```
            pub const fn new(thing: &'a T) -> Self {
                Self(thing, PhantomData)
            }

            /// Convert self to a generic [`Styled`].
            ///
            /// This method erases color-related type parameters, and can be
            /// used to unify types across branches.
            ///
            /// # Example
            ///
            /// Typical use:
            ///
            /// ```rust
            /// use owo_colors::OwoColorize;
            ///
            /// fn is_blue() -> bool {
            ///     // ...
            ///     # true
            /// }
            ///
            /// let styled_str = if is_blue() {
            ///     "hello".blue().into_styled()
            /// } else {
            ///     "hello".green().into_styled()
            /// };
            ///
            /// println!("{}", styled_str);
            /// # assert_eq!(format!("{}", styled_str), "\x1b[34mhello\x1b[0m");
            /// ```
            ///
            /// Usage in const contexts:
            ///
            /// ```rust
            /// use owo_colors::{colors::{Blue, Green}, FgColorDisplay, Styled};
            ///
            /// const fn is_blue() -> bool {
            ///     // ...
            ///     # true
            /// }
            ///
            /// const STYLED_STR: Styled<&str> = if is_blue() {
            ///     FgColorDisplay::<Blue, _>::new("Hello").into_styled()
            /// } else {
            ///     FgColorDisplay::<Green, _>::new("Hello").into_styled()
            /// };
            ///
            /// println!("{}", STYLED_STR);
            /// # assert_eq!(format!("{}", STYLED_STR), "\x1b[34mHello\x1b[0m");
            /// ```
            pub const fn into_styled(self) -> Styled<&'a T> {
                let style = Style::new().fg::<Fg>();
                Styled { style, target: self.0 }
            }

            /// Set the foreground color at runtime. Only use if you do not know which color will be used at
            /// compile-time. If the color is constant, use either [`OwoColorize::fg`] or
            /// a color-specific method, such as [`OwoColorize::green`],
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, AnsiColors};
            ///
            /// println!("{}", "green".color(AnsiColors::Green));
            /// ```
            pub const fn color<NewFg: DynColor>(
                self,
                fg: NewFg,
            ) -> FgDynColorDisplay<'a, NewFg, T> {
                FgDynColorDisplay(self.0, fg)
            }

            /// Set the background color at runtime. Only use if you do not know what color to use at
            /// compile-time. If the color is constant, use either [`OwoColorize::bg`] or
            /// a color-specific method, such as [`OwoColorize::on_yellow`],
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, AnsiColors};
            ///
            /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
            /// ```
            pub const fn on_color<NewBg: DynColor>(
                self,
                bg: NewBg,
            ) -> ComboDynColorDisplay<'a, Fg::DynEquivalent, NewBg, T> {
                ComboDynColorDisplay(self.0, Fg::DYN_EQUIVALENT, bg)
            }

            /// Set the foreground color generically
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, colors::*};
            ///
            /// println!("{}", "red foreground".fg::<Red>());
            /// ```
            pub const fn fg<C: Color>(self) -> FgColorDisplay<'a, C, T> {
                FgColorDisplay(self.0, PhantomData)
            }

            /// Set the background color generically.
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, colors::*};
            ///
            /// println!("{}", "black background".bg::<Black>());
            /// ```
            pub const fn bg<C: Color>(self) -> ComboColorDisplay<'a, Fg, C, T> {
                ComboColorDisplay(self.0, PhantomData)
            }

            $(
                #[$fg_meta]
                #[inline(always)]
                pub const fn $fg_method(self) -> FgColorDisplay<'a, colors::$color, T> {
                    FgColorDisplay(self.0, PhantomData)
                }

                #[$bg_meta]
                #[inline(always)]
                pub const fn $bg_method(self) -> ComboColorDisplay<'a, Fg, colors::$color, T> {
                    ComboColorDisplay(self.0, PhantomData)
                }
             )*
        }

        const _: () = (); // workaround for syntax highlighting bug

        impl<'a, Bg, T: ?Sized> BgColorDisplay<'a, Bg, T>
        where
            Bg: Color,
        {
            /// Create a new [`BgColorDisplay`], from a reference to a type which implements
            /// [`Color`].
            ///
            /// This is a const function: in non-const contexts, [`OwoColorize::bg`] may be more
            /// convenient.
            ///
            /// # Example
            ///
            /// Usage in const contexts:
            ///
            /// ```rust
            /// use owo_colors::{colors::Red, BgColorDisplay};
            ///
            /// const RED_BG_TEXT: BgColorDisplay<Red, str> = BgColorDisplay::new("red background");
            ///
            /// println!("{}", RED_BG_TEXT);
            /// # assert_eq!(format!("{}", RED_BG_TEXT), "\x1b[41mred background\x1b[49m");
            /// ```
            pub const fn new(thing: &'a T) -> Self {
                Self(thing, PhantomData)
            }

            /// Convert self to a generic [`Styled`].
            ///
            /// This method erases color-related type parameters, and can be
            /// used to unify types across branches.
            ///
            /// # Example
            ///
            /// Typical use:
            ///
            /// ```rust
            /// use owo_colors::OwoColorize;
            ///
            /// fn is_red() -> bool {
            ///     // ...
            ///     # true
            /// }
            ///
            /// let styled_str = if is_red() {
            ///     "hello".on_red().into_styled()
            /// } else {
            ///     "hello".on_yellow().into_styled()
            /// };
            ///
            /// println!("{}", styled_str);
            /// # assert_eq!(format!("{}", styled_str), "\x1b[41mhello\x1b[0m");
            /// ```
            ///
            /// Usage in const contexts:
            ///
            /// ```rust
            /// use owo_colors::{colors::{Red, Yellow}, BgColorDisplay, Styled};
            ///
            /// const fn is_red() -> bool {
            ///     // ...
            ///     # true
            /// }
            ///
            /// const STYLED_STR: Styled<&str> = if is_red() {
            ///     BgColorDisplay::<Red, _>::new("Hello").into_styled()
            /// } else {
            ///     BgColorDisplay::<Yellow, _>::new("Hello").into_styled()
            /// };
            ///
            /// println!("{}", STYLED_STR);
            /// # assert_eq!(format!("{}", STYLED_STR), "\x1b[41mHello\x1b[0m");
            /// ```
            pub const fn into_styled(self) -> Styled<&'a T> {
                let style = Style::new().bg::<Bg>();
                Styled { style, target: self.0 }
            }

            /// Set the foreground color at runtime. Only use if you do not know which color will be used at
            /// compile-time. If the color is constant, use either [`OwoColorize::fg`] or
            /// a color-specific method, such as [`OwoColorize::green`],
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, AnsiColors};
            ///
            /// println!("{}", "green".color(AnsiColors::Green));
            /// ```
            pub const fn color<NewFg: DynColor>(
                self,
                fg: NewFg,
            ) -> ComboDynColorDisplay<'a, NewFg, Bg::DynEquivalent, T> {
                ComboDynColorDisplay(self.0, fg, Bg::DYN_EQUIVALENT)
            }

            /// Set the background color at runtime. Only use if you do not know what color to use at
            /// compile-time. If the color is constant, use either [`OwoColorize::bg`] or
            /// a color-specific method, such as [`OwoColorize::on_yellow`],
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, AnsiColors};
            ///
            /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
            /// ```
            pub const fn on_color<NewBg: DynColor>(
                self,
                bg: NewBg,
            ) -> BgDynColorDisplay<'a, NewBg, T> {
                BgDynColorDisplay(self.0, bg)
            }

            /// Set the foreground color generically
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, colors::*};
            ///
            /// println!("{}", "red foreground".fg::<Red>());
            /// ```
            pub const fn fg<C: Color>(self) -> ComboColorDisplay<'a, C, Bg, T> {
                ComboColorDisplay(self.0, PhantomData)
            }

            /// Set the background color generically.
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, colors::*};
            ///
            /// println!("{}", "black background".bg::<Black>());
            /// ```
            pub const fn bg<C: Color>(self) -> BgColorDisplay<'a, C, T> {
                BgColorDisplay(self.0, PhantomData)
            }

            $(
                #[$bg_meta]
                #[inline(always)]
                pub const fn $bg_method(self) -> BgColorDisplay<'a, colors::$color, T> {
                    BgColorDisplay(self.0, PhantomData)
                }

                #[$fg_meta]
                #[inline(always)]
                pub const fn $fg_method(self) -> ComboColorDisplay<'a, colors::$color, Bg, T> {
                    ComboColorDisplay(self.0, PhantomData)
                }
             )*
        }

        const _: () = (); // workaround for syntax highlighting bug

        impl<'a, Fg, Bg, T: ?Sized> ComboColorDisplay<'a, Fg, Bg, T>
        where
            Fg: Color,
            Bg: Color,
        {
            /// Create a new [`ComboColorDisplay`], from a pair of foreground and background types
            /// which implement [`Color`].
            ///
            /// This is a const function: in non-const contexts, calling the [`OwoColorize`]
            /// functions may be more convenient.
            ///
            /// # Example
            ///
            /// Usage in const contexts:
            ///
            /// ```rust
            /// use owo_colors::{colors::{Blue, White}, ComboColorDisplay};
            ///
            /// const COMBO_TEXT: ComboColorDisplay<Blue, White, str> =
            ///    ComboColorDisplay::new("blue text on white background");
            ///
            /// println!("{}", COMBO_TEXT);
            /// # assert_eq!(format!("{}", COMBO_TEXT), "\x1b[34;47mblue text on white background\x1b[0m");
            /// ```
            pub const fn new(thing: &'a T) -> Self {
                Self(thing, PhantomData)
            }

            /// Convert self to a generic [`Styled`].
            ///
            /// This method erases color-related type parameters, and can be
            /// used to unify types across branches.
            ///
            /// # Example
            ///
            /// Typical use:
            ///
            /// ```rust
            /// use owo_colors::OwoColorize;
            ///
            /// fn is_black_on_white() -> bool {
            ///     // ...
            ///     # true
            /// }
            ///
            /// let styled_str = if is_black_on_white() {
            ///     "hello".black().on_white().into_styled()
            /// } else {
            ///     "hello".white().on_black().into_styled()
            /// };
            ///
            /// println!("{}", styled_str);
            /// # assert_eq!(format!("{}", styled_str), "\x1b[30;47mhello\x1b[0m");
            /// ```
            ///
            /// Usage in const contexts:
            ///
            /// ```rust
            /// use owo_colors::{colors::{Black, White}, ComboColorDisplay, Styled};
            ///
            /// const fn is_black_on_white() -> bool {
            ///     // ...
            ///     # true
            /// }
            ///
            /// const STYLED_STR: Styled<&str> = if is_black_on_white() {
            ///     ComboColorDisplay::<Black, White, _>::new("Hello").into_styled()
            /// } else {
            ///     ComboColorDisplay::<White, Black, _>::new("Hello").into_styled()
            /// };
            ///
            /// println!("{}", STYLED_STR);
            /// # assert_eq!(format!("{}", STYLED_STR), "\x1b[30;47mHello\x1b[0m");
            /// ```
            pub const fn into_styled(self) -> Styled<&'a T> {
                let style = Style::new().fg::<Fg>().bg::<Bg>();
                Styled { style, target: self.0 }
            }

            /// Set the background color at runtime. Only use if you do not know what color to use at
            /// compile-time. If the color is constant, use either [`OwoColorize::bg`] or
            /// a color-specific method, such as [`OwoColorize::on_yellow`],
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, AnsiColors};
            ///
            /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
            /// ```
            pub const fn on_color<NewBg: DynColor>(
                self,
                bg: NewBg,
            ) -> ComboDynColorDisplay<'a, Fg::DynEquivalent, NewBg, T> {
                ComboDynColorDisplay(self.0, Fg::DYN_EQUIVALENT, bg)
            }

            /// Set the foreground color at runtime. Only use if you do not know which color will be used at
            /// compile-time. If the color is constant, use either [`OwoColorize::fg`] or
            /// a color-specific method, such as [`OwoColorize::green`],
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, AnsiColors};
            ///
            /// println!("{}", "green".color(AnsiColors::Green));
            /// ```
            pub const fn color<NewFg: DynColor>(
                self,
                fg: NewFg,
            ) -> ComboDynColorDisplay<'a, NewFg, Bg::DynEquivalent, T> {
                ComboDynColorDisplay(self.0, fg, Bg::DYN_EQUIVALENT)
            }

            /// Set the foreground color generically
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, colors::*};
            ///
            /// println!("{}", "red foreground".fg::<Red>());
            /// ```
            pub const fn fg<C: Color>(self) -> ComboColorDisplay<'a, C, Bg, T> {
                ComboColorDisplay(self.0, PhantomData)
            }

            /// Set the background color generically.
            ///
            /// ```rust
            /// use owo_colors::{OwoColorize, colors::*};
            ///
            /// println!("{}", "black background".bg::<Black>());
            /// ```
            pub const fn bg<C: Color>(self) -> ComboColorDisplay<'a, Fg, C, T> {
                ComboColorDisplay(self.0, PhantomData)
            }

            $(
                #[$bg_meta]
                #[inline(always)]
                pub const fn $bg_method(self) -> ComboColorDisplay<'a, Fg, colors::$color, T> {
                    ComboColorDisplay(self.0, PhantomData)
                }

                #[$fg_meta]
                #[inline(always)]
                pub const fn $fg_method(self) -> ComboColorDisplay<'a, colors::$color, Bg, T> {
                    ComboColorDisplay(self.0, PhantomData)
                }
            )*
        }
    };
}

const _: () = (); // workaround for syntax highlighting bug

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

impl<'a, Fg: DynColor + Copy, T: ?Sized> FgDynColorDisplay<'a, Fg, T> {
    /// Create a new [`FgDynColorDisplay`], from a reference to a type which implements
    /// [`DynColor`].
    ///
    /// This is a const function: in non-const contexts, [`OwoColorize::color`] may be more
    /// convenient.
    ///
    /// # Example
    ///
    /// Usage in const contexts:
    ///
    /// ```rust
    /// use owo_colors::{AnsiColors, FgDynColorDisplay};
    ///
    /// const DYN_RED_TEXT: FgDynColorDisplay<AnsiColors, str> =
    ///    FgDynColorDisplay::new("red text (dynamic)", AnsiColors::Red);
    ///
    /// println!("{}", DYN_RED_TEXT);
    /// # assert_eq!(format!("{}", DYN_RED_TEXT), "\x1b[31mred text (dynamic)\x1b[39m");
    /// ```
    pub const fn new(thing: &'a T, color: Fg) -> Self {
        Self(thing, color)
    }

    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::{AnsiColors, CssColors, OwoColorize};
    ///
    /// fn is_blue() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_blue() {
    ///     "hello".color(AnsiColors::Blue).into_styled()
    /// } else {
    ///     "hello".color(CssColors::DarkSeaGreen).into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(format!("{}", styled_str), "\x1b[34mhello\x1b[0m");
    /// ```
    pub fn into_styled(self) -> Styled<&'a T> {
        let Self(target, fg) = self;
        let style = Style::new().color(fg);
        Styled { style, target }
    }

    /// Set the background color at runtime. Only use if you do not know what color to use at
    /// compile-time. If the color is constant, use either [`OwoColorize::bg`] or
    /// a color-specific method, such as [`OwoColorize::on_yellow`],
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
    /// ```
    pub const fn on_color<Bg: DynColor>(self, bg: Bg) -> ComboDynColorDisplay<'a, Fg, Bg, T> {
        let Self(inner, fg) = self;
        ComboDynColorDisplay(inner, fg, bg)
    }

    /// Set the foreground color at runtime. Only use if you do not know which color will be used at
    /// compile-time. If the color is constant, use either [`OwoColorize::fg`] or
    /// a color-specific method, such as [`OwoColorize::green`],
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "green".color(AnsiColors::Green));
    /// ```
    pub const fn color<NewFg: DynColor>(self, fg: NewFg) -> FgDynColorDisplay<'a, NewFg, T> {
        let Self(inner, _) = self;
        FgDynColorDisplay(inner, fg)
    }
}

impl<'a, Bg: DynColor + Copy, T: ?Sized> BgDynColorDisplay<'a, Bg, T> {
    /// Create a new [`BgDynColorDisplay`], from a reference to a type which implements
    /// [`DynColor`].
    ///
    /// This is a const function: in non-const contexts, [`OwoColorize::on_color`] may be more
    /// convenient.
    ///
    /// # Example
    ///
    /// Usage in const contexts:
    ///
    /// ```rust
    /// use owo_colors::{AnsiColors, BgDynColorDisplay};
    ///
    /// const DYN_GREEN_BG_TEXT: BgDynColorDisplay<AnsiColors, str> =
    ///    BgDynColorDisplay::new("green background (dynamic)", AnsiColors::Green);
    ///
    /// println!("{}", DYN_GREEN_BG_TEXT);
    /// # assert_eq!(format!("{}", DYN_GREEN_BG_TEXT), "\x1b[42mgreen background (dynamic)\x1b[49m");
    /// ```
    pub const fn new(thing: &'a T, color: Bg) -> Self {
        Self(thing, color)
    }

    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// ```rust
    /// use owo_colors::{AnsiColors, CssColors, OwoColorize};
    ///
    /// fn is_red() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_red() {
    ///     "hello".on_color(AnsiColors::Red).into_styled()
    /// } else {
    ///     "hello".on_color(CssColors::LightGoldenRodYellow).into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(format!("{}", styled_str), "\x1b[41mhello\x1b[0m");
    /// ```
    pub fn into_styled(self) -> Styled<&'a T> {
        let Self(target, bg) = self;
        let style = Style::new().on_color(bg);
        Styled { style, target }
    }

    /// Set the background color at runtime. Only use if you do not know what color to use at
    /// compile-time. If the color is constant, use either [`OwoColorize::bg`] or
    /// a color-specific method, such as [`OwoColorize::on_yellow`],
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
    /// ```
    pub const fn on_color<NewBg: DynColor>(self, bg: NewBg) -> BgDynColorDisplay<'a, NewBg, T> {
        let Self(inner, _) = self;
        BgDynColorDisplay(inner, bg)
    }

    /// Set the foreground color at runtime. Only use if you do not know which color will be used at
    /// compile-time. If the color is constant, use either [`OwoColorize::fg`] or
    /// a color-specific method, such as [`OwoColorize::green`],
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "green".color(AnsiColors::Green));
    /// ```
    pub const fn color<Fg: DynColor>(self, fg: Fg) -> ComboDynColorDisplay<'a, Fg, Bg, T> {
        let Self(inner, bg) = self;
        ComboDynColorDisplay(inner, fg, bg)
    }
}

impl<'a, Fg: DynColor + Copy, Bg: DynColor + Copy, T: ?Sized> ComboDynColorDisplay<'a, Fg, Bg, T> {
    /// Create a new [`ComboDynColorDisplay`], from a pair of types which implement
    /// [`DynColor`].
    ///
    /// This is a const function: in non-const contexts, other functions may be more convenient.
    ///
    /// # Example
    ///
    /// Usage in const contexts:
    ///
    /// ```rust
    /// use owo_colors::{ComboDynColorDisplay, XtermColors};
    ///
    /// const COMBO_DYN_TEXT: ComboDynColorDisplay<XtermColors, XtermColors, str> =
    ///     ComboDynColorDisplay::new(
    ///         "blue text on lilac background (dynamic)",
    ///         XtermColors::BlueRibbon,
    ///         XtermColors::WistfulLilac,
    ///     );
    ///
    /// println!("{}", COMBO_DYN_TEXT);
    /// # assert_eq!(format!("{}", COMBO_DYN_TEXT), "\x1b[38;5;27;48;5;146mblue text on lilac background (dynamic)\x1b[0m");
    /// ```
    pub const fn new(thing: &'a T, fg: Fg, bg: Bg) -> Self {
        Self(thing, fg, bg)
    }

    /// Convert self to a generic [`Styled`].
    ///
    /// This method erases color-related type parameters, and can be
    /// used to unify types across branches.
    ///
    /// # Example
    ///
    /// Typical use:
    ///
    /// ```rust
    /// use owo_colors::{AnsiColors, CssColors, OwoColorize};
    ///
    /// fn is_black_on_white() -> bool {
    ///     // ...
    ///     # true
    /// }
    ///
    /// let styled_str = if is_black_on_white() {
    ///     "hello".color(AnsiColors::Black).on_color(AnsiColors::White).into_styled()
    /// } else {
    ///     "hello".color(CssColors::White).on_color(CssColors::Black).into_styled()
    /// };
    ///
    /// println!("{}", styled_str);
    /// # assert_eq!(format!("{}", styled_str), "\x1b[30;47mhello\x1b[0m");
    /// ```
    pub fn into_styled(self) -> Styled<&'a T> {
        let Self(target, fg, bg) = self;
        let style = Style::new().color(fg).on_color(bg);
        Styled { style, target }
    }

    /// Set the background color at runtime. Only use if you do not know what color to use at
    /// compile-time. If the color is constant, use either [`OwoColorize::bg`] or
    /// a color-specific method, such as [`OwoColorize::on_yellow`],
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "yellow background".on_color(AnsiColors::BrightYellow));
    /// ```
    pub const fn on_color<NewBg: DynColor>(
        self,
        bg: NewBg,
    ) -> ComboDynColorDisplay<'a, Fg, NewBg, T> {
        let Self(inner, fg, _) = self;
        ComboDynColorDisplay(inner, fg, bg)
    }

    /// Set the foreground color at runtime. Only use if you do not know which color will be used at
    /// compile-time. If the color is constant, use either [`OwoColorize::fg`] or
    /// a color-specific method, such as [`OwoColorize::green`],
    ///
    /// ```rust
    /// use owo_colors::{OwoColorize, AnsiColors};
    ///
    /// println!("{}", "green".color(AnsiColors::Green));
    /// ```
    pub const fn color<NewFg: DynColor>(self, fg: NewFg) -> ComboDynColorDisplay<'a, NewFg, Bg, T> {
        // TODO: Make this const after https://github.com/rust-lang/rust/issues/73255 is stabilized.
        let Self(inner, _, bg) = self;
        ComboDynColorDisplay(inner, fg, bg)
    }
}

#[cfg(test)]
mod tests {
    use crate::{AnsiColors, OwoColorize, colors::*};

    #[test]
    fn fg_bg_combo() {
        let test = "test".red().on_blue();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn bg_fg_combo() {
        let test = "test".on_blue().red();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn fg_bg_dyn_combo() {
        let test = "test".color(AnsiColors::Red).on_color(AnsiColors::Blue);
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn bg_fg_dyn_combo() {
        let test = "test".on_color(AnsiColors::Blue).color(AnsiColors::Red);
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn fg_override() {
        let test = "test".green().yellow().red().on_blue();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn bg_override() {
        let test = "test".on_green().on_yellow().on_blue().red();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn multiple_override() {
        let test = "test"
            .on_green()
            .on_yellow()
            .on_red()
            .green()
            .on_blue()
            .red();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");

        let test = "test"
            .color(AnsiColors::Blue)
            .color(AnsiColors::White)
            .on_color(AnsiColors::Black)
            .color(AnsiColors::Red)
            .on_color(AnsiColors::Blue);
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");

        let test = "test"
            .on_yellow()
            .on_red()
            .on_color(AnsiColors::Black)
            .color(AnsiColors::Red)
            .on_color(AnsiColors::Blue);
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");

        let test = "test"
            .yellow()
            .red()
            .color(AnsiColors::Red)
            .on_color(AnsiColors::Black)
            .on_color(AnsiColors::Blue);
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");

        let test = "test"
            .yellow()
            .red()
            .on_color(AnsiColors::Black)
            .color(AnsiColors::Red)
            .on_color(AnsiColors::Blue);
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn generic_multiple_override() {
        use crate::colors::*;

        let test = "test"
            .bg::<Green>()
            .bg::<Yellow>()
            .bg::<Red>()
            .fg::<Green>()
            .bg::<Blue>()
            .fg::<Red>();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn fg_bg_combo_generic() {
        let test = "test".fg::<Red>().bg::<Blue>();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }

    #[test]
    fn bg_fg_combo_generic() {
        let test = "test".bg::<Blue>().fg::<Red>();
        assert_eq!(test.to_string(), "\x1b[31;44mtest\x1b[0m");
    }
}

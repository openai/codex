use crate::{Style, Attribute, Quirk, Condition};

/// Enum representing a terminal color.
///
/// **Note:** The color examples below are purely demonstrative. The actual
/// color rendered depends entirely on the terminal and its configuration, the
/// latter of which is entirely arbitrary.
#[derive(Debug, PartialEq, Eq, Copy, Clone, PartialOrd, Ord, Hash)]
pub enum Color {
    /// Terminal primary color #9. (foreground code `39`, background code `49`).
    ///
    /// This is the terminal's defined "primary" color, that is, the configured
    /// default foreground and background colors. As such, this color as a
    /// foreground looks "good" against the terminal's default background color,
    /// and this color is a "good" background color for the terminal's default
    /// foreground color.
    Primary,

    /// A color from 0 to 255, for use in 256-color terminals.
    Fixed(u8),

    /// A 24-bit
    /// <span style="background: red; color: white;">R</span>
    /// <span style="background: green; color: white;">G</span>
    /// <span style="background: blue; color: white;">B</span>
    /// "true color", as specified by ISO-8613-3.
    Rgb(u8, u8, u8),

    /// <span style="background: black; color: white;">Black #0</span>
    /// (foreground code `30`, background code `40`).
    Black,

    /// <span style="background: red; color: white;">Red #1</span>
    /// (foreground code `31`, background code `41`).
    Red,

    /// <span style="background: green; color: white;">Green: #2</span>
    /// (foreground code `32`, background code `42`).
    Green,

    /// <span style="background: gold; color: black;">Yellow: #3</span>
    /// (foreground code `33`, background code `43`).
    Yellow,

    /// <span style="background: blue; color: white;">Blue: #4</span>
    /// (foreground code `34`, background code `44`).
    Blue,

    /// <span style="background: darkmagenta; color: white;">Magenta: #5</span>
    /// (foreground code `35`, background code `45`).
    Magenta,

    /// <span style="background: deepskyblue; color: black;">Cyan: #6</span>
    /// (foreground code `36`, background code `46`).
    Cyan,

    /// <span style="background: #eeeeee; color: black;">White: #7</span>
    /// (foreground code `37`, background code `47`).
    White,

    /// <span style="background: gray; color: white;">Bright Black #0</span>
    /// (foreground code `90`, background code `100`).
    BrightBlack,

    /// <span style="background: hotpink; color: white;">Bright Red #1</span>
    /// (foreground code `91`, background code `101`).
    BrightRed,

    /// <span style="background: greenyellow; color: black;">Bright Green: #2</span>
    /// (foreground code `92`, background code `102`).
    BrightGreen,

    /// <span style="background: yellow; color: black;">Bright Yellow: #3</span>
    /// (foreground code `93`, background code `103`).
    BrightYellow,

    /// <span style="background: dodgerblue; color: white;">Bright Blue: #4</span>
    /// (foreground code `94`, background code `104`).
    BrightBlue,

    /// <span style="background: magenta; color: white;">Bright Magenta: #5</span>
    /// (foreground code `95`, background code `105`).
    BrightMagenta,

    /// <span style='background: cyan; color: black;'>Bright Cyan: #6</span>
    /// (foreground code `96`, background code `106`).
    BrightCyan,

    /// <span style="background: white; color: black;">Bright White: #7</span>
    /// (foreground code `97`, background code `107`).
    BrightWhite,
}

pub(crate) enum Variant { Fg, Bg, }

impl Color {
    fn fg_base(&self) -> u8 {
        match self {
            Color::Black => 30,
            Color::Red => 31,
            Color::Green => 32,
            Color::Yellow => 33,
            Color::Blue => 34,
            Color::Magenta => 35,
            Color::Cyan => 36,
            Color::White => 37,
            Color::Fixed(_) | Color::Rgb(..) => 38,
            Color::Primary => 39,
            Color::BrightBlack => 30 + 60,
            Color::BrightRed => 31 + 60,
            Color::BrightGreen => 32 + 60,
            Color::BrightYellow => 33 + 60,
            Color::BrightBlue => 34 + 60,
            Color::BrightMagenta => 35 + 60,
            Color::BrightCyan => 36 + 60,
            Color::BrightWhite => 37 + 60,
        }
    }

    pub(crate) const fn to_bright(self) -> Self {
        match self {
            Color::Black => Color::BrightBlack,
            Color::Red => Color::BrightRed,
            Color::Green => Color::BrightGreen,
            Color::Yellow => Color::BrightYellow,
            Color::Blue => Color::BrightBlue,
            Color::Magenta => Color::BrightMagenta,
            Color::Cyan => Color::BrightCyan,
            Color::White => Color::BrightWhite,
            Color::Fixed(_)
                | Color::Primary
                | Color::Rgb(_, _, _)
                | Color::BrightBlack
                | Color::BrightRed
                | Color::BrightGreen
                | Color::BrightYellow
                | Color::BrightBlue
                | Color::BrightMagenta
                | Color::BrightCyan
                | Color::BrightWhite => self
        }
    }

    pub(crate) fn fmt(&self, f: &mut dyn core::fmt::Write, variant: Variant) -> core::fmt::Result {
        let base = match variant {
            Variant::Fg => self.fg_base(),
            Variant::Bg => self.fg_base() + 10,
        };

        match *self {
            Color::Fixed(num) => write!(f, "{};5;{}", base, num),
            Color::Rgb(r, g, b) => write!(f, "{};2;{};{};{}", base, r, g, b),
            _ => write!(f, "{}", base)
        }
    }

    #[inline(always)]
    const fn apply(self, a: crate::style::Application) -> Style {
        Style::new().fg(self).apply(a)
    }

    /// Returns a `Style` with a foreground color of `self`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::{Style, Color::*};
    ///
    /// // A style with a foreground color of "yellow".
    /// static DEBUG: Style = Yellow.foreground();
    ///
    /// // This is equivalent to the above.
    /// static DEBUG_S: Style = Style::new().fg(Yellow);
    ///
    /// // The following two are equivalent. The latter is preferred.
    /// static DEBUG_A: Style = Yellow.foreground().bold();
    /// static DEBUG_B: Style = Yellow.bold();
    /// # use yansi::Paint;
    /// # assert_eq!("-".paint(DEBUG_A).to_string(), "-".paint(DEBUG_B).to_string());
    /// ```
    pub const fn foreground(self) -> Style {
        Style::new().fg(self)
    }

    /// Returns a `Style` with a background color of `self`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::{Style, Color::*};
    ///
    /// // A style with a background color of "yellow".
    /// static DEBUG: Style = Yellow.background();
    ///
    /// // This is equivalent to the above.
    /// static DEBUG_S: Style = Style::new().bg(Yellow);
    ///
    /// // The following two are equivalent. The latter is preferred.
    /// static DEBUG_A: Style = Yellow.background().green();
    /// static DEBUG_B: Style = Green.on_yellow();
    /// # use yansi::Paint;
    /// # assert_eq!("-".paint(DEBUG_A).to_string(), "-".paint(DEBUG_B).to_string());
    /// ```
    pub const fn background(self) -> Style {
        Style::new().bg(self)
    }

    bg!([pub const] constructor(Self) -> Style);

    attr!([pub const] constructor(Self) -> Style);

    quirk!([pub const] constructor(Self) -> Style);

    whenever!([pub const] constructor(Self) -> Style);
}

impl Default for Color {
    fn default() -> Self {
        Color::Primary
    }
}

impl From<Color> for Style {
    fn from(color: Color) -> Self {
        color.foreground()
    }
}

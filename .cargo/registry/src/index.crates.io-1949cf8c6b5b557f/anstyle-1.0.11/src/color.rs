/// Any ANSI color code scheme
#[allow(clippy::exhaustive_enums)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Color {
    /// Available 4-bit ANSI color palette codes
    ///
    /// The user's terminal defines the meaning of the each palette code.
    Ansi(AnsiColor),
    /// 256 (8-bit) color support
    ///
    /// - `0..16` are [`AnsiColor`] palette codes
    /// - `0..232` map to [`RgbColor`] color values
    /// - `232..` map to [`RgbColor`] gray-scale values
    Ansi256(Ansi256Color),
    /// 24-bit ANSI RGB color codes
    Rgb(RgbColor),
}

impl Color {
    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub fn on(self, background: impl Into<Color>) -> crate::Style {
        crate::Style::new()
            .fg_color(Some(self))
            .bg_color(Some(background.into()))
    }

    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub const fn on_default(self) -> crate::Style {
        crate::Style::new().fg_color(Some(self))
    }

    /// Render the ANSI code for a foreground color
    #[inline]
    pub fn render_fg(self) -> impl core::fmt::Display + Copy {
        match self {
            Self::Ansi(color) => color.as_fg_buffer(),
            Self::Ansi256(color) => color.as_fg_buffer(),
            Self::Rgb(color) => color.as_fg_buffer(),
        }
    }

    #[inline]
    #[cfg(feature = "std")]
    pub(crate) fn write_fg_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        let buffer = match self {
            Self::Ansi(color) => color.as_fg_buffer(),
            Self::Ansi256(color) => color.as_fg_buffer(),
            Self::Rgb(color) => color.as_fg_buffer(),
        };
        buffer.write_to(write)
    }

    /// Render the ANSI code for a background color
    #[inline]
    pub fn render_bg(self) -> impl core::fmt::Display + Copy {
        match self {
            Self::Ansi(color) => color.as_bg_buffer(),
            Self::Ansi256(color) => color.as_bg_buffer(),
            Self::Rgb(color) => color.as_bg_buffer(),
        }
    }

    #[inline]
    #[cfg(feature = "std")]
    pub(crate) fn write_bg_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        let buffer = match self {
            Self::Ansi(color) => color.as_bg_buffer(),
            Self::Ansi256(color) => color.as_bg_buffer(),
            Self::Rgb(color) => color.as_bg_buffer(),
        };
        buffer.write_to(write)
    }

    #[inline]
    pub(crate) fn render_underline(self) -> impl core::fmt::Display + Copy {
        match self {
            Self::Ansi(color) => color.as_underline_buffer(),
            Self::Ansi256(color) => color.as_underline_buffer(),
            Self::Rgb(color) => color.as_underline_buffer(),
        }
    }

    #[inline]
    #[cfg(feature = "std")]
    pub(crate) fn write_underline_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        let buffer = match self {
            Self::Ansi(color) => color.as_underline_buffer(),
            Self::Ansi256(color) => color.as_underline_buffer(),
            Self::Rgb(color) => color.as_underline_buffer(),
        };
        buffer.write_to(write)
    }
}

impl From<AnsiColor> for Color {
    #[inline]
    fn from(inner: AnsiColor) -> Self {
        Self::Ansi(inner)
    }
}

impl From<Ansi256Color> for Color {
    #[inline]
    fn from(inner: Ansi256Color) -> Self {
        Self::Ansi256(inner)
    }
}

impl From<RgbColor> for Color {
    #[inline]
    fn from(inner: RgbColor) -> Self {
        Self::Rgb(inner)
    }
}

impl From<u8> for Color {
    #[inline]
    fn from(inner: u8) -> Self {
        Self::Ansi256(inner.into())
    }
}

impl From<(u8, u8, u8)> for Color {
    #[inline]
    fn from(inner: (u8, u8, u8)) -> Self {
        Self::Rgb(inner.into())
    }
}

/// Available 4-bit ANSI color palette codes
///
/// The user's terminal defines the meaning of the each palette code.
#[allow(clippy::exhaustive_enums)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AnsiColor {
    /// Black: #0 (foreground code `30`, background code `40`).
    Black,

    /// Red: #1 (foreground code `31`, background code `41`).
    Red,

    /// Green: #2 (foreground code `32`, background code `42`).
    Green,

    /// Yellow: #3 (foreground code `33`, background code `43`).
    Yellow,

    /// Blue: #4 (foreground code `34`, background code `44`).
    Blue,

    /// Magenta: #5 (foreground code `35`, background code `45`).
    Magenta,

    /// Cyan: #6 (foreground code `36`, background code `46`).
    Cyan,

    /// White: #7 (foreground code `37`, background code `47`).
    White,

    /// Bright black: #0 (foreground code `90`, background code `100`).
    BrightBlack,

    /// Bright red: #1 (foreground code `91`, background code `101`).
    BrightRed,

    /// Bright green: #2 (foreground code `92`, background code `102`).
    BrightGreen,

    /// Bright yellow: #3 (foreground code `93`, background code `103`).
    BrightYellow,

    /// Bright blue: #4 (foreground code `94`, background code `104`).
    BrightBlue,

    /// Bright magenta: #5 (foreground code `95`, background code `105`).
    BrightMagenta,

    /// Bright cyan: #6 (foreground code `96`, background code `106`).
    BrightCyan,

    /// Bright white: #7 (foreground code `97`, background code `107`).
    BrightWhite,
}

impl AnsiColor {
    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub fn on(self, background: impl Into<Color>) -> crate::Style {
        crate::Style::new()
            .fg_color(Some(self.into()))
            .bg_color(Some(background.into()))
    }

    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub const fn on_default(self) -> crate::Style {
        crate::Style::new().fg_color(Some(Color::Ansi(self)))
    }

    /// Render the ANSI code for a foreground color
    #[inline]
    pub fn render_fg(self) -> impl core::fmt::Display + Copy {
        NullFormatter(self.as_fg_str())
    }

    #[inline]
    fn as_fg_str(&self) -> &'static str {
        match self {
            Self::Black => escape!("3", "0"),
            Self::Red => escape!("3", "1"),
            Self::Green => escape!("3", "2"),
            Self::Yellow => escape!("3", "3"),
            Self::Blue => escape!("3", "4"),
            Self::Magenta => escape!("3", "5"),
            Self::Cyan => escape!("3", "6"),
            Self::White => escape!("3", "7"),
            Self::BrightBlack => escape!("9", "0"),
            Self::BrightRed => escape!("9", "1"),
            Self::BrightGreen => escape!("9", "2"),
            Self::BrightYellow => escape!("9", "3"),
            Self::BrightBlue => escape!("9", "4"),
            Self::BrightMagenta => escape!("9", "5"),
            Self::BrightCyan => escape!("9", "6"),
            Self::BrightWhite => escape!("9", "7"),
        }
    }

    #[inline]
    fn as_fg_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default().write_str(self.as_fg_str())
    }

    /// Render the ANSI code for a background color
    #[inline]
    pub fn render_bg(self) -> impl core::fmt::Display + Copy {
        NullFormatter(self.as_bg_str())
    }

    #[inline]
    fn as_bg_str(&self) -> &'static str {
        match self {
            Self::Black => escape!("4", "0"),
            Self::Red => escape!("4", "1"),
            Self::Green => escape!("4", "2"),
            Self::Yellow => escape!("4", "3"),
            Self::Blue => escape!("4", "4"),
            Self::Magenta => escape!("4", "5"),
            Self::Cyan => escape!("4", "6"),
            Self::White => escape!("4", "7"),
            Self::BrightBlack => escape!("10", "0"),
            Self::BrightRed => escape!("10", "1"),
            Self::BrightGreen => escape!("10", "2"),
            Self::BrightYellow => escape!("10", "3"),
            Self::BrightBlue => escape!("10", "4"),
            Self::BrightMagenta => escape!("10", "5"),
            Self::BrightCyan => escape!("10", "6"),
            Self::BrightWhite => escape!("10", "7"),
        }
    }

    #[inline]
    fn as_bg_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default().write_str(self.as_bg_str())
    }

    #[inline]
    fn as_underline_buffer(&self) -> DisplayBuffer {
        // No per-color codes; must delegate to `Ansi256Color`
        Ansi256Color::from(*self).as_underline_buffer()
    }

    /// Change the color to/from bright
    #[must_use]
    #[inline]
    pub fn bright(self, yes: bool) -> Self {
        if yes {
            match self {
                Self::Black => Self::BrightBlack,
                Self::Red => Self::BrightRed,
                Self::Green => Self::BrightGreen,
                Self::Yellow => Self::BrightYellow,
                Self::Blue => Self::BrightBlue,
                Self::Magenta => Self::BrightMagenta,
                Self::Cyan => Self::BrightCyan,
                Self::White => Self::BrightWhite,
                Self::BrightBlack => self,
                Self::BrightRed => self,
                Self::BrightGreen => self,
                Self::BrightYellow => self,
                Self::BrightBlue => self,
                Self::BrightMagenta => self,
                Self::BrightCyan => self,
                Self::BrightWhite => self,
            }
        } else {
            match self {
                Self::Black => self,
                Self::Red => self,
                Self::Green => self,
                Self::Yellow => self,
                Self::Blue => self,
                Self::Magenta => self,
                Self::Cyan => self,
                Self::White => self,
                Self::BrightBlack => Self::Black,
                Self::BrightRed => Self::Red,
                Self::BrightGreen => Self::Green,
                Self::BrightYellow => Self::Yellow,
                Self::BrightBlue => Self::Blue,
                Self::BrightMagenta => Self::Magenta,
                Self::BrightCyan => Self::Cyan,
                Self::BrightWhite => Self::White,
            }
        }
    }

    /// Report whether the color is bright
    #[inline]
    pub fn is_bright(self) -> bool {
        match self {
            Self::Black => false,
            Self::Red => false,
            Self::Green => false,
            Self::Yellow => false,
            Self::Blue => false,
            Self::Magenta => false,
            Self::Cyan => false,
            Self::White => false,
            Self::BrightBlack => true,
            Self::BrightRed => true,
            Self::BrightGreen => true,
            Self::BrightYellow => true,
            Self::BrightBlue => true,
            Self::BrightMagenta => true,
            Self::BrightCyan => true,
            Self::BrightWhite => true,
        }
    }
}

/// 256 (8-bit) color support
///
/// - `0..16` are [`AnsiColor`] palette codes
/// - `0..232` map to [`RgbColor`] color values
/// - `232..` map to [`RgbColor`] gray-scale values
#[allow(clippy::exhaustive_structs)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Ansi256Color(pub u8);

impl Ansi256Color {
    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub fn on(self, background: impl Into<Color>) -> crate::Style {
        crate::Style::new()
            .fg_color(Some(self.into()))
            .bg_color(Some(background.into()))
    }

    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub const fn on_default(self) -> crate::Style {
        crate::Style::new().fg_color(Some(Color::Ansi256(self)))
    }

    /// Get the raw value
    #[inline]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Convert to [`AnsiColor`] when there is a 1:1 mapping
    #[inline]
    pub const fn into_ansi(self) -> Option<AnsiColor> {
        match self.index() {
            0 => Some(AnsiColor::Black),
            1 => Some(AnsiColor::Red),
            2 => Some(AnsiColor::Green),
            3 => Some(AnsiColor::Yellow),
            4 => Some(AnsiColor::Blue),
            5 => Some(AnsiColor::Magenta),
            6 => Some(AnsiColor::Cyan),
            7 => Some(AnsiColor::White),
            8 => Some(AnsiColor::BrightBlack),
            9 => Some(AnsiColor::BrightRed),
            10 => Some(AnsiColor::BrightGreen),
            11 => Some(AnsiColor::BrightYellow),
            12 => Some(AnsiColor::BrightBlue),
            13 => Some(AnsiColor::BrightMagenta),
            14 => Some(AnsiColor::BrightCyan),
            15 => Some(AnsiColor::BrightWhite),
            _ => None,
        }
    }

    /// Losslessly convert from [`AnsiColor`]
    #[inline]
    pub const fn from_ansi(color: AnsiColor) -> Self {
        match color {
            AnsiColor::Black => Self(0),
            AnsiColor::Red => Self(1),
            AnsiColor::Green => Self(2),
            AnsiColor::Yellow => Self(3),
            AnsiColor::Blue => Self(4),
            AnsiColor::Magenta => Self(5),
            AnsiColor::Cyan => Self(6),
            AnsiColor::White => Self(7),
            AnsiColor::BrightBlack => Self(8),
            AnsiColor::BrightRed => Self(9),
            AnsiColor::BrightGreen => Self(10),
            AnsiColor::BrightYellow => Self(11),
            AnsiColor::BrightBlue => Self(12),
            AnsiColor::BrightMagenta => Self(13),
            AnsiColor::BrightCyan => Self(14),
            AnsiColor::BrightWhite => Self(15),
        }
    }

    /// Render the ANSI code for a foreground color
    #[inline]
    pub fn render_fg(self) -> impl core::fmt::Display + Copy {
        self.as_fg_buffer()
    }

    #[inline]
    fn as_fg_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default()
            .write_str("\x1B[38;5;")
            .write_code(self.index())
            .write_str("m")
    }

    /// Render the ANSI code for a background color
    #[inline]
    pub fn render_bg(self) -> impl core::fmt::Display + Copy {
        self.as_bg_buffer()
    }

    #[inline]
    fn as_bg_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default()
            .write_str("\x1B[48;5;")
            .write_code(self.index())
            .write_str("m")
    }

    #[inline]
    fn as_underline_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default()
            .write_str("\x1B[58;5;")
            .write_code(self.index())
            .write_str("m")
    }
}

impl From<u8> for Ansi256Color {
    #[inline]
    fn from(inner: u8) -> Self {
        Self(inner)
    }
}

impl From<AnsiColor> for Ansi256Color {
    #[inline]
    fn from(inner: AnsiColor) -> Self {
        Self::from_ansi(inner)
    }
}

/// 24-bit ANSI RGB color codes
#[allow(clippy::exhaustive_structs)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub fn on(self, background: impl Into<Color>) -> crate::Style {
        crate::Style::new()
            .fg_color(Some(self.into()))
            .bg_color(Some(background.into()))
    }

    /// Create a [`Style`][crate::Style] with this as the foreground
    #[inline]
    pub const fn on_default(self) -> crate::Style {
        crate::Style::new().fg_color(Some(Color::Rgb(self)))
    }

    /// Red
    #[inline]
    pub const fn r(self) -> u8 {
        self.0
    }

    /// Green
    #[inline]
    pub const fn g(self) -> u8 {
        self.1
    }

    /// Blue
    #[inline]
    pub const fn b(self) -> u8 {
        self.2
    }

    /// Render the ANSI code for a foreground color
    #[inline]
    pub fn render_fg(self) -> impl core::fmt::Display + Copy {
        self.as_fg_buffer()
    }

    #[inline]
    fn as_fg_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default()
            .write_str("\x1B[38;2;")
            .write_code(self.r())
            .write_str(";")
            .write_code(self.g())
            .write_str(";")
            .write_code(self.b())
            .write_str("m")
    }

    /// Render the ANSI code for a background color
    #[inline]
    pub fn render_bg(self) -> impl core::fmt::Display + Copy {
        self.as_bg_buffer()
    }

    #[inline]
    fn as_bg_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default()
            .write_str("\x1B[48;2;")
            .write_code(self.r())
            .write_str(";")
            .write_code(self.g())
            .write_str(";")
            .write_code(self.b())
            .write_str("m")
    }

    #[inline]
    fn as_underline_buffer(&self) -> DisplayBuffer {
        DisplayBuffer::default()
            .write_str("\x1B[58;2;")
            .write_code(self.r())
            .write_str(";")
            .write_code(self.g())
            .write_str(";")
            .write_code(self.b())
            .write_str("m")
    }
}

impl From<(u8, u8, u8)> for RgbColor {
    #[inline]
    fn from(inner: (u8, u8, u8)) -> Self {
        let (r, g, b) = inner;
        Self(r, g, b)
    }
}

const DISPLAY_BUFFER_CAPACITY: usize = 19;

#[derive(Copy, Clone, Default, Debug)]
struct DisplayBuffer {
    buffer: [u8; DISPLAY_BUFFER_CAPACITY],
    len: usize,
}

impl DisplayBuffer {
    #[must_use]
    #[inline(never)]
    fn write_str(mut self, part: &'static str) -> Self {
        for (i, b) in part.as_bytes().iter().enumerate() {
            self.buffer[self.len + i] = *b;
        }
        self.len += part.len();
        self
    }

    #[must_use]
    #[inline(never)]
    fn write_code(mut self, code: u8) -> Self {
        let c1: u8 = (code / 100) % 10;
        let c2: u8 = (code / 10) % 10;
        let c3: u8 = code % 10;

        let mut printed = true;
        if c1 != 0 {
            printed = true;
            self.buffer[self.len] = b'0' + c1;
            self.len += 1;
        }
        if c2 != 0 || printed {
            self.buffer[self.len] = b'0' + c2;
            self.len += 1;
        }
        // If we received a zero value we must still print a value.
        self.buffer[self.len] = b'0' + c3;
        self.len += 1;

        self
    }

    #[inline]
    fn as_str(&self) -> &str {
        // SAFETY: Only `&str` can be written to the buffer
        #[allow(unsafe_code)]
        unsafe {
            core::str::from_utf8_unchecked(&self.buffer[0..self.len])
        }
    }

    #[inline]
    #[cfg(feature = "std")]
    fn write_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        write.write_all(self.as_str().as_bytes())
    }
}

impl core::fmt::Display for DisplayBuffer {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Copy, Clone, Default, Debug)]
struct NullFormatter(&'static str);

impl core::fmt::Display for NullFormatter {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod test {
    use super::*;

    #[test]
    fn max_display_buffer() {
        let c = RgbColor(255, 255, 255);
        let actual = c.render_fg().to_string();
        assert_eq!(actual, "\u{1b}[38;2;255;255;255m");
        assert_eq!(actual.len(), DISPLAY_BUFFER_CAPACITY);
    }

    #[test]
    fn print_size_of() {
        use core::mem::size_of;
        dbg!(size_of::<Color>());
        dbg!(size_of::<AnsiColor>());
        dbg!(size_of::<Ansi256Color>());
        dbg!(size_of::<RgbColor>());
        dbg!(size_of::<DisplayBuffer>());
    }

    #[test]
    fn no_align() {
        #[track_caller]
        fn assert_no_align(d: impl core::fmt::Display) {
            let expected = format!("{d}");
            let actual = format!("{d:<10}");
            assert_eq!(expected, actual);
        }

        assert_no_align(AnsiColor::White.render_fg());
        assert_no_align(AnsiColor::White.render_bg());
        assert_no_align(Ansi256Color(0).render_fg());
        assert_no_align(Ansi256Color(0).render_bg());
        assert_no_align(RgbColor(0, 0, 0).render_fg());
        assert_no_align(RgbColor(0, 0, 0).render_bg());
        assert_no_align(Color::Ansi(AnsiColor::White).render_fg());
        assert_no_align(Color::Ansi(AnsiColor::White).render_bg());
    }
}

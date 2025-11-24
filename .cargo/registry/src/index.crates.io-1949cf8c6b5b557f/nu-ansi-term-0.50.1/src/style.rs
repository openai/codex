/// A style is a collection of properties that can format a string
/// using ANSI escape codes.
///
/// # Examples
///
/// ```
/// use nu_ansi_term::{Style, Color};
///
/// let style = Style::new().bold().on(Color::Black);
/// println!("{}", style.paint("Bold on black"));
/// ```
#[derive(Eq, PartialEq, Clone, Copy)]
#[cfg_attr(
    feature = "derive_serde_style",
    derive(serde::Deserialize, serde::Serialize)
)]
pub struct Style {
    /// The style's foreground color, if it has one.
    pub foreground: Option<Color>,

    /// The style's background color, if it has one.
    pub background: Option<Color>,

    /// Whether this style is bold.
    pub is_bold: bool,

    /// Whether this style is dimmed.
    pub is_dimmed: bool,

    /// Whether this style is italic.
    pub is_italic: bool,

    /// Whether this style is underlined.
    pub is_underline: bool,

    /// Whether this style is blinking.
    pub is_blink: bool,

    /// Whether this style has reverse colors.
    pub is_reverse: bool,

    /// Whether this style is hidden.
    pub is_hidden: bool,

    /// Whether this style is struckthrough.
    pub is_strikethrough: bool,

    /// Wether this style is always displayed starting with a reset code to clear any remaining style artifacts
    pub prefix_with_reset: bool,
}

impl Style {
    /// Creates a new Style with no properties set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new();
    /// println!("{}", style.paint("hi"));
    /// ```
    pub fn new() -> Style {
        Style::default()
    }

    /// Returns a [`Style`] with the `Style.prefix_with_reset` property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().reset_before_style();
    /// println!("{}", style.paint("hey"));
    /// ```
    pub const fn reset_before_style(&self) -> Style {
        Style {
            prefix_with_reset: true,
            ..*self
        }
    }

    /// Returns a `Style` with the bold property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().bold();
    /// println!("{}", style.paint("hey"));
    /// ```
    pub const fn bold(&self) -> Style {
        Style {
            is_bold: true,
            ..*self
        }
    }

    /// Returns a `Style` with the dimmed property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().dimmed();
    /// println!("{}", style.paint("sup"));
    /// ```
    pub const fn dimmed(&self) -> Style {
        Style {
            is_dimmed: true,
            ..*self
        }
    }

    /// Returns a `Style` with the italic property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().italic();
    /// println!("{}", style.paint("greetings"));
    /// ```
    pub const fn italic(&self) -> Style {
        Style {
            is_italic: true,
            ..*self
        }
    }

    /// Returns a `Style` with the underline property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().underline();
    /// println!("{}", style.paint("salutations"));
    /// ```
    pub const fn underline(&self) -> Style {
        Style {
            is_underline: true,
            ..*self
        }
    }

    /// Returns a `Style` with the blink property set.
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().blink();
    /// println!("{}", style.paint("wazzup"));
    /// ```
    pub const fn blink(&self) -> Style {
        Style {
            is_blink: true,
            ..*self
        }
    }

    /// Returns a `Style` with the reverse property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().reverse();
    /// println!("{}", style.paint("aloha"));
    /// ```
    pub const fn reverse(&self) -> Style {
        Style {
            is_reverse: true,
            ..*self
        }
    }

    /// Returns a `Style` with the hidden property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().hidden();
    /// println!("{}", style.paint("ahoy"));
    /// ```
    pub const fn hidden(&self) -> Style {
        Style {
            is_hidden: true,
            ..*self
        }
    }

    /// Returns a `Style` with the strikethrough property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// let style = Style::new().strikethrough();
    /// println!("{}", style.paint("yo"));
    /// ```
    pub const fn strikethrough(&self) -> Style {
        Style {
            is_strikethrough: true,
            ..*self
        }
    }

    /// Returns a `Style` with the foreground color property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::{Style, Color};
    ///
    /// let style = Style::new().fg(Color::Yellow);
    /// println!("{}", style.paint("hi"));
    /// ```
    pub const fn fg(&self, foreground: Color) -> Style {
        Style {
            foreground: Some(foreground),
            ..*self
        }
    }

    /// Returns a `Style` with the background color property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::{Style, Color};
    ///
    /// let style = Style::new().on(Color::Blue);
    /// println!("{}", style.paint("eyyyy"));
    /// ```
    pub const fn on(&self, background: Color) -> Style {
        Style {
            background: Some(background),
            ..*self
        }
    }

    /// Return true if this `Style` has no actual styles, and can be written
    /// without any control characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Style;
    ///
    /// assert_eq!(true,  Style::default().is_plain());
    /// assert_eq!(false, Style::default().bold().is_plain());
    /// ```
    pub fn is_plain(self) -> bool {
        self == Style::default()
    }
}

impl Default for Style {
    /// Returns a style with *no* properties set. Formatting text using this
    /// style returns the exact same text.
    ///
    /// ```
    /// use nu_ansi_term::Style;
    /// assert_eq!(None,  Style::default().foreground);
    /// assert_eq!(None,  Style::default().background);
    /// assert_eq!(false, Style::default().is_bold);
    /// assert_eq!("txt", Style::default().paint("txt").to_string());
    /// ```
    fn default() -> Style {
        Style {
            foreground: None,
            background: None,
            is_bold: false,
            is_dimmed: false,
            is_italic: false,
            is_underline: false,
            is_blink: false,
            is_reverse: false,
            is_hidden: false,
            is_strikethrough: false,
            prefix_with_reset: false,
        }
    }
}

// ---- colors ----

/// A color is one specific type of ANSI escape code, and can refer
/// to either the foreground or background color.
///
/// These use the standard numeric sequences.
/// See <http://invisible-island.net/xterm/ctlseqs/ctlseqs.html>
#[derive(Eq, PartialEq, Clone, Copy, Debug, Default)]
#[cfg_attr(
    feature = "derive_serde_style",
    derive(serde::Deserialize, serde::Serialize)
)]
pub enum Color {
    /// Color #0 (foreground code `30`, background code `40`).
    ///
    /// This is not necessarily the background color, and using it as one may
    /// render the text hard to read on terminals with dark backgrounds.
    Black,

    /// Color #0 (foreground code `90`, background code `100`).
    DarkGray,

    /// Color #1 (foreground code `31`, background code `41`).
    Red,

    /// Color #1 (foreground code `91`, background code `101`).
    LightRed,

    /// Color #2 (foreground code `32`, background code `42`).
    Green,

    /// Color #2 (foreground code `92`, background code `102`).
    LightGreen,

    /// Color #3 (foreground code `33`, background code `43`).
    Yellow,

    /// Color #3 (foreground code `93`, background code `103`).
    LightYellow,

    /// Color #4 (foreground code `34`, background code `44`).
    Blue,

    /// Color #4 (foreground code `94`, background code `104`).
    LightBlue,

    /// Color #5 (foreground code `35`, background code `45`).
    Purple,

    /// Color #5 (foreground code `95`, background code `105`).
    LightPurple,

    /// Color #5 (foreground code `35`, background code `45`).
    Magenta,

    /// Color #5 (foreground code `95`, background code `105`).
    LightMagenta,

    /// Color #6 (foreground code `36`, background code `46`).
    Cyan,

    /// Color #6 (foreground code `96`, background code `106`).
    LightCyan,

    /// Color #7 (foreground code `37`, background code `47`).
    ///
    /// As above, this is not necessarily the foreground color, and may be
    /// hard to read on terminals with light backgrounds.
    White,

    /// Color #7 (foreground code `97`, background code `107`).
    LightGray,

    /// A color number from 0 to 255, for use in 256-color terminal
    /// environments.
    ///
    /// - colors 0 to 7 are the `Black` to `White` variants respectively.
    ///   These colors can usually be changed in the terminal emulator.
    /// - colors 8 to 15 are brighter versions of the eight colors above.
    ///   These can also usually be changed in the terminal emulator, or it
    ///   could be configured to use the original colors and show the text in
    ///   bold instead. It varies depending on the program.
    /// - colors 16 to 231 contain several palettes of bright colors,
    ///   arranged in six squares measuring six by six each.
    /// - colors 232 to 255 are shades of grey from black to white.
    ///
    /// It might make more sense to look at a [color chart][cc].
    ///
    /// [cc]: https://upload.wikimedia.org/wikipedia/commons/1/15/Xterm_256color_chart.svg
    Fixed(u8),

    /// A 24-bit Rgb color, as specified by ISO-8613-3.
    Rgb(u8, u8, u8),

    /// The default color (foreground code `39`, background codr `49`).
    #[default]
    Default,
}

impl Color {
    /// Returns a `Style` with the foreground color set to this color.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Red.normal();
    /// println!("{}", style.paint("hi"));
    /// ```
    pub fn normal(self) -> Style {
        Style {
            foreground: Some(self),
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// bold property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Green.bold();
    /// println!("{}", style.paint("hey"));
    /// ```
    pub fn bold(self) -> Style {
        Style {
            foreground: Some(self),
            is_bold: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// dimmed property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Yellow.dimmed();
    /// println!("{}", style.paint("sup"));
    /// ```
    pub fn dimmed(self) -> Style {
        Style {
            foreground: Some(self),
            is_dimmed: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// italic property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Blue.italic();
    /// println!("{}", style.paint("greetings"));
    /// ```
    pub fn italic(self) -> Style {
        Style {
            foreground: Some(self),
            is_italic: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// underline property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Purple.underline();
    /// println!("{}", style.paint("salutations"));
    /// ```
    pub fn underline(self) -> Style {
        Style {
            foreground: Some(self),
            is_underline: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// blink property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Cyan.blink();
    /// println!("{}", style.paint("wazzup"));
    /// ```
    pub fn blink(self) -> Style {
        Style {
            foreground: Some(self),
            is_blink: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// reverse property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Black.reverse();
    /// println!("{}", style.paint("aloha"));
    /// ```
    pub fn reverse(self) -> Style {
        Style {
            foreground: Some(self),
            is_reverse: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// hidden property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::White.hidden();
    /// println!("{}", style.paint("ahoy"));
    /// ```
    pub fn hidden(self) -> Style {
        Style {
            foreground: Some(self),
            is_hidden: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// strikethrough property set.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Fixed(244).strikethrough();
    /// println!("{}", style.paint("yo"));
    /// ```
    pub fn strikethrough(self) -> Style {
        Style {
            foreground: Some(self),
            is_strikethrough: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` thats resets all styling before applying
    /// the foreground color set to this color.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Fixed(244).reset_before_style();
    /// println!("{}", style.paint("yo"));
    /// ```
    pub fn reset_before_style(self) -> Style {
        Style {
            foreground: Some(self),
            prefix_with_reset: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// background color property set to the given color.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color;
    ///
    /// let style = Color::Rgb(31, 31, 31).on(Color::White);
    /// println!("{}", style.paint("eyyyy"));
    /// ```
    pub fn on(self, background: Color) -> Style {
        Style {
            foreground: Some(self),
            background: Some(background),
            ..Style::default()
        }
    }
}

impl From<Color> for Style {
    /// You can turn a `Color` into a `Style` with the foreground color set
    /// with the `From` trait.
    ///
    /// ```
    /// use nu_ansi_term::{Style, Color};
    /// let green_foreground = Style::default().fg(Color::Green);
    /// assert_eq!(green_foreground, Color::Green.normal());
    /// assert_eq!(green_foreground, Color::Green.into());
    /// assert_eq!(green_foreground, Style::from(Color::Green));
    /// ```
    fn from(color: Color) -> Style {
        color.normal()
    }
}

#[cfg(test)]
#[cfg(feature = "derive_serde_style")]
mod serde_json_tests {
    use super::{Color, Style};

    #[test]
    fn color_serialization() {
        let colors = &[
            Color::Red,
            Color::Blue,
            Color::Rgb(123, 123, 123),
            Color::Fixed(255),
        ];

        assert_eq!(
            serde_json::to_string(&colors).unwrap(),
            "[\"Red\",\"Blue\",{\"Rgb\":[123,123,123]},{\"Fixed\":255}]"
        );
    }

    #[test]
    fn color_deserialization() {
        let colors = [
            Color::Red,
            Color::Blue,
            Color::Rgb(123, 123, 123),
            Color::Fixed(255),
        ];

        for color in colors {
            let serialized = serde_json::to_string(&color).unwrap();
            let deserialized: Color = serde_json::from_str(&serialized).unwrap();

            assert_eq!(color, deserialized);
        }
    }

    #[test]
    fn style_serialization() {
        let style = Style::default();

        assert_eq!(serde_json::to_string(&style).unwrap(), "{\"foreground\":null,\"background\":null,\"is_bold\":false,\"is_dimmed\":false,\"is_italic\":false,\"is_underline\":false,\"is_blink\":false,\"is_reverse\":false,\"is_hidden\":false,\"is_strikethrough\":false,\"prefix_with_reset\":false}".to_string());
    }
}

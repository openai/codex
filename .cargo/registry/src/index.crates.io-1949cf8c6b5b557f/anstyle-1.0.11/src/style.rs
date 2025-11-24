use crate::reset::RESET;

/// ANSI Text styling
///
/// You can print a `Style` to render the corresponding ANSI code.
/// Using the alternate flag `#` will render the ANSI reset code, if needed.
/// Together, this makes it convenient to render styles using inline format arguments.
///
/// # Examples
///
/// ```rust
/// let style = anstyle::Style::new().bold();
///
/// let value = 42;
/// println!("{style}{value}{style:#}");
/// ```
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Style {
    fg: Option<crate::Color>,
    bg: Option<crate::Color>,
    underline: Option<crate::Color>,
    effects: crate::Effects,
}

/// # Core
impl Style {
    /// No effects enabled
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            underline: None,
            effects: crate::Effects::new(),
        }
    }

    /// Set foreground color
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().fg_color(Some(anstyle::AnsiColor::Red.into()));
    /// ```
    #[must_use]
    #[inline]
    pub const fn fg_color(mut self, fg: Option<crate::Color>) -> Self {
        self.fg = fg;
        self
    }

    /// Set background color
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().bg_color(Some(anstyle::AnsiColor::Red.into()));
    /// ```
    #[must_use]
    #[inline]
    pub const fn bg_color(mut self, bg: Option<crate::Color>) -> Self {
        self.bg = bg;
        self
    }

    /// Set underline color
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().underline_color(Some(anstyle::AnsiColor::Red.into()));
    /// ```
    #[must_use]
    #[inline]
    pub const fn underline_color(mut self, underline: Option<crate::Color>) -> Self {
        self.underline = underline;
        self
    }

    /// Set text effects
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().effects(anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE);
    /// ```
    #[must_use]
    #[inline]
    pub const fn effects(mut self, effects: crate::Effects) -> Self {
        self.effects = effects;
        self
    }

    /// Render the ANSI code
    ///
    /// `Style` also implements `Display` directly, so calling this method is optional.
    #[inline]
    pub fn render(self) -> impl core::fmt::Display + Copy {
        StyleDisplay(self)
    }

    fn fmt_to(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::fmt::Display as _;

        self.effects.render().fmt(f)?;

        if let Some(fg) = self.fg {
            fg.render_fg().fmt(f)?;
        }

        if let Some(bg) = self.bg {
            bg.render_bg().fmt(f)?;
        }

        if let Some(underline) = self.underline {
            underline.render_underline().fmt(f)?;
        }

        Ok(())
    }

    /// Write the ANSI code
    #[inline]
    #[cfg(feature = "std")]
    pub fn write_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.effects.write_to(write)?;

        if let Some(fg) = self.fg {
            fg.write_fg_to(write)?;
        }

        if let Some(bg) = self.bg {
            bg.write_bg_to(write)?;
        }

        if let Some(underline) = self.underline {
            underline.write_underline_to(write)?;
        }

        Ok(())
    }

    /// Renders the relevant [`Reset`][crate::Reset] code
    ///
    /// Unlike [`Reset::render`][crate::Reset::render], this will elide the code if there is nothing to reset.
    #[inline]
    pub fn render_reset(self) -> impl core::fmt::Display + Copy {
        if self != Self::new() {
            RESET
        } else {
            ""
        }
    }

    /// Write the relevant [`Reset`][crate::Reset] code
    ///
    /// Unlike [`Reset::render`][crate::Reset::render], this will elide the code if there is nothing to reset.
    #[inline]
    #[cfg(feature = "std")]
    pub fn write_reset_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        if self != Self::new() {
            write.write_all(RESET.as_bytes())
        } else {
            Ok(())
        }
    }
}

/// # Convenience
impl Style {
    /// Apply `bold` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().bold();
    /// ```
    #[must_use]
    #[inline]
    pub const fn bold(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::BOLD);
        self
    }

    /// Apply `dimmed` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().dimmed();
    /// ```
    #[must_use]
    #[inline]
    pub const fn dimmed(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::DIMMED);
        self
    }

    /// Apply `italic` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().italic();
    /// ```
    #[must_use]
    #[inline]
    pub const fn italic(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::ITALIC);
        self
    }

    /// Apply `underline` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().underline();
    /// ```
    #[must_use]
    #[inline]
    pub const fn underline(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::UNDERLINE);
        self
    }

    /// Apply `blink` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().blink();
    /// ```
    #[must_use]
    #[inline]
    pub const fn blink(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::BLINK);
        self
    }

    /// Apply `invert` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().invert();
    /// ```
    #[must_use]
    #[inline]
    pub const fn invert(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::INVERT);
        self
    }

    /// Apply `hidden` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().hidden();
    /// ```
    #[must_use]
    #[inline]
    pub const fn hidden(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::HIDDEN);
        self
    }

    /// Apply `strikethrough` effect
    ///
    /// # Examples
    ///
    /// ```rust
    /// let style = anstyle::Style::new().strikethrough();
    /// ```
    #[must_use]
    #[inline]
    pub const fn strikethrough(mut self) -> Self {
        self.effects = self.effects.insert(crate::Effects::STRIKETHROUGH);
        self
    }
}

/// # Reflection
impl Style {
    /// Get the foreground color
    #[inline]
    pub const fn get_fg_color(self) -> Option<crate::Color> {
        self.fg
    }

    /// Get the background color
    #[inline]
    #[allow(missing_docs)]
    pub const fn get_bg_color(self) -> Option<crate::Color> {
        self.bg
    }

    #[inline]
    #[allow(missing_docs)]
    pub const fn get_underline_color(self) -> Option<crate::Color> {
        self.underline
    }

    #[inline]
    #[allow(missing_docs)]
    pub const fn get_effects(self) -> crate::Effects {
        self.effects
    }

    /// Check if no styling is enabled
    #[inline]
    pub const fn is_plain(self) -> bool {
        self.fg.is_none()
            && self.bg.is_none()
            && self.underline.is_none()
            && self.effects.is_plain()
    }
}

/// # Examples
///
/// ```rust
/// let style: anstyle::Style = anstyle::Effects::BOLD.into();
/// ```
impl From<crate::Effects> for Style {
    #[inline]
    fn from(effects: crate::Effects) -> Self {
        Self::new().effects(effects)
    }
}

/// # Examples
///
/// ```rust
/// let style = anstyle::Style::new() | anstyle::Effects::BOLD.into();
/// ```
impl core::ops::BitOr<crate::Effects> for Style {
    type Output = Self;

    #[inline(always)]
    fn bitor(mut self, rhs: crate::Effects) -> Self {
        self.effects |= rhs;
        self
    }
}

/// # Examples
///
/// ```rust
/// let mut style = anstyle::Style::new();
/// style |= anstyle::Effects::BOLD.into();
/// ```
impl core::ops::BitOrAssign<crate::Effects> for Style {
    #[inline]
    fn bitor_assign(&mut self, other: crate::Effects) {
        self.effects |= other;
    }
}

/// # Examples
///
/// ```rust
/// let style = anstyle::Style::new().bold().underline() - anstyle::Effects::BOLD.into();
/// ```
impl core::ops::Sub<crate::Effects> for Style {
    type Output = Self;

    #[inline]
    fn sub(mut self, other: crate::Effects) -> Self {
        self.effects -= other;
        self
    }
}

/// # Examples
///
/// ```rust
/// let mut style = anstyle::Style::new().bold().underline();
/// style -= anstyle::Effects::BOLD.into();
/// ```
impl core::ops::SubAssign<crate::Effects> for Style {
    #[inline]
    fn sub_assign(&mut self, other: crate::Effects) {
        self.effects -= other;
    }
}

/// # Examples
///
/// ```rust
/// let effects = anstyle::Effects::BOLD;
/// assert_eq!(anstyle::Style::new().effects(effects), effects);
/// assert_ne!(anstyle::Effects::UNDERLINE | effects, effects);
/// assert_ne!(anstyle::RgbColor(0, 0, 0).on_default() | effects, effects);
/// ```
impl PartialEq<crate::Effects> for Style {
    #[inline]
    fn eq(&self, other: &crate::Effects) -> bool {
        let other = Self::from(*other);
        *self == other
    }
}

impl core::fmt::Display for Style {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            self.render_reset().fmt(f)
        } else {
            self.fmt_to(f)
        }
    }
}

#[derive(Copy, Clone, Default, Debug)]
struct StyleDisplay(Style);

impl core::fmt::Display for StyleDisplay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt_to(f)
    }
}

#[test]
#[cfg(feature = "std")]
fn print_size_of() {
    use core::mem::size_of;
    dbg!(size_of::<Style>());
    dbg!(size_of::<StyleDisplay>());
}

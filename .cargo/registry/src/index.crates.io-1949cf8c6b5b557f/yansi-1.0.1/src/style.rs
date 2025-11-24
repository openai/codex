use core::fmt::{self, Write};

use crate::color::{Color, Variant};
use crate::attr_quirk::{Attribute, Quirk};
use crate::condition::Condition;
use crate::set::Set;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{string::String, borrow::Cow};

#[cfg(feature = "std")]
use std::borrow::Cow;

/// A set of styling options.
///
/// ## Equivalence and Ordering
///
/// Only a style's `foreground`, `background`, and set of `attributes` are
/// considered when testing for equivalence or producing an ordering via
/// `PartialEq` or `Eq`, and `PartialOrd` or `Ord`. A style's quirks and
/// conditions are ignored.
#[derive(Default, Debug, Copy, Clone)]
pub struct Style {
    /// The foreground color. Defaults to `None`.
    ///
    /// ```rust
    /// use yansi::{Style, Color};
    ///
    /// assert_eq!(Style::new().foreground, None);
    /// assert_eq!(Style::new().green().foreground, Some(Color::Green));
    /// ```
    pub foreground: Option<Color>,
    /// The background color. Defaults to `None`.
    ///
    /// ```rust
    /// use yansi::{Style, Color};
    ///
    /// assert_eq!(Style::new().background, None);
    /// assert_eq!(Style::new().on_red().background, Some(Color::Red));
    /// ```
    pub background: Option<Color>,
    pub(crate) attributes: Set<Attribute>,
    pub(crate) quirks: Set<Quirk>,
    /// The condition.
    ///
    /// To check a style's condition directly, use [`Style::enabled()`]:
    ///
    /// ```rust
    /// use yansi::{Style, Condition};
    ///
    /// let style = Style::new().whenever(Condition::ALWAYS);
    /// assert!(style.enabled());
    ///
    /// let style = Style::new().whenever(Condition::NEVER);
    /// assert!(!style.enabled());
    /// ```
    pub condition: Option<Condition>,
}

struct AnsiSplicer<'a> {
    f: &'a mut dyn fmt::Write,
    splice: bool,
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum Application {
    fg(Color),
    bg(Color),
    attr(Attribute),
    quirk(Quirk),
    whenever(Condition),
}

impl Style {
    const DEFAULT: Style = Style {
        foreground: None,
        background: None,
        attributes: Set::EMPTY,
        quirks: Set::EMPTY,
        condition: None,
    };

    /// Returns a new style with no foreground or background, no attributes
    /// or quirks, and [`Condition::DEFAULT`].
    ///
    /// This is the default returned by [`Default::default()`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::Style;
    ///
    /// assert_eq!(Style::new(), Style::default());
    /// ```
    #[inline]
    pub const fn new() -> Style {
        Style::DEFAULT
    }

    #[inline(always)]
    pub(crate) const fn apply(mut self, a: Application) -> Style {
        match a {
            Application::fg(color) => self.foreground = Some(color),
            Application::bg(color) => self.background = Some(color),
            Application::whenever(cond) => self.condition = Some(cond),
            Application::attr(attr) => self.attributes = self.attributes.insert(attr),
            Application::quirk(quirk) => self.quirks = self.quirks.insert(quirk),
        }

        self
    }

    /// Returns `true` if this style is enabled, based on
    /// [`condition`](Paint.condition).
    ///
    /// **Note:** _For a style to be effected, both this method **and**
    /// [`yansi::is_enabled()`](crate::is_enabled) must return `true`._
    ///
    /// When there is no condition set, this method always returns `true`. When
    /// a condition has been set, this evaluates the condition and returns the
    /// result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::{Style, Condition};
    ///
    /// let style = Style::new().whenever(Condition::ALWAYS);
    /// assert!(style.enabled());
    ///
    /// let style = Style::new().whenever(Condition::NEVER);
    /// assert!(!style.enabled());
    /// ```
    pub fn enabled(&self) -> bool {
        self.condition.map_or(true, |c| c())
    }

    /// Writes the ANSI code prefix for the currently set styles.
    ///
    /// This method is intended to be used inside of [`fmt::Display`] and
    /// [`fmt::Debug`] implementations for custom or specialized use-cases. Most
    /// users should use [`Painted`] for all painting needs.
    ///
    /// This method writes the ANSI code prefix irrespective of whether painting
    /// is currently enabled or disabled. To write the prefix only if painting
    /// is enabled, condition a call to this method on [`is_enabled()`].
    ///
    /// [`fmt::Display`]: fmt::Display
    /// [`fmt::Debug`]: fmt::Debug
    /// [`Painted`]: crate::Painted
    /// [`is_enabled()`]: crate::is_enabled()
    ///
    /// # Example
    ///
    /// ```rust
    /// use core::fmt;
    /// use yansi::Style;
    ///
    /// struct CustomItem {
    ///     item: u32,
    ///     style: Style
    /// }
    ///
    /// impl fmt::Display for CustomItem {
    ///     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    ///         self.style.fmt_prefix(f)?;
    ///         write!(f, "number: {}", self.item)?;
    ///         self.style.fmt_suffix(f)
    ///     }
    /// }
    /// ```
    pub fn fmt_prefix(&self, f: &mut dyn fmt::Write) -> fmt::Result {
        // Give a sequence-free string when no styles are applied.
        if self == &Style::DEFAULT {
            return Ok(());
        }

        let brighten = |color: Option<Color>, bright: bool| match (color, bright) {
            (Some(color), true) => Some(color.to_bright()),
            _ => color
        };

        let mut f = AnsiSplicer { f, splice: false };
        f.write_str("\x1B[")?;

        for attr in self.attributes.iter() {
            f.splice()?;
            attr.fmt(&mut f)?;
        }

        if let Some(color) = brighten(self.background, self.quirks.contains(Quirk::OnBright)) {
            f.splice()?;
            color.fmt(&mut f, Variant::Bg)?;
        }

        if let Some(color) = brighten(self.foreground, self.quirks.contains(Quirk::Bright)) {
            f.splice()?;
            color.fmt(&mut f, Variant::Fg)?;
        }

        // All of the sequences end with an `m`.
        f.write_char('m')
    }

    /// Returns the ANSI code sequence prefix for the style as a string.
    ///
    /// This returns a string with the exact same sequence written by
    /// [`fmt_prefix()`](Self::fmt_prefix()). See that method for details.
    #[cfg(feature = "alloc")]
    #[cfg_attr(feature = "_nightly", doc(cfg(feature = "alloc")))]
    pub fn prefix(&self) -> Cow<'static, str> {
        let mut prefix = String::new();
        let _ = self.fmt_prefix(&mut prefix);
        prefix.into()
    }

    /// Writes the ANSI code sequence suffix for the style.
    ///
    /// This method is intended to be used inside of [`fmt::Display`] and
    /// [`fmt::Debug`] implementations for custom or specialized use-cases. Most
    /// users should use [`Painted`] for all painting needs.
    ///
    /// This method writes the ANSI code suffix irrespective of whether painting
    /// is currently enabled or disabled. To write the suffix only if painting
    /// is enabled, condition a call to this method on [`is_enabled()`].
    ///
    /// [`fmt::Display`]: fmt::Display
    /// [`fmt::Debug`]: fmt::Debug
    /// [`Painted`]: crate::Painted
    /// [`is_enabled()`]: crate::is_enabled()
    ///
    /// # Example
    ///
    /// ```rust
    /// use core::fmt;
    /// use yansi::Style;
    ///
    /// struct CustomItem {
    ///     item: u32,
    ///     style: Style
    /// }
    ///
    /// impl fmt::Display for CustomItem {
    ///     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    ///         self.style.fmt_prefix(f)?;
    ///         write!(f, "number: {}", self.item)?;
    ///         self.style.fmt_suffix(f)
    ///     }
    /// }
    /// ```
    pub fn fmt_suffix(&self, f: &mut dyn fmt::Write) -> fmt::Result {
        if !self.quirks.contains(Quirk::Resetting) && !self.quirks.contains(Quirk::Clear) {
            if self.quirks.contains(Quirk::Linger) || self == &Style::DEFAULT {
                return Ok(());
            }
        }

        f.write_str("\x1B[0m")
    }

    /// Returns the ANSI code sequence suffix for the style as a string.
    ///
    /// This returns a string with the exact same sequence written by
    /// [`fmt_suffix()`](Self::fmt_suffix()). See that method for details.
    #[cfg(feature = "alloc")]
    #[cfg_attr(feature = "_nightly", doc(cfg(feature = "alloc")))]
    pub fn suffix(&self) -> Cow<'static, str> {
        if !self.quirks.contains(Quirk::Resetting) && !self.quirks.contains(Quirk::Clear) {
            if self.quirks.contains(Quirk::Linger) || self == &Style::DEFAULT {
                return Cow::from("");
            }
        }

        Cow::from("\x1B[0m")
    }

    properties!([pub const] constructor(Self) -> Self);
}

impl AnsiSplicer<'_> {
    fn splice(&mut self) -> fmt::Result {
        if self.splice { self.f.write_char(';')?; }
        self.splice = true;
        Ok(())
    }
}

impl fmt::Write for AnsiSplicer<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.f.write_str(s)
    }
}

impl PartialEq for Style {
    fn eq(&self, other: &Self) -> bool {
        let Style {
            foreground: fg_a,
            background: bg_a,
            attributes: attrs_a,
            quirks: _,
            condition: _,
        } = self;

        let Style {
            foreground: fg_b,
            background: bg_b,
            attributes: attrs_b,
            quirks: _,
            condition: _,
        } = other;

        fg_a == fg_b && bg_a == bg_b && attrs_a == attrs_b
    }
}

impl Eq for Style { }

impl core::hash::Hash for Style {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        let Style { foreground, background, attributes, quirks: _, condition: _, } = self;
        foreground.hash(state);
        background.hash(state);
        attributes.hash(state);
    }
}

impl PartialOrd for Style {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        let Style {
            foreground: fg_a,
            background: bg_a,
            attributes: attrs_a,
            quirks: _,
            condition: _,
        } = self;

        let Style {
            foreground: fg_b,
            background: bg_b,
            attributes: attrs_b,
            quirks: _,
            condition: _,
        } = other;

        match fg_a.partial_cmp(&fg_b) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }

        match bg_a.partial_cmp(&bg_b) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }

        attrs_a.partial_cmp(&attrs_b)
    }
}

impl Ord for Style {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let Style {
            foreground: fg_a,
            background: bg_a,
            attributes: attrs_a,
            quirks: _,
            condition: _,
        } = self;

        let Style {
            foreground: fg_b,
            background: bg_b,
            attributes: attrs_b,
            quirks: _,
            condition: _,
        } = other;

        match fg_a.cmp(&fg_b) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        match bg_a.cmp(&bg_b) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        attrs_a.cmp(&attrs_b)
    }
}

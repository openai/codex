use core::fmt;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{string::{String, ToString}, borrow::Cow};

#[cfg(feature = "std")]
use std::borrow::Cow;

use crate::{Color, Attribute, Quirk, Style, Condition};

/// An arbitrary value with a [`Style`] applied to it.
///
/// A `Painted` can be directly formatted. This results in the internal
/// [`value`](Self::value) being formatted as specified and ANSI code styling
/// sequences corresponding to [`style`](Self::style) being prefixed and
/// suffixed as necessary. Both the global and local [`Condition`] affects
/// whether styling sequences are actually emitted: both must evaluated to true.
/// Otherwise, no styling sequences are emitted.
///
/// ```rust
/// use yansi::{Paint, Condition};
///
/// println!("Hello, {}!", "world".red().underline().blink());
/// // > Hello, world! # world is red, underlined, and blinking
///
/// let v = format!("{}", "world".red().underline().blink());
/// assert_eq!(v, "\u{1b}[4;5;31mworld\u{1b}[0m");
/// println!("{}", v); // > world # world is red, underlined, and blinking
///
/// let v = format!("{}", "world".red().underline().blink().whenever(Condition::NEVER));
/// assert_eq!(v, "world");
/// ```
#[derive(Copy, Clone)]
pub struct Painted<T> {
    /// The value to be styled.
    pub value: T,
    /// The style to apply.
    pub style: Style,
}

/// A trait to apply styling to any value. Implemented for all types.
///
/// Because this trait is implemented for all types, you can use its methods on
/// any type. With the exception of one constructor method, [`Paint::new()`],
/// all methods are called with method syntax:
///
/// ```rust
/// use yansi::Paint;
///
/// "hello".green(); // calls `Paint::<&'static str>::green()`.
/// "hello".strike(); // calls `Paint::<&'static str>::strike()`.
/// 1.on_red(); // calls `Paint::<i32>::red()`.
/// 1.blink(); // calls `Paint::<i32>::blink()`.
/// ```
///
/// ### Chaining
///
/// All methods return a [`Painted`] whose methods are exactly those of `Paint`.
/// This means you can chain `Paint` method calls:
///
/// ```rust
/// use yansi::Paint;
///
/// "hello".green().strike(); // calls `Paint::green()` then `Painted::strike()`.
/// 1.on_red().blink(); // calls `Paint::red()` + `Painted::blink()`.
/// ```
///
/// ### Borrow vs. Owned Receiver
///
/// The returned [`Painted`] type contains a borrow to the receiver:
///
/// ```rust
/// use yansi::{Paint, Painted};
///
/// let v: Painted<&i32> = 1.red();
/// ```
///
/// This is nearly always what you want. In the exceedingly rare case that you
/// _do_ want `Painted` to own its value, use [`Paint::new()`] or the equivalent
/// [`Painted::new()`]:
///
/// ```rust
/// use yansi::{Paint, Painted};
///
/// let v: Painted<i32> = Paint::new(1);
/// let v: Painted<i32> = Painted::new(1);
/// ```
///
/// ### Further Details
///
/// See the [crate level docs](crate#usage) for more details and examples.
pub trait Paint {
    /// Create a new [`Painted`] with a default [`Style`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::Paint;
    ///
    /// let painted = Paint::new("hello");
    /// assert_eq!(painted.style, yansi::Style::new());
    /// ```
    #[inline(always)]
    fn new(self) -> Painted<Self> where Self: Sized {
        Painted::new(self)
    }

    #[doc(hidden)]
    #[inline(always)]
    fn apply(&self, a: crate::style::Application) -> Painted<&Self> {
        Painted::new(self).apply(a)
    }

    /// Apply a style wholesale to `self`. Any previous style is replaced.
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::{Paint, Style, Color::*};
    ///
    /// static DEBUG: Style = Black.bold().on_yellow();
    ///
    /// let painted = "hello".paint(DEBUG);
    /// ```
    #[inline(always)]
    fn paint<S: Into<Style>>(&self, style: S) -> Painted<&Self> {
        Painted { value: self, style: style.into() }
    }

    properties!(signature(&Self) -> Painted<&Self>);
}

#[allow(rustdoc::broken_intra_doc_links)]
impl<T: ?Sized> Paint for T {
    properties!(constructor(&Self) -> Painted<&Self>);
}

impl<T> Painted<T> {
    /// Create a new [`Painted`] with a default [`Style`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::Painted;
    ///
    /// let painted = Painted::new("hello");
    /// assert_eq!(painted.style, yansi::Style::new());
    /// ```
    #[inline(always)]
    pub const fn new(value: T) -> Painted<T> {
        Painted { value, style: Style::new() }
    }

    #[inline(always)]
    const fn apply(mut self, a: crate::style::Application) -> Self {
        self.style = self.style.apply(a);
        self
    }

    #[inline]
    pub(crate) fn enabled(&self) -> bool {
        crate::is_enabled() && self.style.condition.map_or(true, |c| c())
    }

    properties!([pub const] constructor(Self) -> Self);
}

impl<T> Painted<T> {
    pub(crate) fn color_fmt_value(
        &self,
        fmt: &dyn Fn(&T, &mut fmt::Formatter) -> fmt::Result,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        self.style.fmt_prefix(f)?;
        fmt(&self.value, f)?;
        self.style.fmt_suffix(f)
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn reset_fmt_args(
        &self,
        fmt: &dyn Fn(&T, &mut fmt::Formatter) -> fmt::Result,
        f: &mut fmt::Formatter,
        args: &fmt::Arguments<'_>,
    ) -> fmt::Result {
        // A tiny state machine to find escape sequences.
        enum State { Searching, Open, }
        let mut state = State::Searching;
        let escape = |c: char| {
            match state {
                State::Searching if c == '\x1B' => { state = State::Open; true }
                State::Open if c == 'm' => { state = State::Searching; true }
                State::Searching => false,
                State::Open => true,
            }
        };

        // Only replace when the string contains styling.
        let string = args.as_str()
            .map(|string| Cow::Borrowed(string))
            .unwrap_or_else(|| Cow::Owned(args.to_string()));

        if string.contains('\x1B') {
            f.write_str(&string.replace(escape, ""))
        } else {
            fmt(&self.value, f)
        }
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn color_wrap_fmt_args(
        &self,
        fmt: &dyn Fn(&T, &mut fmt::Formatter) -> fmt::Result,
        f: &mut fmt::Formatter,
        args: &fmt::Arguments<'_>,
    ) -> fmt::Result {
        // Only replace when the string contains styling.
        let string = args.as_str()
            .map(|string| Cow::Borrowed(string))
            .unwrap_or_else(|| Cow::Owned(args.to_string()));

        if !string.contains('\x1B') {
            return self.color_fmt_value(fmt, f);
        }

        // Compute the prefix for the style with a reset in front.
        let mut prefix = String::new();
        prefix.push_str("\x1B[0m");
        self.style.fmt_prefix(&mut prefix)?;

        // Write out formatted string, replacing resets with computed prefix.
        self.style.fmt_prefix(f)?;
        write!(f, "{}", string.replace("\x1B[0m", &prefix))?;
        self.style.fmt_suffix(f)
    }

    pub(crate) fn fmt_args(
        &self,
        fmt: &dyn Fn(&T, &mut fmt::Formatter) -> fmt::Result,
        f: &mut fmt::Formatter,
        _args: fmt::Arguments<'_>,
    ) -> fmt::Result {
        let enabled = self.enabled();
        let masked = self.style.quirks.contains(Quirk::Mask);

        #[cfg(not(feature = "alloc"))]
        match (enabled, masked) {
            (true, _) => self.color_fmt_value(fmt, f),
            (false, false) => fmt(&self.value, f),
            (false, true) => Ok(()),
        }

        #[cfg(feature = "alloc")]
        match (enabled, masked, self.style.quirks.contains(Quirk::Wrap)) {
            (true, _, true) => self.color_wrap_fmt_args(fmt, f, &_args),
            (true, _, false) => self.color_fmt_value(fmt, f),
            (false, false, true) => self.reset_fmt_args(fmt, f, &_args),
            (false, false, false) => fmt(&self.value, f),
            (false, true, _) => Ok(()),
        }
    }
}

impl_fmt_traits!(<T> Painted<T> => self.value (T));

impl<T> From<Painted<T>> for Style {
    fn from(painted: Painted<T>) -> Self {
        painted.style
    }
}

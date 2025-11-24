//! Experimental support for hyperlinking.
//!
//! # Usage
//!
//! Enable the `hyperlink` crate feature. Enabling `hyperlink` implicitly
//! enables `std`.
//!
//! Import the `HyperlinkExt` extension trait and use the [`link()`] builder
//! method.
//!
//! [`link()`]: HyperlinkExt::link()
//!
//! ```rust
//! use yansi::Paint;
//! use yansi::hyperlink::HyperlinkExt;
//!
//! println!("Go to {}.", "our docs".link("https://docs.rs/yansi").green());
//! ```
//!
//! `>` Go to <a href="https://docs.rs/yansi"><span style="color: green;">our docs</span></a>.
//!
//! The `link()` method returns a [`PaintedLink`] structure which implements all
//! of the unverisal chainable methods available across the library.
//! Furthermore, [`Painted`] is extended with a [`link()`](Painted::link())
//! method. The net effect is that you can use `link()` as if it were any other
//! styling method:
//!
//! ```rust
//! use yansi::Paint;
//! use yansi::hyperlink::HyperlinkExt;
//!
//! println!("Go to {}.", "our docs".green().link("https://docs.rs/yansi").on_black().invert());
//! ```
//!
//! `>` Go to <a href="https://docs.rs/yansi">
//! <span style="background: green; color: black;">our docs</span>
//! </a>.
//!
//! # Caveats
//!
//! 1. You can only create a link when there is a target value to print, that
//!    is, when the receiver is something "printable". In other words, you
//!    _cannot_ apply `link()` to a bare `Style`. This means the following will
//!    not work:
//!
//!    ```rust,compile_fail
//!    use yansi::{Paint, Style, Color::*};
//!    use yansi::hyperlink::HyperlinkExt;
//!
//!    static LINKED: Style = Green.link("https://docs.rs/yansi");
//!    ```
//!    <br/>
//!
//! 2. While some modern terminals support hyperlinking, many do not. Those that
//!    do not _should_ gracefully ignore the target URL and print the original
//!    value. That is, instead of `>` <a href="https://docs.rs/yansi">our
//!    docs</a>, such terminals would print `>` our docs.
use core::fmt;

use crate::*;

/// A [`Painted`] with an associated target URL to hyperlink.
pub struct PaintedLink<T> {
    painted: Painted<T>,
    link: String,
}

/// Extension trait to apply hyperlinks to any value, implemented for all types.
///
/// See the [module level docs](hyperlink) for usage details.
pub trait HyperlinkExt {
    /// Create a painted hyperlink with a target URL of `url`.
    ///
    /// See [`hyperlink`] for details.
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::hyperlink::HyperlinkExt;
    ///
    /// println!("See {}.", "our docs".link("https://docs.rs/yansi"));
    /// ```
    fn link(&self, url: impl ToString) -> PaintedLink<&Self>;
}

impl<T> PaintedLink<T> {
    fn fmt_args(
        &self,
        fmt: &dyn Fn(&Painted<T>, &mut fmt::Formatter) -> fmt::Result,
        f: &mut fmt::Formatter,
        _args: fmt::Arguments<'_>,
    ) -> fmt::Result {
        if !self.painted.enabled() {
            return fmt(&self.painted, f);
        }

        write!(f, "\x1B]8;;{}\x1B\\", self.link)?;
        fmt(&self.painted, f)?;
        write!(f, "\x1B]8;;\x1B\\")
    }
}

impl_fmt_traits!(<T> PaintedLink<T> => self.painted (Painted<T>));

impl<T> HyperlinkExt for T {
    fn link(&self, url: impl ToString) -> PaintedLink<&Self> {
        PaintedLink { painted: Painted::new(self), link: url.to_string() }
    }
}

/// Experimental support for hyperlinking.
impl<T> Painted<T> {
    /// Create a painted hyperlink with a target URL of `url`.
    ///
    /// See [`hyperlink`] for details.
    ///
    /// # Example
    ///
    /// ```rust
    /// use yansi::Paint;
    /// use yansi::hyperlink::HyperlinkExt;
    ///
    /// println!("See {}.", "our docs".green().link("https://docs.rs/yansi"));
    /// ```
    pub fn link(&self, url: impl ToString) -> PaintedLink<&Self> {
        PaintedLink { painted: Painted::new(self), link: url.to_string() }
    }
}

impl<T> PaintedLink<T> {
    #[inline(always)]
    const fn apply(mut self, a: crate::style::Application) -> Self {
        self.painted.style = self.painted.style.apply(a);
        self
    }

    properties!([pub const] constructor(Self) -> Self);
}

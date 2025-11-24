//! The `Style` type is a simplified view of the various
//! attributes offered by the `term` library. These are
//! enumerated as bits so they can be easily or'd together
//! etc.

use std::default::Default;
use term::{self, Terminal};

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct Style {
    bits: u64,
}

macro_rules! declare_styles {
    ($($style:ident,)*) => {
        #[derive(Copy, Clone)]
        #[allow(non_camel_case_types)]
        enum StyleBit {
            $($style,)*
        }

        $(
            pub const $style: Style = Style { bits: 1 << (StyleBit::$style as u64) };
        )*
    }
}

pub const DEFAULT: Style = Style { bits: 0 };

declare_styles! {
    // Foreground colors:
    FG_BLACK,
    FG_BLUE,
    FG_BRIGHT_BLACK,
    FG_BRIGHT_BLUE,
    FG_BRIGHT_CYAN,
    FG_BRIGHT_GREEN,
    FG_BRIGHT_MAGENTA,
    FG_BRIGHT_RED,
    FG_BRIGHT_WHITE,
    FG_BRIGHT_YELLOW,
    FG_CYAN,
    FG_GREEN,
    FG_MAGENTA,
    FG_RED,
    FG_WHITE,
    FG_YELLOW,

    // Background colors:
    BG_BLACK,
    BG_BLUE,
    BG_BRIGHT_BLACK,
    BG_BRIGHT_BLUE,
    BG_BRIGHT_CYAN,
    BG_BRIGHT_GREEN,
    BG_BRIGHT_MAGENTA,
    BG_BRIGHT_RED,
    BG_BRIGHT_WHITE,
    BG_BRIGHT_YELLOW,
    BG_CYAN,
    BG_GREEN,
    BG_MAGENTA,
    BG_RED,
    BG_WHITE,
    BG_YELLOW,

    // Other:
    BOLD,
    DIM,
    ITALIC,
    UNDERLINE,
    BLINK,
    STANDOUT,
    REVERSE,
    SECURE,
}

impl Style {
    pub fn new() -> Style {
        Style::default()
    }

    pub fn with(self, other_style: Style) -> Style {
        Style {
            bits: self.bits | other_style.bits,
        }
    }

    pub fn contains(self, other_style: Style) -> bool {
        self.with(other_style) == self
    }

    /// Attempts to apply the given style to the given terminal. If
    /// the style is not supported, either there is no effect or else
    /// a similar, substitute style may be applied.
    pub fn apply<T: Terminal + ?Sized>(self, term: &mut T) -> term::Result<()> {
        term.reset()?;

        macro_rules! fg_color {
            ($color:expr, $term_color:ident) => {
                if self.contains($color) {
                    if term.supports_color() {
                        term.fg(term::color::$term_color)?;
                    }
                }
            };
        }

        fg_color!(FG_BLACK, BLACK);
        fg_color!(FG_BLUE, BLUE);
        fg_color!(FG_BRIGHT_BLACK, BRIGHT_BLACK);
        fg_color!(FG_BRIGHT_BLUE, BRIGHT_BLUE);
        fg_color!(FG_BRIGHT_CYAN, BRIGHT_CYAN);
        fg_color!(FG_BRIGHT_GREEN, BRIGHT_GREEN);
        fg_color!(FG_BRIGHT_MAGENTA, BRIGHT_MAGENTA);
        fg_color!(FG_BRIGHT_RED, BRIGHT_RED);
        fg_color!(FG_BRIGHT_WHITE, BRIGHT_WHITE);
        fg_color!(FG_BRIGHT_YELLOW, BRIGHT_YELLOW);
        fg_color!(FG_CYAN, CYAN);
        fg_color!(FG_GREEN, GREEN);
        fg_color!(FG_MAGENTA, MAGENTA);
        fg_color!(FG_RED, RED);
        fg_color!(FG_WHITE, WHITE);
        fg_color!(FG_YELLOW, YELLOW);

        macro_rules! bg_color {
            ($color:expr, $term_color:ident) => {
                if self.contains($color) {
                    if term.supports_color() {
                        term.bg(term::color::$term_color)?;
                    }
                }
            };
        }

        bg_color!(BG_BLACK, BLACK);
        bg_color!(BG_BLUE, BLUE);
        bg_color!(BG_BRIGHT_BLACK, BRIGHT_BLACK);
        bg_color!(BG_BRIGHT_BLUE, BRIGHT_BLUE);
        bg_color!(BG_BRIGHT_CYAN, BRIGHT_CYAN);
        bg_color!(BG_BRIGHT_GREEN, BRIGHT_GREEN);
        bg_color!(BG_BRIGHT_MAGENTA, BRIGHT_MAGENTA);
        bg_color!(BG_BRIGHT_RED, BRIGHT_RED);
        bg_color!(BG_BRIGHT_WHITE, BRIGHT_WHITE);
        bg_color!(BG_BRIGHT_YELLOW, BRIGHT_YELLOW);
        bg_color!(BG_CYAN, CYAN);
        bg_color!(BG_GREEN, GREEN);
        bg_color!(BG_MAGENTA, MAGENTA);
        bg_color!(BG_RED, RED);
        bg_color!(BG_WHITE, WHITE);
        bg_color!(BG_YELLOW, YELLOW);

        macro_rules! attr {
            ($attr:expr, $term_attr:expr) => {
                if self.contains($attr) {
                    let attr = $term_attr;
                    if term.supports_attr(attr) {
                        term.attr(attr)?;
                    }
                }
            };
        }

        attr!(BOLD, term::Attr::Bold);
        attr!(DIM, term::Attr::Dim);
        attr!(ITALIC, term::Attr::Italic(true));
        attr!(UNDERLINE, term::Attr::Underline(true));
        attr!(BLINK, term::Attr::Blink);
        attr!(STANDOUT, term::Attr::Standout(true));
        attr!(REVERSE, term::Attr::Reverse);
        attr!(SECURE, term::Attr::Secure);

        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct StyleCursor<'term, T: ?Sized + Terminal> {
    current_style: Style,
    term: &'term mut T,
}

impl<'term, T: ?Sized + Terminal> StyleCursor<'term, T> {
    pub fn new(term: &'term mut T) -> term::Result<StyleCursor<'term, T>> {
        let current_style = Style::default();
        current_style.apply(term)?;
        Ok(StyleCursor {
            current_style: current_style,
            term: term,
        })
    }

    pub fn term(&mut self) -> &mut T {
        self.term
    }

    pub fn set_style(&mut self, style: Style) -> term::Result<()> {
        if style != self.current_style {
            style.apply(self.term)?;
            self.current_style = style;
        }
        Ok(())
    }
}

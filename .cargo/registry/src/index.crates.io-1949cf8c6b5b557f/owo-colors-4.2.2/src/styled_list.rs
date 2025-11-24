use crate::{Style, Styled, dyn_styles::StyleFlags};
use core::{
    fmt::{self, Display},
    marker::PhantomData,
};

#[cfg(feature = "alloc")]
extern crate alloc;

// Hidden trait for use in `StyledList` bounds
mod sealed {
    pub trait IsStyled {
        type Inner: core::fmt::Display;

        fn style(&self) -> &crate::Style;
        fn inner(&self) -> &Self::Inner;
    }
}

use sealed::IsStyled;

impl<T: IsStyled> IsStyled for &T {
    type Inner = T::Inner;

    fn style(&self) -> &Style {
        <T as IsStyled>::style(*self)
    }

    fn inner(&self) -> &Self::Inner {
        <T as IsStyled>::inner(*self)
    }
}

impl<T: Display> IsStyled for Styled<T> {
    type Inner = T;

    fn style(&self) -> &Style {
        &self.style
    }

    fn inner(&self) -> &T {
        &self.target
    }
}

/// A collection of [`Styled`] items that are displayed in such a way as to minimize the amount of characters
/// that are written when displayed.
///
/// ```rust
/// use owo_colors::{Style, Styled, StyledList};
///
/// let styled_items = [
///     Style::new().red().style("Hello "),
///     Style::new().green().style("World"),
///  ];
///
/// // 29 characters
/// let normal_length = styled_items.iter().map(|item| format!("{}", item).len()).sum::<usize>();
/// // 25 characters
/// let styled_length = format!("{}", StyledList::from(styled_items)).len();
///
/// assert!(styled_length < normal_length);
/// ```
pub struct StyledList<T, U>(pub T, PhantomData<fn(U)>)
where
    T: AsRef<[U]>,
    U: IsStyled;

impl<T, U> From<T> for StyledList<T, U>
where
    T: AsRef<[U]>,
    U: IsStyled,
{
    fn from(list: T) -> Self {
        Self(list, PhantomData)
    }
}

impl<T, U> Display for StyledList<T, U>
where
    T: AsRef<[U]>,
    U: IsStyled,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Handle first item manually
        let first_item = match self.0.as_ref().first() {
            Some(s) => s,
            None => return Ok(()),
        };

        first_item.style().fmt_prefix(f)?;
        write!(f, "{}", first_item.inner())?;

        // Handle the rest
        for window in self.0.as_ref().windows(2) {
            let prev = &window[0];
            let current = &window[1];

            write!(
                f,
                "{}{}",
                current.style().transition_from(prev.style()),
                current.inner()
            )?;
        }

        // Print final reset
        // SAFETY: We know that the first item exists, thus a last item exists
        self.0.as_ref().last().unwrap().style().fmt_suffix(f)
    }
}

impl<'a> Style {
    /// Returns an enum that indicates how the transition from one style to this style should be printed
    fn transition_from(&'a self, from: &Style) -> Transition<'a> {
        if self == from {
            return Transition::Noop;
        }

        // Use full reset if transitioning from colored to non-colored
        // or if previous style contains properties that are not in this style
        if (from.fg.is_some() && self.fg.is_none())
            || (from.bg.is_some() && self.bg.is_none())
            || (from.bold && !self.bold)
            || (!self.style_flags.0 & from.style_flags.0) != 0
        {
            return Transition::FullReset(self);
        }

        // Build up a transition style, that does not require a full reset
        // Contains all properties from `self` that are not in `from`
        let fg = match (self.fg, from.fg) {
            (Some(fg), Some(from_fg)) if fg != from_fg => Some(fg),
            (Some(fg), None) => Some(fg),
            _ => None,
        };

        let bg = match (self.bg, from.bg) {
            (Some(bg), Some(from_bg)) if bg != from_bg => Some(bg),
            (Some(bg), None) => Some(bg),
            _ => None,
        };

        let new_style = Style {
            fg,
            bg,
            bold: from.bold ^ self.bold,
            style_flags: StyleFlags(self.style_flags.0 ^ from.style_flags.0),
        };

        Transition::Style(new_style)
    }
}

/// How the transition between two styles should be printed
#[cfg_attr(test, derive(Debug, PartialEq))]
enum Transition<'a> {
    Noop,
    FullReset(&'a Style),
    Style(Style),
}

impl fmt::Display for Transition<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // Styles are equal
            Transition::Noop => Ok(()),
            // Reset the style & print full prefix
            Transition::FullReset(style) => {
                write!(f, "\x1B[0m")?;
                style.fmt_prefix(f)
            }
            // Print transition style without resetting the style
            Transition::Style(style) => style.fmt_prefix(f),
        }
    }
}

/// A helper alias for [`StyledList`] for easier usage with [`alloc::vec::Vec`].
#[cfg(feature = "alloc")]
pub type StyledVec<T> = StyledList<alloc::vec::Vec<Styled<T>>, Styled<T>>;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_styled_list() {
        let list = &[
            Style::new().red().style("red"),
            Style::new().green().italic().style("green italic"),
            Style::new().red().bold().style("red bold"),
        ];

        let list = StyledList::from(list);

        assert_eq!(
            format!("{}", list),
            "\x1b[31mred\x1b[32;3mgreen italic\x1b[0m\x1b[31;1mred bold\x1b[0m"
        );
    }

    #[test]
    fn test_styled_final_plain() {
        let list = &[
            Style::new().red().style("red"),
            Style::new().green().italic().style("green italic"),
            Style::new().style("plain"),
        ];

        let list = StyledList::from(list);

        assert_eq!(
            format!("{}", list),
            "\x1b[31mred\x1b[32;3mgreen italic\x1b[0mplain"
        );
    }

    #[test]
    fn test_transition_from_noop() {
        let style_current = Style::new().italic().red();
        let style_prev = Style::new().italic().red();

        assert_eq!(style_current.transition_from(&style_prev), Transition::Noop);
    }

    #[test]
    fn test_transition_from_full_reset() {
        let style_current = Style::new().italic().red();
        let style_prev = Style::new().italic().dimmed().red();

        assert_eq!(
            style_current.transition_from(&style_prev),
            Transition::FullReset(&style_current)
        );

        let style_current = Style::new();
        let style_prev = Style::new().red();
        assert_eq!(
            style_current.transition_from(&style_prev),
            Transition::FullReset(&style_current)
        );

        let style_current = Style::new();
        let style_prev = Style::new().bold();
        assert_eq!(
            style_current.transition_from(&style_prev),
            Transition::FullReset(&style_current)
        );
    }

    #[test]
    fn test_transition_from_style() {
        let style_current = Style::new().italic().dimmed().red();
        let style_prev = Style::new().italic().red();

        assert_eq!(
            style_current.transition_from(&style_prev),
            Transition::Style(Style::new().dimmed())
        );

        let style_current = Style::new().red().on_green();
        let style_prev = Style::new().red().on_bright_cyan();
        assert_eq!(
            style_current.transition_from(&style_prev),
            Transition::Style(Style::new().on_green())
        );

        let style_current = Style::new().bold().blue();
        let style_prev = Style::new().bold();
        assert_eq!(
            style_current.transition_from(&style_prev),
            Transition::Style(Style::new().blue())
        );
    }
}

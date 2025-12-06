/// Defines the padding for a [`Block`].
///
/// See the [`padding`] method of [`Block`] to configure its padding.
///
/// This concept is similar to [CSS padding].
///
/// **NOTE**: Terminal cells are often taller than they are wide, so to make horizontal and vertical
/// padding seem equal, doubling the horizontal padding is usually pretty good.
///
/// # Example
///
/// ```
/// use ratatui::widgets::Padding;
///
/// Padding::uniform(1);
/// Padding::horizontal(2);
/// Padding::left(3);
/// Padding::proportional(4);
/// Padding::symmetric(5, 6);
/// ```
///
/// [`Block`]: crate::block::Block
/// [`padding`]: crate::block::Block::padding
/// [CSS padding]: https://developer.mozilla.org/en-US/docs/Web/CSS/padding
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Padding {
    /// Left padding
    pub left: u16,
    /// Right padding
    pub right: u16,
    /// Top padding
    pub top: u16,
    /// Bottom padding
    pub bottom: u16,
}

impl Padding {
    /// `Padding` with all fields set to `0`
    pub const ZERO: Self = Self {
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
    };

    /// Creates a new `Padding` by specifying every field individually.
    ///
    /// Note: the order of the fields does not match the order of the CSS properties.
    pub const fn new(left: u16, right: u16, top: u16, bottom: u16) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    /// Creates a `Padding` with all fields set to `0`.
    #[deprecated = "use `Padding::ZERO` instead"]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Creates a `Padding` with the same value for `left` and `right`.
    pub const fn horizontal(value: u16) -> Self {
        Self {
            left: value,
            right: value,
            top: 0,
            bottom: 0,
        }
    }

    /// Creates a `Padding` with the same value for `top` and `bottom`.
    pub const fn vertical(value: u16) -> Self {
        Self {
            left: 0,
            right: 0,
            top: value,
            bottom: value,
        }
    }

    /// Creates a `Padding` with the same value for all fields.
    pub const fn uniform(value: u16) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    /// Creates a `Padding` that is visually proportional to the terminal.
    ///
    /// This represents a padding of 2x the value for `left` and `right` and 1x the value for
    /// `top` and `bottom`.
    pub const fn proportional(value: u16) -> Self {
        Self {
            left: 2 * value,
            right: 2 * value,
            top: value,
            bottom: value,
        }
    }

    /// Creates a `Padding` that is symmetric.
    ///
    /// The `x` value is used for `left` and `right` and the `y` value is used for `top` and
    /// `bottom`.
    pub const fn symmetric(x: u16, y: u16) -> Self {
        Self {
            left: x,
            right: x,
            top: y,
            bottom: y,
        }
    }

    /// Creates a `Padding` that only sets the `left` padding.
    pub const fn left(value: u16) -> Self {
        Self {
            left: value,
            right: 0,
            top: 0,
            bottom: 0,
        }
    }

    /// Creates a `Padding` that only sets the `right` padding.
    pub const fn right(value: u16) -> Self {
        Self {
            left: 0,
            right: value,
            top: 0,
            bottom: 0,
        }
    }

    /// Creates a `Padding` that only sets the `top` padding.
    pub const fn top(value: u16) -> Self {
        Self {
            left: 0,
            right: 0,
            top: value,
            bottom: 0,
        }
    }

    /// Creates a `Padding` that only sets the `bottom` padding.
    pub const fn bottom(value: u16) -> Self {
        Self {
            left: 0,
            right: 0,
            top: 0,
            bottom: value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        assert_eq!(
            Padding::new(1, 2, 3, 4),
            Padding {
                left: 1,
                right: 2,
                top: 3,
                bottom: 4
            }
        );
    }

    #[test]
    fn constructors() {
        assert_eq!(Padding::horizontal(1), Padding::new(1, 1, 0, 0));
        assert_eq!(Padding::vertical(1), Padding::new(0, 0, 1, 1));
        assert_eq!(Padding::uniform(1), Padding::new(1, 1, 1, 1));
        assert_eq!(Padding::proportional(1), Padding::new(2, 2, 1, 1));
        assert_eq!(Padding::symmetric(1, 2), Padding::new(1, 1, 2, 2));
        assert_eq!(Padding::left(1), Padding::new(1, 0, 0, 0));
        assert_eq!(Padding::right(1), Padding::new(0, 1, 0, 0));
        assert_eq!(Padding::top(1), Padding::new(0, 0, 1, 0));
        assert_eq!(Padding::bottom(1), Padding::new(0, 0, 0, 1));
    }

    #[test]
    const fn can_be_const() {
        const _PADDING: Padding = Padding::new(1, 1, 1, 1);
        const _UNI_PADDING: Padding = Padding::uniform(1);
        const _HORIZONTAL: Padding = Padding::horizontal(1);
        const _VERTICAL: Padding = Padding::vertical(1);
        const _PROPORTIONAL: Padding = Padding::proportional(1);
        const _SYMMETRIC: Padding = Padding::symmetric(1, 1);
        const _LEFT: Padding = Padding::left(1);
        const _RIGHT: Padding = Padding::right(1);
        const _TOP: Padding = Padding::top(1);
        const _BOTTOM: Padding = Padding::bottom(1);
    }
}

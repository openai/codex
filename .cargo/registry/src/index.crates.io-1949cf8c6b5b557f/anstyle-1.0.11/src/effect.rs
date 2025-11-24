/// A set of text effects
///
/// # Examples
///
/// ```rust
/// let effects = anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE;
/// ```
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Effects(u16);

impl Effects {
    /// No [`Effects`] applied
    const PLAIN: Self = Effects(0);

    #[allow(missing_docs)]
    pub const BOLD: Self = Effects(1 << 0);
    #[allow(missing_docs)]
    pub const DIMMED: Self = Effects(1 << 1);
    /// Not widely supported. Sometimes treated as inverse or blink
    pub const ITALIC: Self = Effects(1 << 2);
    /// Style extensions exist for Kitty, VTE, mintty and iTerm2.
    pub const UNDERLINE: Self = Effects(1 << 3);
    #[allow(missing_docs)]
    pub const DOUBLE_UNDERLINE: Self = Effects(1 << 4);
    #[allow(missing_docs)]
    pub const CURLY_UNDERLINE: Self = Effects(1 << 5);
    #[allow(missing_docs)]
    pub const DOTTED_UNDERLINE: Self = Effects(1 << 6);
    #[allow(missing_docs)]
    pub const DASHED_UNDERLINE: Self = Effects(1 << 7);
    #[allow(missing_docs)]
    pub const BLINK: Self = Effects(1 << 8);
    /// Swap foreground and background colors; inconsistent emulation
    pub const INVERT: Self = Effects(1 << 9);
    #[allow(missing_docs)]
    pub const HIDDEN: Self = Effects(1 << 10);
    ///  Characters legible but marked as if for deletion. Not supported in Terminal.app
    pub const STRIKETHROUGH: Self = Effects(1 << 11);

    /// No effects enabled
    ///
    /// # Examples
    ///
    /// ```rust
    /// let effects = anstyle::Effects::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self::PLAIN
    }

    /// Check if no effects are enabled
    ///
    /// # Examples
    ///
    /// ```rust
    /// let effects = anstyle::Effects::new();
    /// assert!(effects.is_plain());
    ///
    /// let effects = anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE;
    /// assert!(!effects.is_plain());
    /// ```
    #[inline]
    pub const fn is_plain(self) -> bool {
        self.0 == Self::PLAIN.0
    }

    /// Returns `true` if all of the effects in `other` are contained within `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let effects = anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE;
    /// assert!(effects.contains(anstyle::Effects::BOLD));
    ///
    /// let effects = anstyle::Effects::new();
    /// assert!(!effects.contains(anstyle::Effects::BOLD));
    /// ```
    #[inline(always)]
    pub const fn contains(self, other: Effects) -> bool {
        (other.0 & self.0) == other.0
    }

    /// Inserts the specified effects in-place.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let effects = anstyle::Effects::new().insert(anstyle::Effects::new());
    /// assert!(effects.is_plain());
    ///
    /// let effects = anstyle::Effects::new().insert(anstyle::Effects::BOLD);
    /// assert!(effects.contains(anstyle::Effects::BOLD));
    /// ```
    #[inline(always)]
    #[must_use]
    pub const fn insert(mut self, other: Effects) -> Self {
        self.0 |= other.0;
        self
    }

    /// Removes the specified effects in-place.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let effects = (anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE).remove(anstyle::Effects::BOLD);
    /// assert!(!effects.contains(anstyle::Effects::BOLD));
    /// assert!(effects.contains(anstyle::Effects::UNDERLINE));
    /// ```
    #[inline(always)]
    #[must_use]
    pub const fn remove(mut self, other: Effects) -> Self {
        self.0 &= !other.0;
        self
    }

    /// Reset all effects in-place
    /// ```rust
    /// let effects = (anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE).clear();
    /// assert!(!effects.contains(anstyle::Effects::BOLD));
    /// assert!(!effects.contains(anstyle::Effects::UNDERLINE));
    /// ```
    #[inline(always)]
    #[must_use]
    pub const fn clear(self) -> Self {
        Self::new()
    }

    /// Enable or disable the specified effects depending on the passed value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let effects = anstyle::Effects::new().set(anstyle::Effects::BOLD, true);
    /// assert!(effects.contains(anstyle::Effects::BOLD));
    /// ```
    #[inline]
    #[must_use]
    pub const fn set(self, other: Self, enable: bool) -> Self {
        if enable {
            self.insert(other)
        } else {
            self.remove(other)
        }
    }

    /// Iterate over enabled effects
    #[inline(always)]
    pub fn iter(self) -> EffectIter {
        EffectIter {
            index: 0,
            effects: self,
        }
    }

    /// Iterate over enabled effect indices
    #[inline(always)]
    pub(crate) fn index_iter(self) -> EffectIndexIter {
        EffectIndexIter {
            index: 0,
            effects: self,
        }
    }

    /// Render the ANSI code
    #[inline]
    pub fn render(self) -> impl core::fmt::Display + Copy {
        EffectsDisplay(self)
    }

    #[inline]
    #[cfg(feature = "std")]
    pub(crate) fn write_to(self, write: &mut dyn std::io::Write) -> std::io::Result<()> {
        for index in self.index_iter() {
            write.write_all(METADATA[index].escape.as_bytes())?;
        }
        Ok(())
    }
}

/// # Examples
///
/// ```rust
/// let effects = anstyle::Effects::new();
/// assert_eq!(format!("{:?}", effects), "Effects()");
///
/// let effects = anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE;
/// assert_eq!(format!("{:?}", effects), "Effects(BOLD | UNDERLINE)");
/// ```
impl core::fmt::Debug for Effects {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Effects(")?;
        for (i, index) in self.index_iter().enumerate() {
            if i != 0 {
                write!(f, " | ")?;
            }
            write!(f, "{}", METADATA[index].name)?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

/// # Examples
///
/// ```rust
/// let effects = anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE;
/// assert_eq!(format!("{:?}", effects), "Effects(BOLD | UNDERLINE)");
/// ```
impl core::ops::BitOr for Effects {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self {
        self.insert(rhs)
    }
}

/// # Examples
///
/// ```rust
/// let mut effects = anstyle::Effects::BOLD;
/// effects |= anstyle::Effects::UNDERLINE;
/// assert_eq!(format!("{:?}", effects), "Effects(BOLD | UNDERLINE)");
/// ```
impl core::ops::BitOrAssign for Effects {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        *self = self.insert(other);
    }
}

/// # Examples
///
/// ```rust
/// let effects = (anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE) - anstyle::Effects::BOLD;
/// assert_eq!(format!("{:?}", effects), "Effects(UNDERLINE)");
/// ```
impl core::ops::Sub for Effects {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        self.remove(other)
    }
}

/// # Examples
///
/// ```rust
/// let mut effects = anstyle::Effects::BOLD | anstyle::Effects::UNDERLINE;
/// effects -= anstyle::Effects::BOLD;
/// assert_eq!(format!("{:?}", effects), "Effects(UNDERLINE)");
/// ```
impl core::ops::SubAssign for Effects {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        *self = self.remove(other);
    }
}

pub(crate) struct Metadata {
    pub(crate) name: &'static str,
    pub(crate) escape: &'static str,
}

pub(crate) const METADATA: [Metadata; 12] = [
    Metadata {
        name: "BOLD",
        escape: escape!("1"),
    },
    Metadata {
        name: "DIMMED",
        escape: escape!("2"),
    },
    Metadata {
        name: "ITALIC",
        escape: escape!("3"),
    },
    Metadata {
        name: "UNDERLINE",
        escape: escape!("4"),
    },
    Metadata {
        name: "DOUBLE_UNDERLINE",
        escape: escape!("21"),
    },
    Metadata {
        name: "CURLY_UNDERLINE",
        escape: escape!("4:3"),
    },
    Metadata {
        name: "DOTTED_UNDERLINE",
        escape: escape!("4:4"),
    },
    Metadata {
        name: "DASHED_UNDERLINE",
        escape: escape!("4:5"),
    },
    Metadata {
        name: "BLINK",
        escape: escape!("5"),
    },
    Metadata {
        name: "INVERT",
        escape: escape!("7"),
    },
    Metadata {
        name: "HIDDEN",
        escape: escape!("8"),
    },
    Metadata {
        name: "STRIKETHROUGH",
        escape: escape!("9"),
    },
];

#[derive(Copy, Clone, Default, Debug)]
struct EffectsDisplay(Effects);

impl core::fmt::Display for EffectsDisplay {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for index in self.0.index_iter() {
            f.write_str(METADATA[index].escape)?;
        }
        Ok(())
    }
}

/// Enumerate each enabled value in [`Effects`]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectIter {
    index: usize,
    effects: Effects,
}

impl Iterator for EffectIter {
    type Item = Effects;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < METADATA.len() {
            let index = self.index;
            self.index += 1;

            let effect = Effects(1 << index);
            if self.effects.contains(effect) {
                return Some(effect);
            }
        }

        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectIndexIter {
    index: usize,
    effects: Effects,
}

impl Iterator for EffectIndexIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < METADATA.len() {
            let index = self.index;
            self.index += 1;

            let effect = Effects(1 << index);
            if self.effects.contains(effect) {
                return Some(index);
            }
        }

        None
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod test {
    use super::*;

    #[test]
    fn print_size_of() {
        use core::mem::size_of;
        dbg!(size_of::<Effects>());
        dbg!(size_of::<EffectsDisplay>());
    }

    #[test]
    fn no_align() {
        #[track_caller]
        fn assert_no_align(d: impl core::fmt::Display) {
            let expected = format!("{d}");
            let actual = format!("{d:<10}");
            assert_eq!(expected, actual);
        }

        assert_no_align(Effects::BOLD.render());
    }
}

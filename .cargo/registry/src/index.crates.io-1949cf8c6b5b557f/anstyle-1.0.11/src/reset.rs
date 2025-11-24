/// Reset terminal formatting
#[allow(clippy::exhaustive_structs)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reset;

impl Reset {
    /// Render the ANSI code
    ///
    /// `Reset` also implements `Display` directly, so calling this method is optional.
    #[inline]
    pub fn render(self) -> impl core::fmt::Display + Copy {
        self
    }
}

impl core::fmt::Display for Reset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(RESET)
    }
}

pub(crate) const RESET: &str = "\x1B[0m";

#[cfg(test)]
#[cfg(feature = "std")]
mod test {
    use super::*;

    #[test]
    fn print_size_of() {
        use core::mem::size_of;
        dbg!(size_of::<Reset>());
    }

    #[test]
    fn no_align() {
        #[track_caller]
        fn assert_no_align(d: impl core::fmt::Display) {
            let expected = format!("{d}");
            let actual = format!("{d:<10}");
            assert_eq!(expected, actual);
        }

        assert_no_align(Reset);
        assert_no_align(Reset.render());
    }
}

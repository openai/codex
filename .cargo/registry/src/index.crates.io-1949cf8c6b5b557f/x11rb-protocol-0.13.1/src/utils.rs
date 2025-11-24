//! Utility functions that are not specific to X11.
//!
//! # RawFdContainer
//!
//! [`RawFdContainer`] is a variant of [`std::os::unix::io::RawFd`] with ownership semantics. This
//! means that the `RawFd` will be closed when the `RawFdContainer` is dropped.
//!
//! On non-`cfg(unix)`-systems, this is an empty type without methods. It still exists as a type so
//! that it can appear in interfaces, but it is not actually possible to construct an instance of
//! `RawFdContainer`.

#[cfg(all(feature = "std", unix))]
mod raw_fd_container {
    use std::os::unix::io::OwnedFd;

    pub(crate) type RawFdContainer = OwnedFd;
}

#[cfg(not(all(feature = "std", unix)))]
mod raw_fd_container {
    use core::convert::Infallible;

    #[derive(Debug)]
    #[doc(hidden)]
    pub struct RawFdContainer(Infallible);

    impl Drop for RawFdContainer {
        fn drop(&mut self) {
            // This function exists for symmetry with cfg(unix)
            match self.0 {}
        }
    }
}

/// A type representative of the file descriptors as they are sent to and from the X server.
///
/// On `cfg(unix)` platforms, this is a type alias for [`std::os::unix::io::OwnedFd`]. See the
/// documentation for that type for more information on how it should be used. In most cases it
/// can be cast into a [`File`] or [`UnixStream`], or otherwise downgraded into the actual
/// underlying file descriptor.
///
/// On non-Unix platforms, this is an uninhabited type in the same vogue as [`Void`]. As handle
/// passing is an undefined operation on non-Unix implementations of the X11 protocol, instances
/// of this type cannot exist. No operations can be called on this type. If handle passing is ever
/// added to any reference implementation of the X11 protocol, this type will be changed to
/// something that can be used to represent the file descriptors.
///
/// Consumers of this type should be careful to check for `cfg(unix)` before using it in any
/// meaningful way. Otherwise, the program will not compile on non-Unix platforms.
///
/// [`File`]: std::fs::File
/// [`UnixStream`]: std::os::unix::net::UnixStream
/// [`Void`]: https://docs.rs/void/latest/void/enum.Void.html
pub type RawFdContainer = raw_fd_container::RawFdContainer;

mod pretty_printer {
    use core::fmt::{Debug, Formatter, Result};

    /// A helper to pretty-print an enumeration value.
    ///
    /// This function prints the given number. If it matches one of the provided variants, that
    /// match is used. Otherwise, the number is printed as a decimal.
    ///
    /// In alternate mode, the second string in the given array is used, else the first.
    pub(crate) fn pretty_print_enum(
        fmt: &mut Formatter<'_>,
        value: u32,
        cases: &[(u32, &str, &str)],
    ) -> Result {
        for (variant, name1, name2) in cases {
            if &value == variant {
                if fmt.alternate() {
                    return fmt.write_str(name2);
                } else {
                    return fmt.write_str(name1);
                }
            }
        }
        Debug::fmt(&value, fmt)
    }

    /// A helper to pretty-print a bitmask.
    ///
    /// This function prints the given number. All bit-matches with the given variants are printed.
    /// Any left-over number is printed as a decimal.
    ///
    /// In alternate mode, the second string in the given array is used, else the first.
    pub(crate) fn pretty_print_bitmask(
        fmt: &mut Formatter<'_>,
        value: u32,
        cases: &[(u32, &str, &str)],
    ) -> Result {
        // First, figure out if there are any bits not covered by any case
        let known_bits = cases.iter().fold(0, |acc, (value, _, _)| acc | value);
        let remaining = value & !known_bits;
        let mut already_printed = if value == 0 || remaining != 0 {
            Debug::fmt(&remaining, fmt)?;
            true
        } else {
            false
        };
        for (variant, name1, name2) in cases {
            if variant & value != 0 {
                if already_printed {
                    fmt.write_str(" | ")?;
                }
                already_printed = true;
                if fmt.alternate() {
                    fmt.write_str(name2)?;
                } else {
                    fmt.write_str(name1)?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod test {
        use super::{pretty_print_bitmask, pretty_print_enum};
        use alloc::format;
        use core::fmt::{Display, Formatter, Result};

        type CallbackType = fn(&mut Formatter<'_>, u32, &[(u32, &str, &str)]) -> Result;

        struct CallbackFormating<'a, 'b> {
            callback: CallbackType,
            value: u32,
            cases: &'a [(u32, &'b str, &'b str)],
        }

        fn new_enum<'a, 'b>(
            value: u32,
            cases: &'a [(u32, &'b str, &'b str)],
        ) -> CallbackFormating<'a, 'b> {
            CallbackFormating {
                callback: pretty_print_enum,
                value,
                cases,
            }
        }

        fn new_bitmask<'a, 'b>(
            value: u32,
            cases: &'a [(u32, &'b str, &'b str)],
        ) -> CallbackFormating<'a, 'b> {
            CallbackFormating {
                callback: pretty_print_bitmask,
                value,
                cases,
            }
        }

        impl Display for CallbackFormating<'_, '_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                (self.callback)(f, self.value, self.cases)
            }
        }

        #[test]
        fn test_enum() {
            let cases = [(0, "zero", "ZERO"), (42, "the answer", "ANSWER")];
            let printer = new_enum(0, &cases);
            assert_eq!(&format!("{}", printer), "zero");
            assert_eq!(&format!("{:#}", printer), "ZERO");
            let printer = new_enum(1, &cases);
            assert_eq!(&format!("{}", printer), "1");
            assert_eq!(&format!("{:#}", printer), "1");
            let printer = new_enum(42, &cases);
            assert_eq!(&format!("{}", printer), "the answer");
            assert_eq!(&format!("{:#}", printer), "ANSWER");
        }

        #[test]
        fn test_bitmask() {
            let bits = [
                (1 << 5, "b5", "B5"),
                (1 << 1, "b1", "B1"),
                (1 << 0, "unused", "UNUSED"),
            ];
            let printer = new_bitmask(8, &bits);
            assert_eq!(&format!("{}", printer), "8");
            assert_eq!(&format!("{:#}", printer), "8");
            let printer = new_bitmask(32, &bits);
            assert_eq!(&format!("{}", printer), "b5");
            assert_eq!(&format!("{:#}", printer), "B5");
            let printer = new_bitmask(34, &bits);
            assert_eq!(&format!("{}", printer), "b5 | b1");
            assert_eq!(&format!("{:#}", printer), "B5 | B1");
            let printer = new_bitmask(42, &bits);
            assert_eq!(&format!("{}", printer), "8 | b5 | b1");
            assert_eq!(&format!("{:#}", printer), "8 | B5 | B1");
        }
    }
}

pub(crate) use pretty_printer::{pretty_print_bitmask, pretty_print_enum};

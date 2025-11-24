use crate::{BitFlag, BitFlags};
use core::fmt::{self, Binary, Debug};

impl<T> fmt::Debug for BitFlags<T>
where
    T: BitFlag + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = T::BITFLAGS_TYPE_NAME;
        let bits = DebugBinaryFormatter(&self.val);
        let iter = if !self.is_empty() {
            Some(FlagFormatter(self.iter()))
        } else {
            None
        };

        if !fmt.alternate() {
            // Concise tuple formatting is a better default
            let mut debug = fmt.debug_tuple(name);
            debug.field(&bits);
            if let Some(iter) = iter {
                debug.field(&iter);
            }
            debug.finish()
        } else {
            // Pretty-printed tuples are ugly and hard to read, so use struct format
            let mut debug = fmt.debug_struct(name);
            debug.field("bits", &bits);
            if let Some(iter) = iter {
                debug.field("flags", &iter);
            }
            debug.finish()
        }
    }
}

impl<T> fmt::Display for BitFlags<T>
where
    T: BitFlag + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&FlagFormatter(self.iter()), fmt)
    }
}

impl<T> fmt::Binary for BitFlags<T>
where
    T: BitFlag,
    T::Numeric: fmt::Binary,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Binary::fmt(&self.bits(), fmt)
    }
}

impl<T> fmt::Octal for BitFlags<T>
where
    T: BitFlag,
    T::Numeric: fmt::Octal,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Octal::fmt(&self.bits(), fmt)
    }
}

impl<T> fmt::LowerHex for BitFlags<T>
where
    T: BitFlag,
    T::Numeric: fmt::LowerHex,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.bits(), fmt)
    }
}

impl<T> fmt::UpperHex for BitFlags<T>
where
    T: BitFlag,
    T::Numeric: fmt::UpperHex,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.bits(), fmt)
    }
}

// Format an iterator of flags into "A | B | etc"
struct FlagFormatter<I>(I);

impl<T: Debug, I: Clone + Iterator<Item = T>> Debug for FlagFormatter<I> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.clone();
        if let Some(val) = iter.next() {
            Debug::fmt(&val, fmt)?;
            for val in iter {
                fmt.write_str(" | ")?;
                Debug::fmt(&val, fmt)?;
            }
            Ok(())
        } else {
            fmt.write_str("<empty>")
        }
    }
}

// A formatter that obeys format arguments but falls back to binary when
// no explicit format is requested. Supports {:08?}, {:08x?}, etc.
struct DebugBinaryFormatter<'a, F>(&'a F);

impl<'a, F: Debug + Binary + 'a> Debug for DebugBinaryFormatter<'a, F> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Check if {:x?} or {:X?} was used; this is determined via the
        // discriminator of core::fmt::FlagV1::{DebugLowerHex, DebugUpperHex},
        // which is not an accessible type: https://github.com/rust-lang/rust/blob/d65e272a9fe3e61aa5f229c5358e35a909435575/src/libcore/fmt/mod.rs#L306
        // See also: https://github.com/rust-lang/rfcs/pull/2226
        #[allow(deprecated)]
        let format_hex = fmt.flags() >> 4;
        let width = fmt.width().unwrap_or(0);

        if format_hex & 1 != 0 {
            // FlagV1::DebugLowerHex
            write!(fmt, "{:#0width$x?}", &self.0, width = width)
        } else if format_hex & 2 != 0 {
            // FlagV1::DebugUpperHex
            write!(fmt, "{:#0width$X?}", &self.0, width = width)
        } else {
            // Fall back to binary otheriwse
            write!(fmt, "{:#0width$b}", &self.0, width = width)
        }
    }
}

#[test]
fn flag_formatter() {
    use core::iter;

    macro_rules! assert_fmt {
        ($fmt:expr, $expr:expr, $expected:expr) => {
            assert_eq!(format!($fmt, FlagFormatter($expr)), $expected)
        };
    }

    assert_fmt!("{:?}", iter::empty::<u8>(), "<empty>");
    assert_fmt!("{:?}", iter::once(1), "1");
    assert_fmt!("{:?}", [1, 2].iter(), "1 | 2");
    assert_fmt!("{:?}", [1, 2, 10].iter(), "1 | 2 | 10");
    assert_fmt!("{:02x?}", [1, 2, 10].iter(), "01 | 02 | 0a");
    assert_fmt!("{:#04X?}", [1, 2, 10].iter(), "0x01 | 0x02 | 0x0A");
}

#[test]
fn debug_binary_formatter() {
    macro_rules! assert_fmt {
        ($fmt:expr, $expr:expr, $expected:expr) => {
            assert_eq!(format!($fmt, DebugBinaryFormatter(&$expr)), $expected)
        };
    }

    assert_fmt!("{:?}", 10, "0b1010");
    assert_fmt!("{:#?}", 10, "0b1010");
    assert_fmt!("{:010?}", 10, "0b00001010");
    assert_fmt!("{:010x?}", 10, "0x0000000a");
    assert_fmt!("{:#010X?}", 10, "0x0000000A");
}

/// A simple and not the most-efficient fixed size string on the stack.
///
/// This supplanted some uses of `Box<str>` for storing tiny strings in an
/// effort to reduce our dependence on dynamic memory allocation.
///
/// Also, since it isn't needed and it lets us save on storage requirements,
/// `N` must be less than `256` (so that the length can fit in a `u8`).
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[doc(hidden)] // not part of Jiff's public API
pub struct ArrayStr<const N: usize> {
    /// The UTF-8 bytes that make up the string.
    ///
    /// This array---the entire array---is always valid UTF-8. And
    /// the `0..self.len` sub-slice is also always valid UTF-8.
    bytes: [u8; N],
    /// The number of bytes used by the string in `bytes`.
    ///
    /// (We could technically save this byte in some cases and use a NUL
    /// terminator. For example, since we don't permit NUL bytes in POSIX time
    /// zone abbreviation strings, but this is simpler and only one byte and
    /// generalizes. And we're not really trying to micro-optimize the storage
    /// requirements when we use these array strings. Or at least, I don't know
    /// of a reason to.)
    len: u8,
}

impl<const N: usize> ArrayStr<N> {
    /// Creates a new fixed capacity string.
    ///
    /// If the given string exceeds `N` bytes, then this returns
    /// `None`.
    pub(crate) const fn new(s: &str) -> Option<ArrayStr<N>> {
        let len = s.len();
        if len > N {
            return None;
        }
        let mut bytes = [0; N];
        let mut i = 0;
        while i < s.as_bytes().len() {
            bytes[i] = s.as_bytes()[i];
            i += 1;
        }
        // OK because we don't ever use anything bigger than u8::MAX for `N`.
        // And we probably shouldn't, because that would be a pretty chunky
        // array. If such a thing is needed, please file an issue to discuss.
        debug_assert!(N <= u8::MAX as usize, "size of ArrayStr is too big");
        Some(ArrayStr { bytes, len: len as u8 })
    }

    /// Returns the capacity of this array string.
    pub(crate) fn capacity() -> usize {
        N
    }

    /// Append the bytes given to the end of this string.
    ///
    /// If the capacity would be exceeded, then this is a no-op and `false`
    /// is returned.
    pub(crate) fn push_str(&mut self, s: &str) -> bool {
        let len = usize::from(self.len);
        let Some(new_len) = len.checked_add(s.len()) else { return false };
        if new_len > N {
            return false;
        }
        self.bytes[len..new_len].copy_from_slice(s.as_bytes());
        // OK because we don't ever use anything bigger than u8::MAX for `N`.
        // And we probably shouldn't, because that would be a pretty chunky
        // array. If such a thing is needed, please file an issue to discuss.
        debug_assert!(
            N <= usize::from(u8::MAX),
            "size of ArrayStr is too big"
        );
        self.len = u8::try_from(new_len).unwrap();
        true
    }

    /// Returns this array string as a string slice.
    pub(crate) fn as_str(&self) -> &str {
        // OK because construction guarantees valid UTF-8.
        //
        // This is bullet proof enough to use unchecked `str` construction
        // here, but I can't dream up of a benchmark where it matters.
        core::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap()
    }
}

/// Easy construction of `ArrayStr` from `&'static str`.
///
/// We specifically limit to `&'static str` to approximate string literals.
/// This prevents most cases of accidentally creating a non-string literal
/// that panics if the string is too big.
///
/// This impl primarily exists to make writing tests more convenient.
impl<const N: usize> From<&'static str> for ArrayStr<N> {
    fn from(s: &'static str) -> ArrayStr<N> {
        ArrayStr::new(s).unwrap()
    }
}

impl<const N: usize> PartialEq<str> for ArrayStr<N> {
    fn eq(&self, rhs: &str) -> bool {
        self.as_str() == rhs
    }
}

impl<const N: usize> PartialEq<&str> for ArrayStr<N> {
    fn eq(&self, rhs: &&str) -> bool {
        self.as_str() == *rhs
    }
}

impl<const N: usize> PartialEq<ArrayStr<N>> for str {
    fn eq(&self, rhs: &ArrayStr<N>) -> bool {
        self == rhs.as_str()
    }
}

impl<const N: usize> core::fmt::Debug for ArrayStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize> core::fmt::Display for ArrayStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}

impl<const N: usize> core::fmt::Write for ArrayStr<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.push_str(s) {
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

impl<const N: usize> AsRef<str> for ArrayStr<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A self-imposed limit on the size of a time zone abbreviation, in bytes.
///
/// POSIX says this:
///
/// > Indicate no less than three, nor more than {TZNAME_MAX}, bytes that are
/// > the designation for the standard (std) or the alternative (dst -such as
/// > Daylight Savings Time) timezone.
///
/// But it doesn't seem worth the trouble to query `TZNAME_MAX`. Interestingly,
/// IANA says:
///
/// > are 3 or more characters specifying the standard and daylight saving time
/// > (DST) zone abbreviations
///
/// Which implies that IANA thinks there is no limit. But that seems unwise.
/// Moreover, in practice, it seems like the `date` utility supports fairly
/// long abbreviations. On my mac (so, BSD `date` as I understand it):
///
/// ```text
/// $ TZ=ZZZ5YYYYYYYYYYYYYYYYYYYYY date
/// Sun Mar 17 20:05:58 YYYYYYYYYYYYYYYYYYYYY 2024
/// ```
///
/// And on my Linux machine (so, GNU `date`):
///
/// ```text
/// $ TZ=ZZZ5YYYYYYYYYYYYYYYYYYYYY date
/// Sun Mar 17 08:05:36 PM YYYYYYYYYYYYYYYYYYYYY 2024
/// ```
///
/// I don't know exactly what limit these programs use, but 30 seems good
/// enough?
///
/// (Previously, I had been using 255 and stuffing the string in a `Box<str>`.
/// But as part of work on [#168], I was looking to remove allocation from as
/// many places as possible. And this was one candidate. But making room on the
/// stack for 255 byte abbreviations seemed gratuitous. So I picked something
/// smaller. If we come across an abbreviation bigger than this max, then we'll
/// error.)
///
/// [#168]: https://github.com/BurntSushi/jiff/issues/168
const ABBREVIATION_MAX: usize = 30;

/// A type alias for centralizing the definition of a time zone abbreviation.
///
/// Basically, this creates one single coherent place where we control the
/// length of a time zone abbreviation.
#[doc(hidden)] // not part of Jiff's public API
pub type Abbreviation = ArrayStr<ABBREVIATION_MAX>;

#[cfg(test)]
mod tests {
    use core::fmt::Write;

    use super::*;

    #[test]
    fn fmt_write() {
        let mut dst = ArrayStr::<5>::new("").unwrap();
        assert!(write!(&mut dst, "abcd").is_ok());
        assert!(write!(&mut dst, "e").is_ok());
        assert!(write!(&mut dst, "f").is_err());
    }
}

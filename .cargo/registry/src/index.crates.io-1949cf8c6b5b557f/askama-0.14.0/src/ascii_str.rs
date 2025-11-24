// FIXME: Replace `AsciiChar` with `[core:ascii::Char]` once [#110998] is stable
// [#110998]: https://github.com/rust-lang/rust/issues/110998

#![allow(unreachable_pub)]

use core::ops::{Deref, Index, IndexMut};

pub use _ascii_char::AsciiChar;

/// A string that only contains ASCII characters, same layout as [`str`].
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct AsciiStr([AsciiChar]);

impl AsciiStr {
    #[inline]
    pub const fn new_sized<const N: usize>(src: &str) -> [AsciiChar; N] {
        if !src.is_ascii() || src.len() > N {
            panic!();
        }

        let src = src.as_bytes();
        let mut result = [AsciiChar::NULL; N];
        let mut i = 0;
        while i < src.len() {
            result[i] = AsciiChar::new(src[i]);
            i += 1;
        }
        result
    }

    #[inline]
    pub const fn from_slice(src: &[AsciiChar]) -> &Self {
        // SAFETY: `Self` is transparent over `[AsciiChar]`.
        unsafe { core::mem::transmute::<&[AsciiChar], &AsciiStr>(src) }
    }

    #[inline]
    pub const fn as_str(&self) -> &str {
        // SAFETY: `Self` has the same layout as `str`,
        // and all ASCII characters are valid UTF-8 characters.
        unsafe { core::mem::transmute::<&AsciiStr, &str>(self) }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// Must not implement `DerefMut`. Not every `char` is an ASCII character.
impl Deref for AsciiStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<Idx> Index<Idx> for AsciiStr
where
    [AsciiChar]: Index<Idx, Output = [AsciiChar]>,
{
    type Output = [AsciiChar];

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl<Idx> IndexMut<Idx> for AsciiStr
where
    [AsciiChar]: IndexMut<Idx, Output = [AsciiChar]>,
{
    #[inline]
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Default for &'static AsciiStr {
    #[inline]
    fn default() -> Self {
        // SAFETY: `Self` has the same layout as `str`.
        unsafe { core::mem::transmute::<&str, &AsciiStr>("") }
    }
}

impl AsciiChar {
    pub const NULL: AsciiChar = AsciiChar::new(0);

    #[inline]
    pub const fn slice_as_bytes<const N: usize>(src: &[AsciiChar; N]) -> &[u8; N] {
        // SAFETY: `[AsciiChar]` has the same layout as `[u8]`.
        unsafe { core::mem::transmute::<&[AsciiChar; N], &[u8; N]>(src) }
    }

    #[inline]
    pub const fn two_digits(d: u32) -> [Self; 2] {
        const ALPHABET: &[u8; 10] = b"0123456789";

        if d >= ALPHABET.len().pow(2) as u32 {
            panic!();
        }
        [
            Self::new(ALPHABET[d as usize / ALPHABET.len()]),
            Self::new(ALPHABET[d as usize % ALPHABET.len()]),
        ]
    }

    #[inline]
    pub const fn two_hex_digits(d: u32) -> [Self; 2] {
        const ALPHABET: &[u8; 16] = b"0123456789abcdef";

        if d >= ALPHABET.len().pow(2) as u32 {
            panic!();
        }
        [
            Self::new(ALPHABET[d as usize / ALPHABET.len()]),
            Self::new(ALPHABET[d as usize % ALPHABET.len()]),
        ]
    }
}

mod _ascii_char {
    /// A character that is known to be in ASCII range, same layout as [`u8`].
    #[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(transparent)]
    pub struct AsciiChar(u8);

    impl AsciiChar {
        #[inline]
        pub const fn new(c: u8) -> Self {
            if c.is_ascii() { Self(c) } else { panic!() }
        }
    }
}

use alloc::string::String;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use self::map::lookup;
mod map;

#[derive(Clone, Copy, Debug, Default)]
pub struct Unicode<S>(pub S);

impl<S: AsRef<str>> Unicode<S> {
    pub fn to_folded_case(&self) -> String {
        self.0.as_ref().chars().flat_map(lookup).collect()
    }
}

impl<S1: AsRef<str>, S2: AsRef<str>> PartialEq<Unicode<S2>> for Unicode<S1> {
    #[inline]
    fn eq(&self, other: &Unicode<S2>) -> bool {
        let mut left = self.0.as_ref().chars().flat_map(lookup);
        let mut right = other.0.as_ref().chars().flat_map(lookup);

        // inline Iterator::eq since not added until Rust 1.5
        loop {
            let x = match left.next() {
                None => return right.next().is_none(),
                Some(val) => val,
            };

            let y = match right.next() {
                None => return false,
                Some(val) => val,
            };

            if x != y {
                return false;
            }
        }
    }
}

impl<S: AsRef<str>> Eq for Unicode<S> {}

impl<T: AsRef<str>> PartialOrd for Unicode<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: AsRef<str>> Ord for Unicode<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let self_chars = self.0.as_ref().chars().flat_map(lookup);
        let other_chars = other.0.as_ref().chars().flat_map(lookup);
        self_chars.cmp(other_chars)
    }
}

impl<S: AsRef<str>> Hash for Unicode<S> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        let mut buf = [0; 4];
        for c in self.0.as_ref().chars().flat_map(|c| lookup(c)) {
            let len = char_to_utf8(c, &mut buf);
            // we can't use `write(buf)` because the ASCII variant uses
            // `write_u8`. The docs for Hash say that's technically different.
            // ¯\_(ツ)_/¯
            for &b in &buf[..len] {
                hasher.write_u8(b);
            }
        }
        // prefix-freedom
        hasher.write_u8(0xFF);
    }
}

#[inline]
fn char_to_utf8(c: char, dst: &mut [u8; 4]) -> usize {
    const TAG_CONT: u8 = 0b1000_0000;
    const TAG_TWO_B: u8 = 0b1100_0000;
    const TAG_THREE_B: u8 = 0b1110_0000;
    const TAG_FOUR_B: u8 = 0b1111_0000;

    let code = c as u32;
    if code <= 0x7F {
        dst[0] = code as u8;
        1
    } else if code <= 0x7FF {
        dst[0] = (code >> 6 & 0x1F) as u8 | TAG_TWO_B;
        dst[1] = (code & 0x3F) as u8 | TAG_CONT;
        2
    } else if code <= 0xFFFF {
        dst[0] = (code >> 12 & 0x0F) as u8 | TAG_THREE_B;
        dst[1] = (code >> 6 & 0x3F) as u8 | TAG_CONT;
        dst[2] = (code & 0x3F) as u8 | TAG_CONT;
        3
    } else {
        dst[0] = (code >> 18 & 0x07) as u8 | TAG_FOUR_B;
        dst[1] = (code >> 12 & 0x3F) as u8 | TAG_CONT;
        dst[2] = (code >> 6 & 0x3F) as u8 | TAG_CONT;
        dst[3] = (code & 0x3F) as u8 | TAG_CONT;
        4
    }
}

// internal mod so that the enum can be 'pub'
// thanks privacy-checker :___(
mod fold {
    #[derive(Clone, Copy)]
    pub enum Fold {
        Zero,
        One(char),
        Two(char, char),
        Three(char, char, char),
    }

    impl Iterator for Fold {
        type Item = char;
        #[inline]
        fn next(&mut self) -> Option<char> {
            match *self {
                Fold::Zero => None,
                Fold::One(one) => {
                    *self = Fold::Zero;
                    Some(one)
                }
                Fold::Two(one, two) => {
                    *self = Fold::One(two);
                    Some(one)
                }
                Fold::Three(one, two, three) => {
                    *self = Fold::Two(one, two);
                    Some(three)
                }
            }
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            match *self {
                Fold::Zero => (0, Some(0)),
                Fold::One(..) => (1, Some(1)),
                Fold::Two(..) => (2, Some(2)),
                Fold::Three(..) => (3, Some(3)),
            }
        }
    }
    impl From<(char,)> for Fold {
        #[inline]
        fn from((one,): (char,)) -> Fold {
            Fold::One(one)
        }
    }

    impl From<(char, char)> for Fold {
        #[inline]
        fn from((one, two): (char, char)) -> Fold {
            Fold::Two(one, two)
        }
    }

    impl From<(char, char, char)> for Fold {
        #[inline]
        fn from((one, two, three): (char, char, char)) -> Fold {
            Fold::Three(one, two, three)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Unicode;

    macro_rules! eq {
        ($left:expr, $right:expr) => {{
            assert_eq!(Unicode($left), Unicode($right));
        }};
    }

    #[test]
    fn test_ascii_folding() {
        eq!("foo bar", "FoO BAR");
    }

    #[test]
    fn test_simple_case_folding() {
        eq!("στιγμας", "στιγμασ");
    }

    #[test]
    fn test_full_case_folding() {
        eq!("ﬂour", "flour");
        eq!("Maße", "MASSE");
        eq!("ᾲ στο διάολο", "ὰι στο διάολο");
    }

    #[test]
    fn test_to_folded_case() {
        assert_eq!(Unicode("Maße").to_folded_case(), "masse");
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_ascii_folding(b: &mut ::test::Bencher) {
        b.bytes = b"foo bar".len() as u64;
        b.iter(|| eq!("foo bar", "FoO BAR"));
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_simple_case_folding(b: &mut ::test::Bencher) {
        b.bytes = "στιγμας".len() as u64;
        b.iter(|| eq!("στιγμας", "στιγμασ"));
    }
}

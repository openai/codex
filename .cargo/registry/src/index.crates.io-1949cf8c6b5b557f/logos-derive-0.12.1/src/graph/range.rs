use regex_syntax::hir::ClassBytesRange;
use regex_syntax::hir::ClassUnicodeRange;
use regex_syntax::utf8::Utf8Range;

use std::cmp::{Ord, Ordering};

#[derive(Clone, Copy, PartialOrd, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: u8,
    pub end: u8,
}

impl Range {
    pub fn as_byte(&self) -> Option<u8> {
        if self.is_byte() {
            Some(self.start)
        } else {
            None
        }
    }

    pub fn is_byte(&self) -> bool {
        self.start == self.end
    }
}

impl From<u8> for Range {
    fn from(byte: u8) -> Range {
        Range {
            start: byte,
            end: byte,
        }
    }
}

impl From<&u8> for Range {
    fn from(byte: &u8) -> Range {
        Range::from(*byte)
    }
}

impl Iterator for Range {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        match self.start.cmp(&self.end) {
            std::cmp::Ordering::Less => {
                let res = self.start;
                self.start += 1;

                Some(res)
            }
            std::cmp::Ordering::Equal => {
                let res = self.start;

                // Necessary so that range 0xFF-0xFF doesn't loop forever
                self.start = 0xFF;
                self.end = 0x00;

                Some(res)
            }
            std::cmp::Ordering::Greater => None,
        }
    }
}

impl Ord for Range {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl From<Utf8Range> for Range {
    fn from(r: Utf8Range) -> Range {
        Range {
            start: r.start,
            end: r.end,
        }
    }
}

impl From<ClassUnicodeRange> for Range {
    fn from(r: ClassUnicodeRange) -> Range {
        let start = r.start() as u32;
        let end = r.end() as u32;

        if start >= 128 || end >= 128 && end != 0x0010FFFF {
            panic!("Casting non-ascii ClassUnicodeRange to Range")
        }

        Range {
            start: start as u8,
            end: end as u8,
        }
    }
}

impl From<ClassBytesRange> for Range {
    fn from(r: ClassBytesRange) -> Range {
        Range {
            start: r.start(),
            end: r.end(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_iter_one() {
        let byte = Range::from(b'!');
        let collected = byte.take(1000).collect::<Vec<_>>();

        assert_eq!(b"!", &collected[..]);
    }

    #[test]
    fn range_iter_few() {
        let byte = Range {
            start: b'a',
            end: b'd',
        };
        let collected = byte.take(1000).collect::<Vec<_>>();

        assert_eq!(b"abcd", &collected[..]);
    }

    #[test]
    fn range_iter_bunds() {
        let byte = Range::from(0xFA..=0xFF);

        let collected = byte.take(1000).collect::<Vec<_>>();

        assert_eq!(b"\xFA\xFB\xFC\xFD\xFE\xFF", &collected[..]);
    }
}

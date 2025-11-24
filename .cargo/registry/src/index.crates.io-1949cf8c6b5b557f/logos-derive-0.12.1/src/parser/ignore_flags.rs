use std::ops::{BitAnd, BitOr};

use proc_macro2::{Ident, TokenStream, TokenTree};

use crate::parser::Parser;
use crate::util::is_punct;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IgnoreFlags {
    bits: u8,
}

#[allow(non_upper_case_globals)]
impl IgnoreFlags {
    pub const Empty: Self = Self::new(0x00);
    pub const IgnoreCase: Self = Self::new(0x01);
    pub const IgnoreAsciiCase: Self = Self::new(0x02);

    #[inline]
    pub const fn new(bits: u8) -> Self {
        Self { bits }
    }

    /// Enables a variant.
    #[inline]
    pub fn enable(&mut self, variant: Self) {
        self.bits |= variant.bits;
    }

    /// Checks if this `IgnoreFlags` contains *any* of the given variants.
    #[inline]
    pub fn contains(&self, variants: Self) -> bool {
        self.bits & variants.bits != 0
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Parses an identifier an enables it for `self`.
    ///
    /// Valid inputs are (that produces `true`):
    /// * `"case"` (incompatible with `"ascii_case"`)
    /// * `"ascii_case"` (incompatible with `"case"`)
    ///
    /// An error causes this function to return `false` and emits an error to
    /// the given `Parser`.
    fn parse_ident(&mut self, ident: Ident, parser: &mut Parser) -> bool {
        match ident.to_string().as_str() {
            "case" => {
                if self.contains(Self::IgnoreAsciiCase) {
                    parser.err(
                        "\
                        The flag \"case\" cannot be used along with \"ascii_case\"\
                        ",
                        ident.span(),
                    );
                    false
                } else {
                    self.enable(Self::IgnoreCase);
                    true
                }
            }
            "ascii_case" => {
                if self.contains(Self::IgnoreCase) {
                    parser.err(
                        "\
                        The flag \"ascii_case\" cannot be used along with \"case\"\
                        ",
                        ident.span(),
                    );
                    false
                } else {
                    self.enable(Self::IgnoreAsciiCase);
                    true
                }
            }
            unknown => {
                parser.err(
                    format!(
                        "\
                        Unknown flag: {}\n\
                        \n\
                        Expected one of: case, ascii_case\
                        ",
                        unknown
                    ),
                    ident.span(),
                );
                false
            }
        }
    }

    pub fn parse_group(&mut self, name: Ident, tokens: TokenStream, parser: &mut Parser) {
        // Little finite state machine to parse "<flag>(,<flag>)*,?"

        // FSM description for future maintenance
        // 0: Initial state
        //   <flag> -> 1
        //        _ -> error
        // 1: A flag was found
        //        , -> 2
        //     None -> done
        //        _ -> error
        // 2: A comma was found (after a <flag>)
        //   <flag> -> 1
        //     None -> done
        //        _ -> error
        let mut state = 0u8;

        let mut tokens = tokens.into_iter();

        loop {
            state = match state {
                0 => match tokens.next() {
                    Some(TokenTree::Ident(ident)) => {
                        if self.parse_ident(ident, parser) {
                            1
                        } else {
                            return;
                        }
                    }
                    _ => {
                        parser.err(
                            "\
                            Invalid ignore flag\n\
                            \n\
                            Expected one of: case, ascii_case\
                            ",
                            name.span(),
                        );
                        return;
                    }
                },
                1 => match tokens.next() {
                    Some(tt) if is_punct(&tt, ',') => 2,
                    None => return,
                    Some(unexpected_tt) => {
                        parser.err(
                            format!(
                                "\
                                Unexpected token: {:?}\
                                ",
                                unexpected_tt.to_string(),
                            ),
                            unexpected_tt.span(),
                        );
                        return;
                    }
                },
                2 => match tokens.next() {
                    Some(TokenTree::Ident(ident)) => {
                        if self.parse_ident(ident, parser) {
                            1
                        } else {
                            return;
                        }
                    }
                    None => return,
                    Some(unexpected_tt) => {
                        parser.err(
                            format!(
                                "\
                                Unexpected token: {:?}\
                                ",
                                unexpected_tt.to_string(),
                            ),
                            unexpected_tt.span(),
                        );
                        return;
                    }
                },
                _ => unreachable!("Internal Error: invalid state ({})", state),
            }
        }
    }
}

impl BitOr for IgnoreFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self::new(self.bits | other.bits)
    }
}

impl BitAnd for IgnoreFlags {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        Self::new(self.bits & other.bits)
    }
}

pub mod ascii_case {
    use regex_syntax::hir;

    use crate::mir::Mir;
    use crate::parser::Literal;

    pub trait MakeAsciiCaseInsensitive {
        /// Creates a equivalent regular expression which ignore the letter casing
        /// of ascii characters.
        fn make_ascii_case_insensitive(self) -> Mir;
    }

    impl MakeAsciiCaseInsensitive for u8 {
        fn make_ascii_case_insensitive(self) -> Mir {
            if b'a' <= self && self <= b'z' {
                Mir::Alternation(vec![
                    Mir::Literal(hir::Literal::Byte(self - 32)),
                    Mir::Literal(hir::Literal::Byte(self)),
                ])
            } else if b'A' <= self && self <= b'Z' {
                Mir::Alternation(vec![
                    Mir::Literal(hir::Literal::Byte(self)),
                    Mir::Literal(hir::Literal::Byte(self + 32)),
                ])
            } else {
                Mir::Literal(hir::Literal::Byte(self))
            }
        }
    }

    impl MakeAsciiCaseInsensitive for char {
        fn make_ascii_case_insensitive(self) -> Mir {
            if self.is_ascii() {
                (self as u8).make_ascii_case_insensitive()
            } else {
                Mir::Literal(hir::Literal::Unicode(self))
            }
        }
    }

    impl MakeAsciiCaseInsensitive for hir::Literal {
        fn make_ascii_case_insensitive(self) -> Mir {
            match self {
                hir::Literal::Byte(b) => b.make_ascii_case_insensitive(),
                hir::Literal::Unicode(c) => c.make_ascii_case_insensitive(),
            }
        }
    }

    impl MakeAsciiCaseInsensitive for hir::ClassBytes {
        fn make_ascii_case_insensitive(mut self) -> Mir {
            self.case_fold_simple();
            Mir::Class(hir::Class::Bytes(self))
        }
    }

    impl MakeAsciiCaseInsensitive for hir::ClassUnicode {
        fn make_ascii_case_insensitive(mut self) -> Mir {
            use std::cmp;

            // Manuall implementation to only perform the case folding on ascii characters.

            let mut ranges = Vec::new();

            for range in self.ranges() {
                #[inline]
                fn overlaps(st1: u8, end1: u8, st2: u8, end2: u8) -> bool {
                    (st2 <= st1 && st1 <= end2) || (st1 <= st2 && st2 <= end1)
                }

                #[inline]
                fn make_ascii(c: char) -> Option<u8> {
                    if c.is_ascii() {
                        return Some(c as u8);
                    } else {
                        None
                    }
                }

                match (make_ascii(range.start()), make_ascii(range.end())) {
                    (Some(start), Some(end)) => {
                        if overlaps(b'a', b'z', start, end) {
                            let lower = cmp::max(start, b'a');
                            let upper = cmp::min(end, b'z');
                            ranges.push(hir::ClassUnicodeRange::new(
                                (lower - 32) as char,
                                (upper - 32) as char,
                            ))
                        }

                        if overlaps(b'A', b'Z', start, end) {
                            let lower = cmp::max(start, b'A');
                            let upper = cmp::min(end, b'Z');
                            ranges.push(hir::ClassUnicodeRange::new(
                                (lower + 32) as char,
                                (upper + 32) as char,
                            ))
                        }
                    }
                    (Some(start), None) => {
                        if overlaps(b'a', b'z', start, b'z') {
                            let lower = cmp::max(start, b'a');
                            ranges.push(hir::ClassUnicodeRange::new((lower - 32) as char, 'Z'))
                        }

                        if overlaps(b'A', b'Z', start, b'Z') {
                            let lower = cmp::max(start, b'A');
                            ranges.push(hir::ClassUnicodeRange::new((lower + 32) as char, 'Z'))
                        }
                    }
                    _ => (),
                }
            }

            self.union(&hir::ClassUnicode::new(ranges));

            Mir::Class(hir::Class::Unicode(self))
        }
    }

    impl MakeAsciiCaseInsensitive for hir::Class {
        fn make_ascii_case_insensitive(self) -> Mir {
            match self {
                hir::Class::Bytes(b) => b.make_ascii_case_insensitive(),
                hir::Class::Unicode(u) => u.make_ascii_case_insensitive(),
            }
        }
    }

    impl MakeAsciiCaseInsensitive for &Literal {
        fn make_ascii_case_insensitive(self) -> Mir {
            match self {
                Literal::Bytes(bytes) => Mir::Concat(
                    bytes
                        .value()
                        .into_iter()
                        .map(|b| b.make_ascii_case_insensitive())
                        .collect(),
                ),
                Literal::Utf8(s) => Mir::Concat(
                    s.value()
                        .chars()
                        .map(|b| b.make_ascii_case_insensitive())
                        .collect(),
                ),
            }
        }
    }

    impl MakeAsciiCaseInsensitive for Mir {
        fn make_ascii_case_insensitive(self) -> Mir {
            match self {
                Mir::Empty => Mir::Empty,
                Mir::Loop(l) => Mir::Loop(Box::new(l.make_ascii_case_insensitive())),
                Mir::Maybe(m) => Mir::Maybe(Box::new(m.make_ascii_case_insensitive())),
                Mir::Concat(c) => Mir::Concat(
                    c.into_iter()
                        .map(|m| m.make_ascii_case_insensitive())
                        .collect(),
                ),
                Mir::Alternation(a) => Mir::Alternation(
                    a.into_iter()
                        .map(|m| m.make_ascii_case_insensitive())
                        .collect(),
                ),
                Mir::Class(c) => c.make_ascii_case_insensitive(),
                Mir::Literal(l) => l.make_ascii_case_insensitive(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::MakeAsciiCaseInsensitive;
        use crate::mir::{Class, Mir};
        use regex_syntax::hir::{ClassUnicode, ClassUnicodeRange};

        fn assert_range(in_s: char, in_e: char, expected: &[(char, char)]) {
            let range = ClassUnicodeRange::new(in_s, in_e);
            let class = ClassUnicode::new(vec![range]);

            let expected =
                ClassUnicode::new(expected.iter().map(|&(a, b)| ClassUnicodeRange::new(a, b)));

            if let Mir::Class(Class::Unicode(result)) = class.make_ascii_case_insensitive() {
                assert_eq!(result, expected);
            } else {
                panic!("Not a unicode class");
            };
        }

        #[test]
        fn no_letters_left() {
            assert_range(' ', '+', &[(' ', '+')]);
        }

        #[test]
        fn no_letters_right() {
            assert_range('{', '~', &[('{', '~')]);
        }

        #[test]
        fn no_letters_middle() {
            assert_range('[', '`', &[('[', '`')]);
        }

        #[test]
        fn lowercase_left_edge() {
            assert_range('a', 'd', &[('a', 'd'), ('A', 'D')]);
        }

        #[test]
        fn lowercase_right_edge() {
            assert_range('r', 'z', &[('r', 'z'), ('R', 'Z')]);
        }

        #[test]
        fn lowercase_total() {
            assert_range('a', 'z', &[('a', 'z'), ('A', 'Z')]);
        }

        #[test]
        fn uppercase_left_edge() {
            assert_range('A', 'D', &[('a', 'd'), ('A', 'D')]);
        }

        #[test]
        fn uppercase_right_edge() {
            assert_range('R', 'Z', &[('r', 'z'), ('R', 'Z')]);
        }

        #[test]
        fn uppercase_total() {
            assert_range('A', 'Z', &[('a', 'z'), ('A', 'Z')]);
        }

        #[test]
        fn lowercase_cross_left() {
            assert_range('[', 'h', &[('[', 'h'), ('A', 'H')]);
        }

        #[test]
        fn lowercase_cross_right() {
            assert_range('d', '}', &[('d', '}'), ('D', 'Z')]);
        }

        #[test]
        fn uppercase_cross_left() {
            assert_range(';', 'H', &[(';', 'H'), ('a', 'h')]);
        }

        #[test]
        fn uppercase_cross_right() {
            assert_range('T', ']', &[('t', 'z'), ('T', ']')]);
        }

        #[test]
        fn cross_both() {
            assert_range('X', 'c', &[('X', 'c'), ('x', 'z'), ('A', 'C')]);
        }

        #[test]
        fn all_letters() {
            assert_range('+', '|', &[('+', '|')]);
        }

        #[test]
        fn oob_all_letters() {
            assert_range('#', 'é', &[('#', 'é')]);
        }

        #[test]
        fn oob_from_uppercase() {
            assert_range('Q', 'é', &[('A', 'é')]);
        }

        #[test]
        fn oob_from_lowercase() {
            assert_range('q', 'é', &[('q', 'é'), ('Q', 'Z')]);
        }

        #[test]
        fn oob_no_letters() {
            assert_range('|', 'é', &[('|', 'é')]);
        }
    }
}

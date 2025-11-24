/*!
This module provides a regular expression printer for `Hir`.
*/

use std::fmt;

use crate::hir::visitor::{self, Visitor};
use crate::hir::{self, Hir, HirKind};
use crate::is_meta_character;

/// A builder for constructing a printer.
///
/// Note that since a printer doesn't have any configuration knobs, this type
/// remains unexported.
#[derive(Clone, Debug)]
struct PrinterBuilder {
    _priv: (),
}

impl Default for PrinterBuilder {
    fn default() -> PrinterBuilder {
        PrinterBuilder::new()
    }
}

impl PrinterBuilder {
    fn new() -> PrinterBuilder {
        PrinterBuilder { _priv: () }
    }

    fn build(&self) -> Printer {
        Printer { _priv: () }
    }
}

/// A printer for a regular expression's high-level intermediate
/// representation.
///
/// A printer converts a high-level intermediate representation (HIR) to a
/// regular expression pattern string. This particular printer uses constant
/// stack space and heap space proportional to the size of the HIR.
///
/// Since this printer is only using the HIR, the pattern it prints will likely
/// not resemble the original pattern at all. For example, a pattern like
/// `\pL` will have its entire class written out.
///
/// The purpose of this printer is to provide a means to mutate an HIR and then
/// build a regular expression from the result of that mutation. (A regex
/// library could provide a constructor from this HIR explicitly, but that
/// creates an unnecessary public coupling between the regex library and this
/// specific HIR representation.)
#[derive(Debug)]
pub struct Printer {
    _priv: (),
}

impl Printer {
    /// Create a new printer.
    pub fn new() -> Printer {
        PrinterBuilder::new().build()
    }

    /// Print the given `Ast` to the given writer. The writer must implement
    /// `fmt::Write`. Typical implementations of `fmt::Write` that can be used
    /// here are a `fmt::Formatter` (which is available in `fmt::Display`
    /// implementations) or a `&mut String`.
    pub fn print<W: fmt::Write>(&mut self, hir: &Hir, wtr: W) -> fmt::Result {
        visitor::visit(hir, Writer { wtr })
    }
}

#[derive(Debug)]
struct Writer<W> {
    wtr: W,
}

impl<W: fmt::Write> Visitor for Writer<W> {
    type Output = ();
    type Err = fmt::Error;

    fn finish(self) -> fmt::Result {
        Ok(())
    }

    fn visit_pre(&mut self, hir: &Hir) -> fmt::Result {
        match *hir.kind() {
            HirKind::Empty
            | HirKind::Repetition(_)
            | HirKind::Concat(_)
            | HirKind::Alternation(_) => {}
            HirKind::Literal(hir::Literal::Unicode(c)) => {
                self.write_literal_char(c)?;
            }
            HirKind::Literal(hir::Literal::Byte(b)) => {
                self.write_literal_byte(b)?;
            }
            HirKind::Class(hir::Class::Unicode(ref cls)) => {
                self.wtr.write_str("[")?;
                for range in cls.iter() {
                    if range.start() == range.end() {
                        self.write_literal_char(range.start())?;
                    } else {
                        self.write_literal_char(range.start())?;
                        self.wtr.write_str("-")?;
                        self.write_literal_char(range.end())?;
                    }
                }
                self.wtr.write_str("]")?;
            }
            HirKind::Class(hir::Class::Bytes(ref cls)) => {
                self.wtr.write_str("(?-u:[")?;
                for range in cls.iter() {
                    if range.start() == range.end() {
                        self.write_literal_class_byte(range.start())?;
                    } else {
                        self.write_literal_class_byte(range.start())?;
                        self.wtr.write_str("-")?;
                        self.write_literal_class_byte(range.end())?;
                    }
                }
                self.wtr.write_str("])")?;
            }
            HirKind::Anchor(hir::Anchor::StartLine) => {
                self.wtr.write_str("(?m:^)")?;
            }
            HirKind::Anchor(hir::Anchor::EndLine) => {
                self.wtr.write_str("(?m:$)")?;
            }
            HirKind::Anchor(hir::Anchor::StartText) => {
                self.wtr.write_str(r"\A")?;
            }
            HirKind::Anchor(hir::Anchor::EndText) => {
                self.wtr.write_str(r"\z")?;
            }
            HirKind::WordBoundary(hir::WordBoundary::Unicode) => {
                self.wtr.write_str(r"\b")?;
            }
            HirKind::WordBoundary(hir::WordBoundary::UnicodeNegate) => {
                self.wtr.write_str(r"\B")?;
            }
            HirKind::WordBoundary(hir::WordBoundary::Ascii) => {
                self.wtr.write_str(r"(?-u:\b)")?;
            }
            HirKind::WordBoundary(hir::WordBoundary::AsciiNegate) => {
                self.wtr.write_str(r"(?-u:\B)")?;
            }
            HirKind::Group(ref x) => match x.kind {
                hir::GroupKind::CaptureIndex(_) => {
                    self.wtr.write_str("(")?;
                }
                hir::GroupKind::CaptureName { ref name, .. } => {
                    write!(self.wtr, "(?P<{}>", name)?;
                }
                hir::GroupKind::NonCapturing => {
                    self.wtr.write_str("(?:")?;
                }
            },
        }
        Ok(())
    }

    fn visit_post(&mut self, hir: &Hir) -> fmt::Result {
        match *hir.kind() {
            // Handled during visit_pre
            HirKind::Empty
            | HirKind::Literal(_)
            | HirKind::Class(_)
            | HirKind::Anchor(_)
            | HirKind::WordBoundary(_)
            | HirKind::Concat(_)
            | HirKind::Alternation(_) => {}
            HirKind::Repetition(ref x) => {
                match x.kind {
                    hir::RepetitionKind::ZeroOrOne => {
                        self.wtr.write_str("?")?;
                    }
                    hir::RepetitionKind::ZeroOrMore => {
                        self.wtr.write_str("*")?;
                    }
                    hir::RepetitionKind::OneOrMore => {
                        self.wtr.write_str("+")?;
                    }
                    hir::RepetitionKind::Range(ref x) => match *x {
                        hir::RepetitionRange::Exactly(m) => {
                            write!(self.wtr, "{{{}}}", m)?;
                        }
                        hir::RepetitionRange::AtLeast(m) => {
                            write!(self.wtr, "{{{},}}", m)?;
                        }
                        hir::RepetitionRange::Bounded(m, n) => {
                            write!(self.wtr, "{{{},{}}}", m, n)?;
                        }
                    },
                }
                if !x.greedy {
                    self.wtr.write_str("?")?;
                }
            }
            HirKind::Group(_) => {
                self.wtr.write_str(")")?;
            }
        }
        Ok(())
    }

    fn visit_alternation_in(&mut self) -> fmt::Result {
        self.wtr.write_str("|")
    }
}

impl<W: fmt::Write> Writer<W> {
    fn write_literal_char(&mut self, c: char) -> fmt::Result {
        if is_meta_character(c) {
            self.wtr.write_str("\\")?;
        }
        self.wtr.write_char(c)
    }

    fn write_literal_byte(&mut self, b: u8) -> fmt::Result {
        let c = b as char;
        if c <= 0x7F as char && !c.is_control() && !c.is_whitespace() {
            self.write_literal_char(c)
        } else {
            write!(self.wtr, "(?-u:\\x{:02X})", b)
        }
    }

    fn write_literal_class_byte(&mut self, b: u8) -> fmt::Result {
        let c = b as char;
        if c <= 0x7F as char && !c.is_control() && !c.is_whitespace() {
            self.write_literal_char(c)
        } else {
            write!(self.wtr, "\\x{:02X}", b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Printer;
    use crate::ParserBuilder;

    fn roundtrip(given: &str, expected: &str) {
        roundtrip_with(|b| b, given, expected);
    }

    fn roundtrip_bytes(given: &str, expected: &str) {
        roundtrip_with(|b| b.allow_invalid_utf8(true), given, expected);
    }

    fn roundtrip_with<F>(mut f: F, given: &str, expected: &str)
    where
        F: FnMut(&mut ParserBuilder) -> &mut ParserBuilder,
    {
        let mut builder = ParserBuilder::new();
        f(&mut builder);
        let hir = builder.build().parse(given).unwrap();

        let mut printer = Printer::new();
        let mut dst = String::new();
        printer.print(&hir, &mut dst).unwrap();

        // Check that the result is actually valid.
        builder.build().parse(&dst).unwrap();

        assert_eq!(expected, dst);
    }

    #[test]
    fn print_literal() {
        roundtrip("a", "a");
        roundtrip(r"\xff", "\u{FF}");
        roundtrip_bytes(r"\xff", "\u{FF}");
        roundtrip_bytes(r"(?-u)\xff", r"(?-u:\xFF)");
        roundtrip("☃", "☃");
    }

    #[test]
    fn print_class() {
        roundtrip(r"[a]", r"[a]");
        roundtrip(r"[a-z]", r"[a-z]");
        roundtrip(r"[a-z--b-c--x-y]", r"[ad-wz]");
        roundtrip(r"[^\x01-\u{10FFFF}]", "[\u{0}]");
        roundtrip(r"[-]", r"[\-]");
        roundtrip(r"[☃-⛄]", r"[☃-⛄]");

        roundtrip(r"(?-u)[a]", r"(?-u:[a])");
        roundtrip(r"(?-u)[a-z]", r"(?-u:[a-z])");
        roundtrip_bytes(r"(?-u)[a-\xFF]", r"(?-u:[a-\xFF])");

        // The following test that the printer escapes meta characters
        // in character classes.
        roundtrip(r"[\[]", r"[\[]");
        roundtrip(r"[Z-_]", r"[Z-_]");
        roundtrip(r"[Z-_--Z]", r"[\[-_]");

        // The following test that the printer escapes meta characters
        // in byte oriented character classes.
        roundtrip_bytes(r"(?-u)[\[]", r"(?-u:[\[])");
        roundtrip_bytes(r"(?-u)[Z-_]", r"(?-u:[Z-_])");
        roundtrip_bytes(r"(?-u)[Z-_--Z]", r"(?-u:[\[-_])");
    }

    #[test]
    fn print_anchor() {
        roundtrip(r"^", r"\A");
        roundtrip(r"$", r"\z");
        roundtrip(r"(?m)^", r"(?m:^)");
        roundtrip(r"(?m)$", r"(?m:$)");
    }

    #[test]
    fn print_word_boundary() {
        roundtrip(r"\b", r"\b");
        roundtrip(r"\B", r"\B");
        roundtrip(r"(?-u)\b", r"(?-u:\b)");
        roundtrip_bytes(r"(?-u)\B", r"(?-u:\B)");
    }

    #[test]
    fn print_repetition() {
        roundtrip("a?", "a?");
        roundtrip("a??", "a??");
        roundtrip("(?U)a?", "a??");

        roundtrip("a*", "a*");
        roundtrip("a*?", "a*?");
        roundtrip("(?U)a*", "a*?");

        roundtrip("a+", "a+");
        roundtrip("a+?", "a+?");
        roundtrip("(?U)a+", "a+?");

        roundtrip("a{1}", "a{1}");
        roundtrip("a{1,}", "a{1,}");
        roundtrip("a{1,5}", "a{1,5}");
        roundtrip("a{1}?", "a{1}?");
        roundtrip("a{1,}?", "a{1,}?");
        roundtrip("a{1,5}?", "a{1,5}?");
        roundtrip("(?U)a{1}", "a{1}?");
        roundtrip("(?U)a{1,}", "a{1,}?");
        roundtrip("(?U)a{1,5}", "a{1,5}?");
    }

    #[test]
    fn print_group() {
        roundtrip("()", "()");
        roundtrip("(?P<foo>)", "(?P<foo>)");
        roundtrip("(?:)", "(?:)");

        roundtrip("(a)", "(a)");
        roundtrip("(?P<foo>a)", "(?P<foo>a)");
        roundtrip("(?:a)", "(?:a)");

        roundtrip("((((a))))", "((((a))))");
    }

    #[test]
    fn print_alternation() {
        roundtrip("|", "|");
        roundtrip("||", "||");

        roundtrip("a|b", "a|b");
        roundtrip("a|b|c", "a|b|c");
        roundtrip("foo|bar|quux", "foo|bar|quux");
    }
}

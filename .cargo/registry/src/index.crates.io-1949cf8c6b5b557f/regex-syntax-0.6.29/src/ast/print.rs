/*!
This module provides a regular expression printer for `Ast`.
*/

use std::fmt;

use crate::ast::visitor::{self, Visitor};
use crate::ast::{self, Ast};

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

/// A printer for a regular expression abstract syntax tree.
///
/// A printer converts an abstract syntax tree (AST) to a regular expression
/// pattern string. This particular printer uses constant stack space and heap
/// space proportional to the size of the AST.
///
/// This printer will not necessarily preserve the original formatting of the
/// regular expression pattern string. For example, all whitespace and comments
/// are ignored.
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
    pub fn print<W: fmt::Write>(&mut self, ast: &Ast, wtr: W) -> fmt::Result {
        visitor::visit(ast, Writer { wtr })
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

    fn visit_pre(&mut self, ast: &Ast) -> fmt::Result {
        match *ast {
            Ast::Group(ref x) => self.fmt_group_pre(x),
            Ast::Class(ast::Class::Bracketed(ref x)) => {
                self.fmt_class_bracketed_pre(x)
            }
            _ => Ok(()),
        }
    }

    fn visit_post(&mut self, ast: &Ast) -> fmt::Result {
        use crate::ast::Class;

        match *ast {
            Ast::Empty(_) => Ok(()),
            Ast::Flags(ref x) => self.fmt_set_flags(x),
            Ast::Literal(ref x) => self.fmt_literal(x),
            Ast::Dot(_) => self.wtr.write_str("."),
            Ast::Assertion(ref x) => self.fmt_assertion(x),
            Ast::Class(Class::Perl(ref x)) => self.fmt_class_perl(x),
            Ast::Class(Class::Unicode(ref x)) => self.fmt_class_unicode(x),
            Ast::Class(Class::Bracketed(ref x)) => {
                self.fmt_class_bracketed_post(x)
            }
            Ast::Repetition(ref x) => self.fmt_repetition(x),
            Ast::Group(ref x) => self.fmt_group_post(x),
            Ast::Alternation(_) => Ok(()),
            Ast::Concat(_) => Ok(()),
        }
    }

    fn visit_alternation_in(&mut self) -> fmt::Result {
        self.wtr.write_str("|")
    }

    fn visit_class_set_item_pre(
        &mut self,
        ast: &ast::ClassSetItem,
    ) -> Result<(), Self::Err> {
        match *ast {
            ast::ClassSetItem::Bracketed(ref x) => {
                self.fmt_class_bracketed_pre(x)
            }
            _ => Ok(()),
        }
    }

    fn visit_class_set_item_post(
        &mut self,
        ast: &ast::ClassSetItem,
    ) -> Result<(), Self::Err> {
        use crate::ast::ClassSetItem::*;

        match *ast {
            Empty(_) => Ok(()),
            Literal(ref x) => self.fmt_literal(x),
            Range(ref x) => {
                self.fmt_literal(&x.start)?;
                self.wtr.write_str("-")?;
                self.fmt_literal(&x.end)?;
                Ok(())
            }
            Ascii(ref x) => self.fmt_class_ascii(x),
            Unicode(ref x) => self.fmt_class_unicode(x),
            Perl(ref x) => self.fmt_class_perl(x),
            Bracketed(ref x) => self.fmt_class_bracketed_post(x),
            Union(_) => Ok(()),
        }
    }

    fn visit_class_set_binary_op_in(
        &mut self,
        ast: &ast::ClassSetBinaryOp,
    ) -> Result<(), Self::Err> {
        self.fmt_class_set_binary_op_kind(&ast.kind)
    }
}

impl<W: fmt::Write> Writer<W> {
    fn fmt_group_pre(&mut self, ast: &ast::Group) -> fmt::Result {
        use crate::ast::GroupKind::*;
        match ast.kind {
            CaptureIndex(_) => self.wtr.write_str("("),
            CaptureName(ref x) => {
                self.wtr.write_str("(?P<")?;
                self.wtr.write_str(&x.name)?;
                self.wtr.write_str(">")?;
                Ok(())
            }
            NonCapturing(ref flags) => {
                self.wtr.write_str("(?")?;
                self.fmt_flags(flags)?;
                self.wtr.write_str(":")?;
                Ok(())
            }
        }
    }

    fn fmt_group_post(&mut self, _ast: &ast::Group) -> fmt::Result {
        self.wtr.write_str(")")
    }

    fn fmt_repetition(&mut self, ast: &ast::Repetition) -> fmt::Result {
        use crate::ast::RepetitionKind::*;
        match ast.op.kind {
            ZeroOrOne if ast.greedy => self.wtr.write_str("?"),
            ZeroOrOne => self.wtr.write_str("??"),
            ZeroOrMore if ast.greedy => self.wtr.write_str("*"),
            ZeroOrMore => self.wtr.write_str("*?"),
            OneOrMore if ast.greedy => self.wtr.write_str("+"),
            OneOrMore => self.wtr.write_str("+?"),
            Range(ref x) => {
                self.fmt_repetition_range(x)?;
                if !ast.greedy {
                    self.wtr.write_str("?")?;
                }
                Ok(())
            }
        }
    }

    fn fmt_repetition_range(
        &mut self,
        ast: &ast::RepetitionRange,
    ) -> fmt::Result {
        use crate::ast::RepetitionRange::*;
        match *ast {
            Exactly(x) => write!(self.wtr, "{{{}}}", x),
            AtLeast(x) => write!(self.wtr, "{{{},}}", x),
            Bounded(x, y) => write!(self.wtr, "{{{},{}}}", x, y),
        }
    }

    fn fmt_literal(&mut self, ast: &ast::Literal) -> fmt::Result {
        use crate::ast::LiteralKind::*;

        match ast.kind {
            Verbatim => self.wtr.write_char(ast.c),
            Punctuation => write!(self.wtr, r"\{}", ast.c),
            Octal => write!(self.wtr, r"\{:o}", ast.c as u32),
            HexFixed(ast::HexLiteralKind::X) => {
                write!(self.wtr, r"\x{:02X}", ast.c as u32)
            }
            HexFixed(ast::HexLiteralKind::UnicodeShort) => {
                write!(self.wtr, r"\u{:04X}", ast.c as u32)
            }
            HexFixed(ast::HexLiteralKind::UnicodeLong) => {
                write!(self.wtr, r"\U{:08X}", ast.c as u32)
            }
            HexBrace(ast::HexLiteralKind::X) => {
                write!(self.wtr, r"\x{{{:X}}}", ast.c as u32)
            }
            HexBrace(ast::HexLiteralKind::UnicodeShort) => {
                write!(self.wtr, r"\u{{{:X}}}", ast.c as u32)
            }
            HexBrace(ast::HexLiteralKind::UnicodeLong) => {
                write!(self.wtr, r"\U{{{:X}}}", ast.c as u32)
            }
            Special(ast::SpecialLiteralKind::Bell) => {
                self.wtr.write_str(r"\a")
            }
            Special(ast::SpecialLiteralKind::FormFeed) => {
                self.wtr.write_str(r"\f")
            }
            Special(ast::SpecialLiteralKind::Tab) => self.wtr.write_str(r"\t"),
            Special(ast::SpecialLiteralKind::LineFeed) => {
                self.wtr.write_str(r"\n")
            }
            Special(ast::SpecialLiteralKind::CarriageReturn) => {
                self.wtr.write_str(r"\r")
            }
            Special(ast::SpecialLiteralKind::VerticalTab) => {
                self.wtr.write_str(r"\v")
            }
            Special(ast::SpecialLiteralKind::Space) => {
                self.wtr.write_str(r"\ ")
            }
        }
    }

    fn fmt_assertion(&mut self, ast: &ast::Assertion) -> fmt::Result {
        use crate::ast::AssertionKind::*;
        match ast.kind {
            StartLine => self.wtr.write_str("^"),
            EndLine => self.wtr.write_str("$"),
            StartText => self.wtr.write_str(r"\A"),
            EndText => self.wtr.write_str(r"\z"),
            WordBoundary => self.wtr.write_str(r"\b"),
            NotWordBoundary => self.wtr.write_str(r"\B"),
        }
    }

    fn fmt_set_flags(&mut self, ast: &ast::SetFlags) -> fmt::Result {
        self.wtr.write_str("(?")?;
        self.fmt_flags(&ast.flags)?;
        self.wtr.write_str(")")?;
        Ok(())
    }

    fn fmt_flags(&mut self, ast: &ast::Flags) -> fmt::Result {
        use crate::ast::{Flag, FlagsItemKind};

        for item in &ast.items {
            match item.kind {
                FlagsItemKind::Negation => self.wtr.write_str("-"),
                FlagsItemKind::Flag(ref flag) => match *flag {
                    Flag::CaseInsensitive => self.wtr.write_str("i"),
                    Flag::MultiLine => self.wtr.write_str("m"),
                    Flag::DotMatchesNewLine => self.wtr.write_str("s"),
                    Flag::SwapGreed => self.wtr.write_str("U"),
                    Flag::Unicode => self.wtr.write_str("u"),
                    Flag::IgnoreWhitespace => self.wtr.write_str("x"),
                },
            }?;
        }
        Ok(())
    }

    fn fmt_class_bracketed_pre(
        &mut self,
        ast: &ast::ClassBracketed,
    ) -> fmt::Result {
        if ast.negated {
            self.wtr.write_str("[^")
        } else {
            self.wtr.write_str("[")
        }
    }

    fn fmt_class_bracketed_post(
        &mut self,
        _ast: &ast::ClassBracketed,
    ) -> fmt::Result {
        self.wtr.write_str("]")
    }

    fn fmt_class_set_binary_op_kind(
        &mut self,
        ast: &ast::ClassSetBinaryOpKind,
    ) -> fmt::Result {
        use crate::ast::ClassSetBinaryOpKind::*;
        match *ast {
            Intersection => self.wtr.write_str("&&"),
            Difference => self.wtr.write_str("--"),
            SymmetricDifference => self.wtr.write_str("~~"),
        }
    }

    fn fmt_class_perl(&mut self, ast: &ast::ClassPerl) -> fmt::Result {
        use crate::ast::ClassPerlKind::*;
        match ast.kind {
            Digit if ast.negated => self.wtr.write_str(r"\D"),
            Digit => self.wtr.write_str(r"\d"),
            Space if ast.negated => self.wtr.write_str(r"\S"),
            Space => self.wtr.write_str(r"\s"),
            Word if ast.negated => self.wtr.write_str(r"\W"),
            Word => self.wtr.write_str(r"\w"),
        }
    }

    fn fmt_class_ascii(&mut self, ast: &ast::ClassAscii) -> fmt::Result {
        use crate::ast::ClassAsciiKind::*;
        match ast.kind {
            Alnum if ast.negated => self.wtr.write_str("[:^alnum:]"),
            Alnum => self.wtr.write_str("[:alnum:]"),
            Alpha if ast.negated => self.wtr.write_str("[:^alpha:]"),
            Alpha => self.wtr.write_str("[:alpha:]"),
            Ascii if ast.negated => self.wtr.write_str("[:^ascii:]"),
            Ascii => self.wtr.write_str("[:ascii:]"),
            Blank if ast.negated => self.wtr.write_str("[:^blank:]"),
            Blank => self.wtr.write_str("[:blank:]"),
            Cntrl if ast.negated => self.wtr.write_str("[:^cntrl:]"),
            Cntrl => self.wtr.write_str("[:cntrl:]"),
            Digit if ast.negated => self.wtr.write_str("[:^digit:]"),
            Digit => self.wtr.write_str("[:digit:]"),
            Graph if ast.negated => self.wtr.write_str("[:^graph:]"),
            Graph => self.wtr.write_str("[:graph:]"),
            Lower if ast.negated => self.wtr.write_str("[:^lower:]"),
            Lower => self.wtr.write_str("[:lower:]"),
            Print if ast.negated => self.wtr.write_str("[:^print:]"),
            Print => self.wtr.write_str("[:print:]"),
            Punct if ast.negated => self.wtr.write_str("[:^punct:]"),
            Punct => self.wtr.write_str("[:punct:]"),
            Space if ast.negated => self.wtr.write_str("[:^space:]"),
            Space => self.wtr.write_str("[:space:]"),
            Upper if ast.negated => self.wtr.write_str("[:^upper:]"),
            Upper => self.wtr.write_str("[:upper:]"),
            Word if ast.negated => self.wtr.write_str("[:^word:]"),
            Word => self.wtr.write_str("[:word:]"),
            Xdigit if ast.negated => self.wtr.write_str("[:^xdigit:]"),
            Xdigit => self.wtr.write_str("[:xdigit:]"),
        }
    }

    fn fmt_class_unicode(&mut self, ast: &ast::ClassUnicode) -> fmt::Result {
        use crate::ast::ClassUnicodeKind::*;
        use crate::ast::ClassUnicodeOpKind::*;

        if ast.negated {
            self.wtr.write_str(r"\P")?;
        } else {
            self.wtr.write_str(r"\p")?;
        }
        match ast.kind {
            OneLetter(c) => self.wtr.write_char(c),
            Named(ref x) => write!(self.wtr, "{{{}}}", x),
            NamedValue { op: Equal, ref name, ref value } => {
                write!(self.wtr, "{{{}={}}}", name, value)
            }
            NamedValue { op: Colon, ref name, ref value } => {
                write!(self.wtr, "{{{}:{}}}", name, value)
            }
            NamedValue { op: NotEqual, ref name, ref value } => {
                write!(self.wtr, "{{{}!={}}}", name, value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Printer;
    use crate::ast::parse::ParserBuilder;

    fn roundtrip(given: &str) {
        roundtrip_with(|b| b, given);
    }

    fn roundtrip_with<F>(mut f: F, given: &str)
    where
        F: FnMut(&mut ParserBuilder) -> &mut ParserBuilder,
    {
        let mut builder = ParserBuilder::new();
        f(&mut builder);
        let ast = builder.build().parse(given).unwrap();

        let mut printer = Printer::new();
        let mut dst = String::new();
        printer.print(&ast, &mut dst).unwrap();
        assert_eq!(given, dst);
    }

    #[test]
    fn print_literal() {
        roundtrip("a");
        roundtrip(r"\[");
        roundtrip_with(|b| b.octal(true), r"\141");
        roundtrip(r"\x61");
        roundtrip(r"\x7F");
        roundtrip(r"\u0061");
        roundtrip(r"\U00000061");
        roundtrip(r"\x{61}");
        roundtrip(r"\x{7F}");
        roundtrip(r"\u{61}");
        roundtrip(r"\U{61}");

        roundtrip(r"\a");
        roundtrip(r"\f");
        roundtrip(r"\t");
        roundtrip(r"\n");
        roundtrip(r"\r");
        roundtrip(r"\v");
        roundtrip(r"(?x)\ ");
    }

    #[test]
    fn print_dot() {
        roundtrip(".");
    }

    #[test]
    fn print_concat() {
        roundtrip("ab");
        roundtrip("abcde");
        roundtrip("a(bcd)ef");
    }

    #[test]
    fn print_alternation() {
        roundtrip("a|b");
        roundtrip("a|b|c|d|e");
        roundtrip("|a|b|c|d|e");
        roundtrip("|a|b|c|d|e|");
        roundtrip("a(b|c|d)|e|f");
    }

    #[test]
    fn print_assertion() {
        roundtrip(r"^");
        roundtrip(r"$");
        roundtrip(r"\A");
        roundtrip(r"\z");
        roundtrip(r"\b");
        roundtrip(r"\B");
    }

    #[test]
    fn print_repetition() {
        roundtrip("a?");
        roundtrip("a??");
        roundtrip("a*");
        roundtrip("a*?");
        roundtrip("a+");
        roundtrip("a+?");
        roundtrip("a{5}");
        roundtrip("a{5}?");
        roundtrip("a{5,}");
        roundtrip("a{5,}?");
        roundtrip("a{5,10}");
        roundtrip("a{5,10}?");
    }

    #[test]
    fn print_flags() {
        roundtrip("(?i)");
        roundtrip("(?-i)");
        roundtrip("(?s-i)");
        roundtrip("(?-si)");
        roundtrip("(?siUmux)");
    }

    #[test]
    fn print_group() {
        roundtrip("(?i:a)");
        roundtrip("(?P<foo>a)");
        roundtrip("(a)");
    }

    #[test]
    fn print_class() {
        roundtrip(r"[abc]");
        roundtrip(r"[a-z]");
        roundtrip(r"[^a-z]");
        roundtrip(r"[a-z0-9]");
        roundtrip(r"[-a-z0-9]");
        roundtrip(r"[-a-z0-9]");
        roundtrip(r"[a-z0-9---]");
        roundtrip(r"[a-z&&m-n]");
        roundtrip(r"[[a-z&&m-n]]");
        roundtrip(r"[a-z--m-n]");
        roundtrip(r"[a-z~~m-n]");
        roundtrip(r"[a-z[0-9]]");
        roundtrip(r"[a-z[^0-9]]");

        roundtrip(r"\d");
        roundtrip(r"\D");
        roundtrip(r"\s");
        roundtrip(r"\S");
        roundtrip(r"\w");
        roundtrip(r"\W");

        roundtrip(r"[[:alnum:]]");
        roundtrip(r"[[:^alnum:]]");
        roundtrip(r"[[:alpha:]]");
        roundtrip(r"[[:^alpha:]]");
        roundtrip(r"[[:ascii:]]");
        roundtrip(r"[[:^ascii:]]");
        roundtrip(r"[[:blank:]]");
        roundtrip(r"[[:^blank:]]");
        roundtrip(r"[[:cntrl:]]");
        roundtrip(r"[[:^cntrl:]]");
        roundtrip(r"[[:digit:]]");
        roundtrip(r"[[:^digit:]]");
        roundtrip(r"[[:graph:]]");
        roundtrip(r"[[:^graph:]]");
        roundtrip(r"[[:lower:]]");
        roundtrip(r"[[:^lower:]]");
        roundtrip(r"[[:print:]]");
        roundtrip(r"[[:^print:]]");
        roundtrip(r"[[:punct:]]");
        roundtrip(r"[[:^punct:]]");
        roundtrip(r"[[:space:]]");
        roundtrip(r"[[:^space:]]");
        roundtrip(r"[[:upper:]]");
        roundtrip(r"[[:^upper:]]");
        roundtrip(r"[[:word:]]");
        roundtrip(r"[[:^word:]]");
        roundtrip(r"[[:xdigit:]]");
        roundtrip(r"[[:^xdigit:]]");

        roundtrip(r"\pL");
        roundtrip(r"\PL");
        roundtrip(r"\p{L}");
        roundtrip(r"\P{L}");
        roundtrip(r"\p{X=Y}");
        roundtrip(r"\P{X=Y}");
        roundtrip(r"\p{X:Y}");
        roundtrip(r"\P{X:Y}");
        roundtrip(r"\p{X!=Y}");
        roundtrip(r"\P{X!=Y}");
    }
}

//! Simple Rust AST. This is what the various code generators create,
//! which then gets serialized.

use crate::grammar::parse_tree::Visibility;
use crate::grammar::repr::{self, Grammar};
use crate::tls::Tls;
use std::fmt::{self, Display};
use std::io::{self, Write};

macro_rules! rust {
    ($w:expr, $($args:tt)*) => {
        ($w).writeln(&::std::fmt::format(format_args!($($args)*)))?
    }
}

/// A wrapper around a Write instance that handles indentation for
/// Rust code. It expects Rust code to be written in a stylized way,
/// with lots of braces and newlines (example shown here with no
/// indentation). Over time maybe we can extend this to make things
/// look prettier, but seems like...meh, just run it through some
/// rustfmt tool.
///
/// ```ignore
/// fn foo(
/// arg1: Type1,
/// arg2: Type2,
/// arg3: Type3)
/// -> ReturnType
/// {
/// match foo {
/// Variant => {
/// }
/// }
/// }
/// ```
pub struct RustWrite<W: Write> {
    write: W,
    indent: usize,
}

const TAB: usize = 4;

impl<'me, W: Write> RustWrite<W> {
    pub fn new(w: W) -> RustWrite<W> {
        RustWrite {
            write: w,
            indent: 0,
        }
    }

    pub fn into_inner(self) -> W {
        self.write
    }

    fn write_indentation(&mut self) -> io::Result<()> {
        if Tls::session().emit_whitespace {
            write!(self.write, "{0:1$}", "", self.indent)?;
        }
        Ok(())
    }

    fn write_indented(&mut self, out: &str) -> io::Result<()> {
        self.write_indentation()?;
        writeln!(self.write, "{}", out)
    }

    pub fn write_table_row<I, C>(&mut self, iterable: I) -> io::Result<()>
    where
        I: IntoIterator<Item = (i32, C)>,
        C: fmt::Display,
    {
        let session = Tls::session();
        if session.emit_comments {
            for (i, comment) in iterable {
                self.write_indentation()?;
                writeln!(self.write, "{}, {}", i, comment)?;
            }
        } else {
            self.write_indentation()?;
            let mut first = true;
            for (i, _comment) in iterable {
                if !first && session.emit_whitespace {
                    write!(self.write, " ")?;
                }
                write!(self.write, "{},", i)?;
                first = false;
            }
        }
        writeln!(self.write)
    }

    pub fn writeln(&mut self, out: &str) -> io::Result<()> {
        let buf = out.as_bytes();

        // pass empty lines through with no indentation
        if buf.is_empty() {
            return self.write.write_all("\n".as_bytes());
        }

        let n = buf.len() - 1;

        // If the line begins with a `}`, `]`, or `)`, first decrement the indentation.
        if buf[0] == b'}' || buf[0] == b']' || buf[0] == b')' {
            self.indent -= TAB;
        }

        self.write_indented(out)?;

        // Detect a line that ends in a `{` or `(` and increase indentation for future lines.
        if buf[n] == b'{' || buf[n] == b'[' || buf[n] == b'(' {
            self.indent += TAB;
        }

        Ok(())
    }

    /// Create and return fn-header builder. Don't forget to invoke
    /// `emit` at the end. =)
    pub fn fn_header(&'me mut self, visibility: &'me Visibility, name: String) -> FnHeader<'me, W> {
        FnHeader::new(self, visibility, name)
    }

    pub fn write_module_attributes(&mut self, grammar: &Grammar) -> io::Result<()> {
        for attribute in grammar.module_attributes.iter() {
            rust!(self, "{}", attribute);
        }
        Ok(())
    }

    pub fn write_uses(&mut self, super_prefix: &str, grammar: &Grammar) -> io::Result<()> {
        // things the user wrote
        for u in &grammar.uses {
            if u.starts_with("super::") {
                rust!(self, "use {}{};", super_prefix, u);
            } else {
                rust!(self, "use {};", u);
            }
        }

        self.write_standard_uses(&grammar.prefix)
    }

    pub fn write_standard_uses(&mut self, prefix: &str) -> io::Result<()> {
        // Stuff that we plan to use.
        // Occasionally we happen to not use it after all, hence the allow.
        rust!(self, "#[allow(unused_extern_crates)]");
        rust!(
            self,
            "extern crate lalrpop_util as {p}lalrpop_util;",
            p = prefix,
        );
        rust!(self, "#[allow(unused_imports)]");
        rust!(
            self,
            "use self::{p}lalrpop_util::state_machine as {p}state_machine;",
            p = prefix,
        );
        rust!(self, "extern crate core;");
        rust!(self, "extern crate alloc;");

        Ok(())
    }
}

pub struct FnHeader<'me, W: Write + 'me> {
    write: &'me mut RustWrite<W>,
    visibility: &'me Visibility,
    name: String,
    type_parameters: Vec<String>,
    parameters: Vec<String>,
    return_type: String,
    where_clauses: Vec<String>,
}

impl<'me, W: Write> FnHeader<'me, W> {
    pub fn new(write: &'me mut RustWrite<W>, visibility: &'me Visibility, name: String) -> Self {
        FnHeader {
            write,
            visibility,
            name,
            type_parameters: vec![],
            parameters: vec![],
            return_type: "()".to_string(),
            where_clauses: vec![],
        }
    }

    /// Adds the type-parameters, where-clauses, and parameters from
    /// the grammar.
    pub fn with_grammar(self, grammar: &Grammar) -> Self {
        self.with_type_parameters(&grammar.type_parameters)
            .with_where_clauses(&grammar.where_clauses)
            .with_parameters(&grammar.parameters)
    }

    /// Declare a series of type parameters. Note that lifetime
    /// parameters must come first.
    pub fn with_type_parameters(mut self, tps: impl IntoIterator<Item = impl Display>) -> Self {
        self.type_parameters
            .extend(tps.into_iter().map(|t| t.to_string()));
        self
    }

    /// Add where clauses to the list.
    pub fn with_where_clauses(mut self, tps: impl IntoIterator<Item = impl Display>) -> Self {
        self.where_clauses
            .extend(tps.into_iter().map(|t| t.to_string()));
        self
    }

    /// Declare a series of parameters. You can supply strings of the
    /// form `"foo: Bar"` or else `repr::Parameter` references.
    pub fn with_parameters(
        mut self,
        parameters: impl IntoIterator<Item = impl ParameterDisplay>,
    ) -> Self {
        self.parameters.extend(
            parameters
                .into_iter()
                .map(ParameterDisplay::to_parameter_string),
        );
        self
    }

    /// Add where clauses to the list.
    pub fn with_return_type(mut self, rt: impl Display) -> Self {
        self.return_type = format!("{}", rt);
        self
    }

    /// Emit fn header -- everything up to the opening `{` for the
    /// body.
    pub fn emit(self) -> io::Result<()> {
        rust!(self.write, "{}fn {}<", self.visibility, self.name);

        for type_parameter in &self.type_parameters {
            rust!(self.write, "{0:1$}{2},", "", TAB, type_parameter);
        }

        rust!(self.write, ">(");

        for parameter in &self.parameters {
            rust!(self.write, "{},", parameter);
        }

        rust!(self.write, ") -> {}", self.return_type);

        if !self.where_clauses.is_empty() {
            rust!(self.write, "where");

            for where_clause in &self.where_clauses {
                rust!(self.write, "    {},", where_clause);
            }
        }

        Ok(())
    }
}

pub trait ParameterDisplay {
    fn to_parameter_string(self) -> String;
}

impl ParameterDisplay for String {
    fn to_parameter_string(self) -> String {
        self
    }
}

impl<'me> ParameterDisplay for &'me repr::Parameter {
    fn to_parameter_string(self) -> String {
        format!("{}: {}", self.name, self.ty)
    }
}

//! The definition of patterns is shared between the parse-tree and the
//! repr, but customized by a type T that represents the different type
//! representations.

use crate::grammar::parse_tree::{Path, Span};
use std::fmt::{Display, Error, Formatter};
use string_cache::DefaultAtom as Atom;
use crate::util::Sep;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern<T> {
    pub span: Span,
    pub kind: PatternKind<T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldPattern<T> {
    pub field_span: Span,
    pub field_name: Atom,
    pub pattern: Pattern<T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind<T> {
    Enum(Path, Vec<Pattern<T>>),
    Struct(Path, Vec<FieldPattern<T>>, /* trailing ..? */ bool),
    Path(Path),
    Tuple(Vec<Pattern<T>>),
    TupleStruct(Path, Vec<Pattern<T>>),
    Usize(usize),
    Underscore,
    DotDot,
    Choose(T),
    CharLiteral(Atom),
    String(String),
}

impl<T> Pattern<T> {
    pub fn for_each_binding<U>(&self, map_fn: &mut dyn FnMut(&T) -> U) {
        self.map(map_fn);
    }

    pub fn map<U>(&self, map_fn: &mut dyn FnMut(&T) -> U) -> Pattern<U> {
        Pattern {
            span: self.span,
            kind: self.kind.map(map_fn),
        }
    }
}

impl<T> PatternKind<T> {
    pub fn map<U>(&self, map_fn: &mut dyn FnMut(&T) -> U) -> PatternKind<U> {
        match *self {
            PatternKind::Path(ref path) => PatternKind::Path(path.clone()),
            PatternKind::Enum(ref path, ref pats) => PatternKind::Enum(
                path.clone(),
                pats.iter().map(|pat| pat.map(map_fn)).collect(),
            ),
            PatternKind::Struct(ref path, ref fields, dotdot) => PatternKind::Struct(
                path.clone(),
                fields.iter().map(|pat| pat.map(map_fn)).collect(),
                dotdot,
            ),
            PatternKind::Tuple(ref pats) => {
                PatternKind::Tuple(pats.iter().map(|p| p.map(map_fn)).collect())
            }
            PatternKind::TupleStruct(ref path, ref pats) => {
                PatternKind::TupleStruct(path.clone(), pats.iter().map(|p| p.map(map_fn)).collect())
            }
            PatternKind::Underscore => PatternKind::Underscore,
            PatternKind::DotDot => PatternKind::DotDot,
            PatternKind::Usize(n) => PatternKind::Usize(n),
            PatternKind::Choose(ref ty) => PatternKind::Choose(map_fn(ty)),
            PatternKind::CharLiteral(ref c) => PatternKind::CharLiteral(c.clone()),
            PatternKind::String(ref s) => PatternKind::String(s.clone()),
        }
    }
}

impl<T> FieldPattern<T> {
    pub fn map<U>(&self, map_fn: &mut dyn FnMut(&T) -> U) -> FieldPattern<U> {
        FieldPattern {
            field_name: self.field_name.clone(),
            field_span: self.field_span,
            pattern: self.pattern.map(map_fn),
        }
    }
}

impl<T: Display> Display for Pattern<T> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}", self.kind)
    }
}

impl<T: Display> Display for PatternKind<T> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            PatternKind::Path(ref path) => write!(fmt, "{}", path),
            PatternKind::Enum(ref path, ref pats) => write!(fmt, "{}({})", path, Sep(", ", pats)),
            PatternKind::Struct(ref path, ref fields, false) => {
                write!(fmt, "{} {{ {} }}", path, Sep(", ", fields))
            }
            PatternKind::Struct(ref path, ref fields, true) if fields.is_empty() => {
                write!(fmt, "{} {{ .. }}", path)
            }
            PatternKind::Struct(ref path, ref fields, true) => {
                write!(fmt, "{} {{ {}, .. }}", path, Sep(", ", fields))
            }
            PatternKind::Tuple(ref paths) => write!(fmt, "({})", Sep(", ", paths)),
            PatternKind::TupleStruct(ref path, ref paths) => {
                write!(fmt, "{}({})", path, Sep(", ", paths))
            }
            PatternKind::Underscore => write!(fmt, "_"),
            PatternKind::DotDot => write!(fmt, ".."),
            PatternKind::Usize(n) => write!(fmt, "{}", n),
            PatternKind::Choose(ref ty) => write!(fmt, "{}", ty),
            PatternKind::CharLiteral(ref c) => write!(fmt, "'{}'", c),
            PatternKind::String(ref s) => write!(fmt, "{:?}", s),
        }
    }
}

impl<T: Display> Display for FieldPattern<T> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}: {}", self.field_name, self.pattern)
    }
}

use std::convert::TryFrom;

use regex_syntax::hir::{Hir, HirKind, RepetitionKind};
use regex_syntax::ParserBuilder;

pub use regex_syntax::hir::{Class, ClassUnicode, Literal};

use crate::error::{Error, Result};

/// Middle Intermediate Representation of the regex, built from
/// `regex_syntax`'s `Hir`. The goal here is to strip and canonicalize
/// the tree, so that we don't have to do transformations later on the
/// graph, with the potential of running into looping references.
#[derive(Clone, Debug)]
pub enum Mir {
    Empty,
    Loop(Box<Mir>),
    Maybe(Box<Mir>),
    Concat(Vec<Mir>),
    Alternation(Vec<Mir>),
    Class(Class),
    Literal(Literal),
}

impl Mir {
    pub fn utf8(source: &str) -> Result<Mir> {
        Mir::try_from(ParserBuilder::new().build().parse(source)?)
    }

    pub fn utf8_ignore_case(source: &str) -> Result<Mir> {
        Mir::try_from(
            ParserBuilder::new()
                .case_insensitive(true)
                .build()
                .parse(source)?,
        )
    }

    pub fn binary(source: &str) -> Result<Mir> {
        Mir::try_from(
            ParserBuilder::new()
                .allow_invalid_utf8(true)
                .unicode(false)
                .build()
                .parse(source)?,
        )
    }

    pub fn binary_ignore_case(source: &str) -> Result<Mir> {
        Mir::try_from(
            ParserBuilder::new()
                .allow_invalid_utf8(true)
                .unicode(false)
                .case_insensitive(true)
                .build()
                .parse(source)?,
        )
    }

    pub fn priority(&self) -> usize {
        match self {
            Mir::Empty | Mir::Loop(_) | Mir::Maybe(_) => 0,
            Mir::Concat(concat) => concat.iter().map(Mir::priority).sum(),
            Mir::Alternation(alt) => alt.iter().map(Mir::priority).min().unwrap_or(0),
            Mir::Class(_) => 1,
            Mir::Literal(_) => 2,
        }
    }
}

impl TryFrom<Hir> for Mir {
    type Error = Error;

    fn try_from(hir: Hir) -> Result<Mir> {
        match hir.into_kind() {
            HirKind::Empty => Ok(Mir::Empty),
            HirKind::Concat(concat) => {
                let mut out = Vec::with_capacity(concat.len());

                fn extend(mir: Mir, out: &mut Vec<Mir>) {
                    match mir {
                        Mir::Concat(nested) => {
                            for child in nested {
                                extend(child, out);
                            }
                        }
                        mir => out.push(mir),
                    }
                }

                for hir in concat {
                    extend(Mir::try_from(hir)?, &mut out);
                }

                Ok(Mir::Concat(out))
            }
            HirKind::Alternation(alternation) => {
                let alternation = alternation
                    .into_iter()
                    .map(Mir::try_from)
                    .collect::<Result<_>>()?;

                Ok(Mir::Alternation(alternation))
            }
            HirKind::Literal(literal) => Ok(Mir::Literal(literal)),
            HirKind::Class(class) => Ok(Mir::Class(class)),
            HirKind::Repetition(repetition) => {
                if !repetition.greedy {
                    return Err("#[regex]: non-greedy parsing is currently unsupported.".into());
                }

                let kind = repetition.kind;
                let mir = Mir::try_from(*repetition.hir)?;

                match kind {
                    RepetitionKind::ZeroOrOne => Ok(Mir::Maybe(Box::new(mir))),
                    RepetitionKind::ZeroOrMore => Ok(Mir::Loop(Box::new(mir))),
                    RepetitionKind::OneOrMore => {
                        Ok(Mir::Concat(vec![mir.clone(), Mir::Loop(Box::new(mir))]))
                    }
                    RepetitionKind::Range(..) => {
                        Err("#[regex]: {n,m} repetition range is currently unsupported.".into())
                    }
                }
            }
            HirKind::Group(group) => Mir::try_from(*group.hir),
            HirKind::WordBoundary(_) => {
                Err("#[regex]: word boundaries are currently unsupported.".into())
            }
            HirKind::Anchor(_) => {
                Err("#[regex]: anchors in #[regex] are currently unsupported.".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mir;

    #[test]
    fn priorities() {
        let regexes = [
            ("[a-z]+", 1),
            ("a|b", 2),
            ("a|[b-z]", 1),
            ("(foo)+", 6),
            ("foobar", 12),
            ("(fooz|bar)+qux", 12),
        ];

        for (regex, expected) in regexes.iter() {
            let mir = Mir::utf8(regex).unwrap();
            assert_eq!(mir.priority(), *expected);
        }
    }
}

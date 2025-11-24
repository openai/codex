//! Normalization processes a parse tree until it is in suitable form to
//! be converted to the more canonical form. This is done as a series of
//! passes, each contained in their own module below.

use crate::grammar::parse_tree as pt;
use crate::grammar::repr as r;
use crate::session::Session;

pub type NormResult<T> = Result<T, NormError>;

#[derive(Clone, Debug)]
pub struct NormError {
    pub message: String,
    pub span: pt::Span,
}

macro_rules! return_err {
    ($span: expr, $($args:expr),+) => {
        return Err(NormError {
            message: format!($($args),+),
            span: $span
        })
    }
}

pub fn normalize(session: &Session, grammar: pt::Grammar) -> NormResult<r::Grammar> {
    normalize_helper(session, grammar, true)
}

/// for unit tests, it is convenient to skip the validation step, and supply a dummy session
#[cfg(test)]
pub fn normalize_without_validating(grammar: pt::Grammar) -> NormResult<r::Grammar> {
    normalize_helper(&Session::new(), grammar, false)
}

fn normalize_helper(
    session: &Session,
    grammar: pt::Grammar,
    validate: bool,
) -> NormResult<r::Grammar> {
    let grammar = lower_helper(session, grammar, validate)?;
    let grammar = profile!(session, "Inlining", inline::inline(grammar)?);
    Ok(grammar)
}

fn lower_helper(session: &Session, grammar: pt::Grammar, validate: bool) -> NormResult<r::Grammar> {
    profile!(
        session,
        "Grammar validation",
        if validate {
            prevalidate::validate(&grammar)?;
        }
    );
    let grammar = profile!(session, "Grammar resolution", resolve::resolve(grammar)?);
    let grammar = profile!(session, "Precedence expansion", precedence::expand_precedence(grammar)?);
    let grammar = profile!(
        session,
        "Macro expansion",
        macro_expand::expand_macros(grammar)?
    );
    let grammar = profile!(session, "Token check", token_check::validate(grammar)?);
    let types = profile!(session, "Infer types", tyinfer::infer_types(&grammar)?);
    let grammar = profile!(session, "Lowering", lower::lower(session, grammar, types)?);
    Ok(grammar)
}

// These are executed *IN ORDER*:

// Check most safety conditions.
mod prevalidate;

// Resolve identifiers into terminals/nonterminals etc.
mod resolve;

mod precedence;

// Expands macros and expressions
//
//     X = ...1 Comma<X> (X Y Z) ...2
//
// to
//
//     X = ...1 `Comma<X>` `(X Y Z)` ...2
//     `Comma_X`: Vec<<X>> = ...;
//     `(X Y Z)` = X Y Z;
//
// AFTER THIS POINT: No more macros, macro references, guarded
// alternatives, repeats, or expr symbols, though type indirections
// may occur.
mod macro_expand;

// Check if there is an extern token and all terminals have have a
// conversion; if no extern token, synthesize an intern token.
mod token_check;

// Computes types where the user omitted them (or from macro
// byproducts).
//
// AFTER THIS POINT: there is a separate `repr::Types` table
// providing all nonterminals with an explicit type.
mod tyinfer;

// Lowers the parse tree to the repr notation.
mod lower;

// Inline nonterminals that have requested it.
mod inline;

///////////////////////////////////////////////////////////////////////////
// Shared routines

mod norm_util;

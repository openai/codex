use std::cmp::{Ord, Ordering};
use std::fmt::{self, Debug};

use proc_macro2::{Span, TokenStream};
use syn::{spanned::Spanned, Ident};

use crate::graph::{Disambiguate, Node};
use crate::util::MaybeVoid;

#[derive(Clone)]
pub struct Leaf<'t> {
    pub ident: &'t Ident,
    pub span: Span,
    pub priority: usize,
    pub field: MaybeVoid,
    pub callback: Option<Callback>,
}

#[derive(Clone)]
pub enum Callback {
    Label(TokenStream),
    Inline(Box<InlineCallback>),
}

#[derive(Clone)]
pub struct InlineCallback {
    pub arg: Ident,
    pub body: TokenStream,
    pub span: Span,
}

impl From<InlineCallback> for Callback {
    fn from(inline: InlineCallback) -> Callback {
        Callback::Inline(Box::new(inline))
    }
}

impl Callback {
    pub fn span(&self) -> Span {
        match self {
            Callback::Label(tokens) => tokens.span(),
            Callback::Inline(inline) => inline.span,
        }
    }
}

impl<'t> Leaf<'t> {
    pub fn new(ident: &'t Ident, span: Span) -> Self {
        Leaf {
            ident,
            span,
            priority: 0,
            field: MaybeVoid::Void,
            callback: None,
        }
    }

    pub fn callback(mut self, callback: Option<Callback>) -> Self {
        self.callback = callback;
        self
    }

    pub fn field(mut self, field: MaybeVoid) -> Self {
        self.field = field;
        self
    }

    pub fn priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl Disambiguate for Leaf<'_> {
    fn cmp(left: &Leaf, right: &Leaf) -> Ordering {
        Ord::cmp(&left.priority, &right.priority)
    }
}

impl<'t> From<Leaf<'t>> for Node<Leaf<'t>> {
    fn from(leaf: Leaf<'t>) -> Self {
        Node::Leaf(leaf)
    }
}

impl Debug for Leaf<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "::{}", self.ident)?;

        match self.callback {
            Some(Callback::Label(ref label)) => write!(f, " ({})", label),
            Some(Callback::Inline(_)) => f.write_str(" (<inline>)"),
            None => Ok(()),
        }
    }
}

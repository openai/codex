use proc_macro2::TokenStream;
use quote::quote;

use crate::generator::Generator;
use crate::graph::NodeId;

/// This struct keeps track of bytes available to be read without
/// bounds checking across the tree.
///
/// For example, a branch that matches 4 bytes followed by a fork
/// with smallest branch containing of 2 bytes can do a bounds check
/// for 6 bytes ahead, and leave the remaining 2 byte array (fixed size)
/// to be handled by the fork, avoiding bound checks there.
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Context {
    /// Amount of bytes that haven't been bumped yet but should
    /// before a new read is performed
    at: usize,
    /// Number of bytes available without bound checks
    available: usize,
    /// Whether or not the Lexer has been bumped at least by 1 byte
    bumped: bool,
    /// Node to backtrack to to in case an explicit match has failed.
    /// If `None` will instead produce an error token.
    backtrack: Option<NodeId>,
}

impl Context {
    pub fn can_backtrack(&self) -> bool {
        self.backtrack.is_some()
    }

    pub fn switch(&mut self, miss: Option<NodeId>) -> Option<TokenStream> {
        self.backtrack = Some(miss?);
        self.bump()
    }

    pub const fn advance(self, n: usize) -> Self {
        Context {
            at: self.at + n,
            ..self
        }
    }

    pub fn bump(&mut self) -> Option<TokenStream> {
        match self.at {
            0 => None,
            n => {
                let tokens = quote!(lex.bump_unchecked(#n););
                self.at = 0;
                self.available = 0;
                self.bumped = true;
                Some(tokens)
            }
        }
    }

    pub fn remainder(&self) -> usize {
        self.available.saturating_sub(self.at)
    }

    pub fn read_unchecked(&mut self, len: usize) -> TokenStream {
        let at = self.at;

        match len {
            0 => {
                self.advance(1);

                quote!(lex.read_unchecked::<u8>(#at))
            }
            l => {
                self.advance(l);

                quote!(lex.read_unchecked::<&[u8; #l]>(#at))
            }
        }
    }

    pub fn read(&mut self, len: usize) -> TokenStream {
        self.available = len;

        match (self.at, len) {
            (0, 0) => quote!(lex.read::<u8>()),
            (a, 0) => quote!(lex.read_at::<u8>(#a)),
            (0, l) => quote!(lex.read::<&[u8; #l]>()),
            (a, l) => quote!(lex.read_at::<&[u8; #l]>(#a)),
        }
    }

    pub fn wipe(&mut self) {
        self.available = 0;
    }

    const fn backtrack(self) -> Self {
        Context {
            at: 0,
            available: 0,
            bumped: self.bumped,
            backtrack: None,
        }
    }

    pub fn miss(mut self, miss: Option<NodeId>, gen: &mut Generator) -> TokenStream {
        self.wipe();
        match (miss, self.backtrack) {
            (Some(id), _) => gen.goto(id, self).clone(),
            (_, Some(id)) => gen.goto(id, self.backtrack()).clone(),
            _ if self.bumped => quote!(lex.error()),
            _ => quote!(_error(lex)),
        }
    }

    pub fn write_suffix(&self, buf: &mut String) {
        use std::fmt::Write;

        if self.at > 0 {
            let _ = write!(buf, "_at{}", self.at);
        }
        if self.available > 0 {
            let _ = write!(buf, "_with{}", self.available);
        }
        if let Some(id) = self.backtrack {
            let _ = write!(buf, "_ctx{}", id);
        }
        if self.bumped {
            buf.push_str("_x");
        }
    }
}

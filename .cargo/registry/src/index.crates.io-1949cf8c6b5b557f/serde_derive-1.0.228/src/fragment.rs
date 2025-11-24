use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{token, Token};

pub enum Fragment {
    /// Tokens that can be used as an expression.
    Expr(TokenStream),
    /// Tokens that can be used inside a block. The surrounding curly braces are
    /// not part of these tokens.
    Block(TokenStream),
}

macro_rules! quote_expr {
    ($($tt:tt)*) => {
        $crate::fragment::Fragment::Expr(quote!($($tt)*))
    }
}

macro_rules! quote_block {
    ($($tt:tt)*) => {
        $crate::fragment::Fragment::Block(quote!($($tt)*))
    }
}

/// Interpolate a fragment in place of an expression. This involves surrounding
/// Block fragments in curly braces.
pub struct Expr(pub Fragment);
impl ToTokens for Expr {
    fn to_tokens(&self, out: &mut TokenStream) {
        match &self.0 {
            Fragment::Expr(expr) => expr.to_tokens(out),
            Fragment::Block(block) => {
                token::Brace::default().surround(out, |out| block.to_tokens(out));
            }
        }
    }
}

/// Interpolate a fragment as the statements of a block.
pub struct Stmts(pub Fragment);
impl ToTokens for Stmts {
    fn to_tokens(&self, out: &mut TokenStream) {
        match &self.0 {
            Fragment::Expr(expr) => expr.to_tokens(out),
            Fragment::Block(block) => block.to_tokens(out),
        }
    }
}

/// Interpolate a fragment as the value part of a `match` expression. This
/// involves putting a comma after expressions and curly braces around blocks.
pub struct Match(pub Fragment);
impl ToTokens for Match {
    fn to_tokens(&self, out: &mut TokenStream) {
        match &self.0 {
            Fragment::Expr(expr) => {
                expr.to_tokens(out);
                <Token![,]>::default().to_tokens(out);
            }
            Fragment::Block(block) => {
                token::Brace::default().surround(out, |out| block.to_tokens(out));
            }
        }
    }
}

impl AsRef<TokenStream> for Fragment {
    fn as_ref(&self) -> &TokenStream {
        match self {
            Fragment::Expr(expr) => expr,
            Fragment::Block(block) => block,
        }
    }
}

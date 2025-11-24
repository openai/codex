use quote::ToTokens;

use crate::{Error, FromMeta, Result};

/// Either a path or a closure.
///
/// This type is useful for options that historically took a path,
/// e.g. `#[darling(with = ...)]` or `#[serde(skip_serializing_if = ...)]`
/// and now want to also allow using a closure to avoid needing a separate
/// function declaration.
///
/// In `darling`, this value is wrapped in [`core::convert::identity`] before usage;
/// this allows treatment of the closure and path cases as equivalent, and prevents
/// a closure from accessing locals in the generated code.
#[derive(Debug, Clone)]
pub struct Callable {
    /// The callable
    call: syn::Expr,
}

impl AsRef<syn::Expr> for Callable {
    fn as_ref(&self) -> &syn::Expr {
        &self.call
    }
}

impl From<syn::ExprPath> for Callable {
    fn from(value: syn::ExprPath) -> Self {
        Self {
            call: syn::Expr::Path(value),
        }
    }
}

impl From<syn::ExprClosure> for Callable {
    fn from(value: syn::ExprClosure) -> Self {
        Self {
            call: syn::Expr::Closure(value),
        }
    }
}

impl From<Callable> for syn::Expr {
    fn from(value: Callable) -> Self {
        value.call
    }
}

impl FromMeta for Callable {
    fn from_expr(expr: &syn::Expr) -> Result<Self> {
        match expr {
            syn::Expr::Path(_) | syn::Expr::Closure(_) => Ok(Self { call: expr.clone() }),
            _ => Err(Error::unexpected_expr_type(expr)),
        }
    }
}

impl ToTokens for Callable {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.call.to_tokens(tokens);
    }
}

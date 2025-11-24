use quote::ToTokens;
use syn::{Expr, ExprClosure, ExprLit, ExprPath, Lit, Path};

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
    call: Expr,
}

impl AsRef<Expr> for Callable {
    fn as_ref(&self) -> &Expr {
        &self.call
    }
}

impl From<Path> for Callable {
    fn from(path: Path) -> Self {
        Self::from(ExprPath {
            attrs: vec![],
            qself: None,
            path,
        })
    }
}

impl From<ExprPath> for Callable {
    fn from(value: ExprPath) -> Self {
        Self {
            call: Expr::Path(value),
        }
    }
}

impl From<ExprClosure> for Callable {
    fn from(value: ExprClosure) -> Self {
        Self {
            call: Expr::Closure(value),
        }
    }
}

impl From<Callable> for Expr {
    fn from(value: Callable) -> Self {
        value.call
    }
}

impl FromMeta for Callable {
    fn from_expr(expr: &Expr) -> Result<Self> {
        match expr {
            Expr::Path(_) | Expr::Closure(_) => Ok(Self { call: expr.clone() }),
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => syn::parse_str::<Path>(&s.value())
                .map_err(|e| {
                    Error::custom(format!("`with` must be a path if it's a string: {}", e))
                        .with_span(s)
                })
                .map(Self::from),
            _ => Err(Error::unexpected_expr_type(expr)),
        }
    }
}

impl ToTokens for Callable {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.call.to_tokens(tokens);
    }
}

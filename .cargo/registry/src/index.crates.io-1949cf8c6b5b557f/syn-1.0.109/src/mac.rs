use super::*;
use crate::token::{Brace, Bracket, Paren};
use proc_macro2::TokenStream;
#[cfg(feature = "parsing")]
use proc_macro2::{Delimiter, Group, Span, TokenTree};

#[cfg(feature = "parsing")]
use crate::parse::{Parse, ParseStream, Parser, Result};

ast_struct! {
    /// A macro invocation: `println!("{}", mac)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Macro {
        pub path: Path,
        pub bang_token: Token![!],
        pub delimiter: MacroDelimiter,
        pub tokens: TokenStream,
    }
}

ast_enum! {
    /// A grouping token that surrounds a macro body: `m!(...)` or `m!{...}` or `m![...]`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum MacroDelimiter {
        Paren(Paren),
        Brace(Brace),
        Bracket(Bracket),
    }
}

#[cfg(feature = "parsing")]
fn delimiter_span_close(macro_delimiter: &MacroDelimiter) -> Span {
    let delimiter = match macro_delimiter {
        MacroDelimiter::Paren(_) => Delimiter::Parenthesis,
        MacroDelimiter::Brace(_) => Delimiter::Brace,
        MacroDelimiter::Bracket(_) => Delimiter::Bracket,
    };
    let mut group = Group::new(delimiter, TokenStream::new());
    group.set_span(match macro_delimiter {
        MacroDelimiter::Paren(token) => token.span,
        MacroDelimiter::Brace(token) => token.span,
        MacroDelimiter::Bracket(token) => token.span,
    });
    group.span_close()
}

impl Macro {
    /// Parse the tokens within the macro invocation's delimiters into a syntax
    /// tree.
    ///
    /// This is equivalent to `syn::parse2::<T>(mac.tokens)` except that it
    /// produces a more useful span when `tokens` is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use syn::{parse_quote, Expr, ExprLit, Ident, Lit, LitStr, Macro, Token};
    /// use syn::ext::IdentExt;
    /// use syn::parse::{Error, Parse, ParseStream, Result};
    /// use syn::punctuated::Punctuated;
    ///
    /// // The arguments expected by libcore's format_args macro, and as a
    /// // result most other formatting and printing macros like println.
    /// //
    /// //     println!("{} is {number:.prec$}", "x", prec=5, number=0.01)
    /// struct FormatArgs {
    ///     format_string: Expr,
    ///     positional_args: Vec<Expr>,
    ///     named_args: Vec<(Ident, Expr)>,
    /// }
    ///
    /// impl Parse for FormatArgs {
    ///     fn parse(input: ParseStream) -> Result<Self> {
    ///         let format_string: Expr;
    ///         let mut positional_args = Vec::new();
    ///         let mut named_args = Vec::new();
    ///
    ///         format_string = input.parse()?;
    ///         while !input.is_empty() {
    ///             input.parse::<Token![,]>()?;
    ///             if input.is_empty() {
    ///                 break;
    ///             }
    ///             if input.peek(Ident::peek_any) && input.peek2(Token![=]) {
    ///                 while !input.is_empty() {
    ///                     let name: Ident = input.call(Ident::parse_any)?;
    ///                     input.parse::<Token![=]>()?;
    ///                     let value: Expr = input.parse()?;
    ///                     named_args.push((name, value));
    ///                     if input.is_empty() {
    ///                         break;
    ///                     }
    ///                     input.parse::<Token![,]>()?;
    ///                 }
    ///                 break;
    ///             }
    ///             positional_args.push(input.parse()?);
    ///         }
    ///
    ///         Ok(FormatArgs {
    ///             format_string,
    ///             positional_args,
    ///             named_args,
    ///         })
    ///     }
    /// }
    ///
    /// // Extract the first argument, the format string literal, from an
    /// // invocation of a formatting or printing macro.
    /// fn get_format_string(m: &Macro) -> Result<LitStr> {
    ///     let args: FormatArgs = m.parse_body()?;
    ///     match args.format_string {
    ///         Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => Ok(lit),
    ///         other => {
    ///             // First argument was not a string literal expression.
    ///             // Maybe something like: println!(concat!(...), ...)
    ///             Err(Error::new_spanned(other, "format string must be a string literal"))
    ///         }
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let invocation = parse_quote! {
    ///         println!("{:?}", Instant::now())
    ///     };
    ///     let lit = get_format_string(&invocation).unwrap();
    ///     assert_eq!(lit.value(), "{:?}");
    /// }
    /// ```
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_body<T: Parse>(&self) -> Result<T> {
        self.parse_body_with(T::parse)
    }

    /// Parse the tokens within the macro invocation's delimiters using the
    /// given parser.
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_body_with<F: Parser>(&self, parser: F) -> Result<F::Output> {
        let scope = delimiter_span_close(&self.delimiter);
        crate::parse::parse_scoped(parser, scope, self.tokens.clone())
    }
}

#[cfg(feature = "parsing")]
pub fn parse_delimiter(input: ParseStream) -> Result<(MacroDelimiter, TokenStream)> {
    input.step(|cursor| {
        if let Some((TokenTree::Group(g), rest)) = cursor.token_tree() {
            let span = g.span();
            let delimiter = match g.delimiter() {
                Delimiter::Parenthesis => MacroDelimiter::Paren(Paren(span)),
                Delimiter::Brace => MacroDelimiter::Brace(Brace(span)),
                Delimiter::Bracket => MacroDelimiter::Bracket(Bracket(span)),
                Delimiter::None => {
                    return Err(cursor.error("expected delimiter"));
                }
            };
            Ok(((delimiter, g.stream()), rest))
        } else {
            Err(cursor.error("expected delimiter"))
        }
    })
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::parse::{Parse, ParseStream, Result};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Macro {
        fn parse(input: ParseStream) -> Result<Self> {
            let tokens;
            Ok(Macro {
                path: input.call(Path::parse_mod_style)?,
                bang_token: input.parse()?,
                delimiter: {
                    let (delimiter, content) = parse_delimiter(input)?;
                    tokens = content;
                    delimiter
                },
                tokens,
            })
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Macro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.path.to_tokens(tokens);
            self.bang_token.to_tokens(tokens);
            match &self.delimiter {
                MacroDelimiter::Paren(paren) => {
                    paren.surround(tokens, |tokens| self.tokens.to_tokens(tokens));
                }
                MacroDelimiter::Brace(brace) => {
                    brace.surround(tokens, |tokens| self.tokens.to_tokens(tokens));
                }
                MacroDelimiter::Bracket(bracket) => {
                    bracket.surround(tokens, |tokens| self.tokens.to_tokens(tokens));
                }
            }
        }
    }
}

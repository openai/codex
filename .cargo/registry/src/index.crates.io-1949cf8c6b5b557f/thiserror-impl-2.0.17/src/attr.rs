use proc_macro2::{Delimiter, Group, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use std::collections::BTreeSet as Set;
use syn::parse::discouraged::Speculative;
use syn::parse::{End, ParseStream};
use syn::{
    braced, bracketed, parenthesized, token, Attribute, Error, ExprPath, Ident, Index, LitFloat,
    LitInt, LitStr, Meta, Result, Token,
};

pub struct Attrs<'a> {
    pub display: Option<Display<'a>>,
    pub source: Option<Source<'a>>,
    pub backtrace: Option<&'a Attribute>,
    pub from: Option<From<'a>>,
    pub transparent: Option<Transparent<'a>>,
    pub fmt: Option<Fmt<'a>>,
}

#[derive(Clone)]
pub struct Display<'a> {
    pub original: &'a Attribute,
    pub fmt: LitStr,
    pub args: TokenStream,
    pub requires_fmt_machinery: bool,
    pub has_bonus_display: bool,
    pub infinite_recursive: bool,
    pub implied_bounds: Set<(usize, Trait)>,
    pub bindings: Vec<(Ident, TokenStream)>,
}

#[derive(Copy, Clone)]
pub struct Source<'a> {
    pub original: &'a Attribute,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct From<'a> {
    pub original: &'a Attribute,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct Transparent<'a> {
    pub original: &'a Attribute,
    pub span: Span,
}

#[derive(Clone)]
pub struct Fmt<'a> {
    pub original: &'a Attribute,
    pub path: ExprPath,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum Trait {
    Debug,
    Display,
    Octal,
    LowerHex,
    UpperHex,
    Pointer,
    Binary,
    LowerExp,
    UpperExp,
}

pub fn get(input: &[Attribute]) -> Result<Attrs> {
    let mut attrs = Attrs {
        display: None,
        source: None,
        backtrace: None,
        from: None,
        transparent: None,
        fmt: None,
    };

    for attr in input {
        if attr.path().is_ident("error") {
            parse_error_attribute(&mut attrs, attr)?;
        } else if attr.path().is_ident("source") {
            attr.meta.require_path_only()?;
            if attrs.source.is_some() {
                return Err(Error::new_spanned(attr, "duplicate #[source] attribute"));
            }
            let span = (attr.pound_token.span)
                .join(attr.bracket_token.span.join())
                .unwrap_or(attr.path().get_ident().unwrap().span());
            attrs.source = Some(Source {
                original: attr,
                span,
            });
        } else if attr.path().is_ident("backtrace") {
            attr.meta.require_path_only()?;
            if attrs.backtrace.is_some() {
                return Err(Error::new_spanned(attr, "duplicate #[backtrace] attribute"));
            }
            attrs.backtrace = Some(attr);
        } else if attr.path().is_ident("from") {
            match attr.meta {
                Meta::Path(_) => {}
                Meta::List(_) | Meta::NameValue(_) => {
                    // Assume this is meant for derive_more crate or something.
                    continue;
                }
            }
            if attrs.from.is_some() {
                return Err(Error::new_spanned(attr, "duplicate #[from] attribute"));
            }
            let span = (attr.pound_token.span)
                .join(attr.bracket_token.span.join())
                .unwrap_or(attr.path().get_ident().unwrap().span());
            attrs.from = Some(From {
                original: attr,
                span,
            });
        }
    }

    Ok(attrs)
}

fn parse_error_attribute<'a>(attrs: &mut Attrs<'a>, attr: &'a Attribute) -> Result<()> {
    mod kw {
        syn::custom_keyword!(transparent);
        syn::custom_keyword!(fmt);
    }

    attr.parse_args_with(|input: ParseStream| {
        let lookahead = input.lookahead1();
        let fmt = if lookahead.peek(LitStr) {
            input.parse::<LitStr>()?
        } else if lookahead.peek(kw::transparent) {
            let kw: kw::transparent = input.parse()?;
            if attrs.transparent.is_some() {
                return Err(Error::new_spanned(
                    attr,
                    "duplicate #[error(transparent)] attribute",
                ));
            }
            attrs.transparent = Some(Transparent {
                original: attr,
                span: kw.span,
            });
            return Ok(());
        } else if lookahead.peek(kw::fmt) {
            input.parse::<kw::fmt>()?;
            input.parse::<Token![=]>()?;
            let path: ExprPath = input.parse()?;
            if attrs.fmt.is_some() {
                return Err(Error::new_spanned(
                    attr,
                    "duplicate #[error(fmt = ...)] attribute",
                ));
            }
            attrs.fmt = Some(Fmt {
                original: attr,
                path,
            });
            return Ok(());
        } else {
            return Err(lookahead.error());
        };

        let args = if input.is_empty() || input.peek(Token![,]) && input.peek2(End) {
            input.parse::<Option<Token![,]>>()?;
            TokenStream::new()
        } else {
            parse_token_expr(input, false)?
        };

        let requires_fmt_machinery = !args.is_empty();

        let display = Display {
            original: attr,
            fmt,
            args,
            requires_fmt_machinery,
            has_bonus_display: false,
            infinite_recursive: false,
            implied_bounds: Set::new(),
            bindings: Vec::new(),
        };
        if attrs.display.is_some() {
            return Err(Error::new_spanned(
                attr,
                "only one #[error(...)] attribute is allowed",
            ));
        }
        attrs.display = Some(display);
        Ok(())
    })
}

fn parse_token_expr(input: ParseStream, mut begin_expr: bool) -> Result<TokenStream> {
    let mut tokens = Vec::new();
    while !input.is_empty() {
        if input.peek(token::Group) {
            let group: TokenTree = input.parse()?;
            tokens.push(group);
            begin_expr = false;
            continue;
        }

        if begin_expr && input.peek(Token![.]) {
            if input.peek2(Ident) {
                input.parse::<Token![.]>()?;
                begin_expr = false;
                continue;
            } else if input.peek2(LitInt) {
                input.parse::<Token![.]>()?;
                let int: Index = input.parse()?;
                tokens.push({
                    let ident = format_ident!("_{}", int.index, span = int.span);
                    TokenTree::Ident(ident)
                });
                begin_expr = false;
                continue;
            } else if input.peek2(LitFloat) {
                let ahead = input.fork();
                ahead.parse::<Token![.]>()?;
                let float: LitFloat = ahead.parse()?;
                let repr = float.to_string();
                let mut indices = repr.split('.').map(syn::parse_str::<Index>);
                if let (Some(Ok(first)), Some(Ok(second)), None) =
                    (indices.next(), indices.next(), indices.next())
                {
                    input.advance_to(&ahead);
                    tokens.push({
                        let ident = format_ident!("_{}", first, span = float.span());
                        TokenTree::Ident(ident)
                    });
                    tokens.push({
                        let mut punct = Punct::new('.', Spacing::Alone);
                        punct.set_span(float.span());
                        TokenTree::Punct(punct)
                    });
                    tokens.push({
                        let mut literal = Literal::u32_unsuffixed(second.index);
                        literal.set_span(float.span());
                        TokenTree::Literal(literal)
                    });
                    begin_expr = false;
                    continue;
                }
            }
        }

        begin_expr = input.peek(Token![break])
            || input.peek(Token![continue])
            || input.peek(Token![if])
            || input.peek(Token![in])
            || input.peek(Token![match])
            || input.peek(Token![mut])
            || input.peek(Token![return])
            || input.peek(Token![while])
            || input.peek(Token![+])
            || input.peek(Token![&])
            || input.peek(Token![!])
            || input.peek(Token![^])
            || input.peek(Token![,])
            || input.peek(Token![/])
            || input.peek(Token![=])
            || input.peek(Token![>])
            || input.peek(Token![<])
            || input.peek(Token![|])
            || input.peek(Token![%])
            || input.peek(Token![;])
            || input.peek(Token![*])
            || input.peek(Token![-]);

        let token: TokenTree = if input.peek(token::Paren) {
            let content;
            let delimiter = parenthesized!(content in input);
            let nested = parse_token_expr(&content, true)?;
            let mut group = Group::new(Delimiter::Parenthesis, nested);
            group.set_span(delimiter.span.join());
            TokenTree::Group(group)
        } else if input.peek(token::Brace) {
            let content;
            let delimiter = braced!(content in input);
            let nested = parse_token_expr(&content, true)?;
            let mut group = Group::new(Delimiter::Brace, nested);
            group.set_span(delimiter.span.join());
            TokenTree::Group(group)
        } else if input.peek(token::Bracket) {
            let content;
            let delimiter = bracketed!(content in input);
            let nested = parse_token_expr(&content, true)?;
            let mut group = Group::new(Delimiter::Bracket, nested);
            group.set_span(delimiter.span.join());
            TokenTree::Group(group)
        } else {
            input.parse()?
        };
        tokens.push(token);
    }
    Ok(TokenStream::from_iter(tokens))
}

impl ToTokens for Display<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.infinite_recursive {
            let span = self.fmt.span();
            tokens.extend(quote_spanned! {span=>
                #[warn(unconditional_recursion)]
                fn _fmt() { _fmt() }
            });
        }

        let fmt = &self.fmt;
        let args = &self.args;

        // Currently `write!(f, "text")` produces less efficient code than
        // `f.write_str("text")`. We recognize the case when the format string
        // has no braces and no interpolated values, and generate simpler code.
        let write = if self.requires_fmt_machinery {
            quote! {
                ::core::write!(__formatter, #fmt #args)
            }
        } else {
            quote! {
                __formatter.write_str(#fmt)
            }
        };

        tokens.extend(if self.bindings.is_empty() {
            write
        } else {
            let locals = self.bindings.iter().map(|(local, _value)| local);
            let values = self.bindings.iter().map(|(_local, value)| value);
            quote! {
                match (#(#values,)*) {
                    (#(#locals,)*) => #write
                }
            }
        });
    }
}

impl ToTokens for Trait {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let trait_name = match self {
            Trait::Debug => "Debug",
            Trait::Display => "Display",
            Trait::Octal => "Octal",
            Trait::LowerHex => "LowerHex",
            Trait::UpperHex => "UpperHex",
            Trait::Pointer => "Pointer",
            Trait::Binary => "Binary",
            Trait::LowerExp => "LowerExp",
            Trait::UpperExp => "UpperExp",
        };
        let ident = Ident::new(trait_name, Span::call_site());
        tokens.extend(quote!(::core::fmt::#ident));
    }
}

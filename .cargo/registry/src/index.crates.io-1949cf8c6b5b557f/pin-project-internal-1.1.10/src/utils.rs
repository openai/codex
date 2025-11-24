// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::mem;

use proc_macro2::{Group, Spacing, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote, quote_spanned};
use syn::{
    Attribute, ExprPath, ExprStruct, Generics, Ident, Item, Lifetime, LifetimeParam, Macro,
    PatStruct, PatTupleStruct, Path, PathArguments, PredicateType, QSelf, Result, Token, Type,
    TypeParamBound, TypePath, Variant, Visibility, WherePredicate,
    parse::{Parse, ParseBuffer, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    token,
    visit_mut::{self, VisitMut},
};

pub(crate) type Variants = Punctuated<Variant, Token![,]>;

macro_rules! parse_quote_spanned {
    ($span:expr => $($tt:tt)*) => {
        syn::parse2(quote::quote_spanned!($span => $($tt)*)).unwrap_or_else(|e| panic!("{}", e))
    };
}

/// Determines the lifetime names. Ensure it doesn't overlap with any existing
/// lifetime names.
pub(crate) fn determine_lifetime_name(lifetime_name: &mut String, generics: &mut Generics) {
    struct CollectLifetimes(Vec<String>);

    impl VisitMut for CollectLifetimes {
        fn visit_lifetime_param_mut(&mut self, def: &mut LifetimeParam) {
            self.0.push(def.lifetime.to_string());
        }
    }

    debug_assert!(lifetime_name.starts_with('\''));

    let mut lifetimes = CollectLifetimes(vec![]);
    lifetimes.visit_generics_mut(generics);

    while lifetimes.0.iter().any(|name| name.starts_with(&**lifetime_name)) {
        lifetime_name.push('_');
    }
}

/// Like `insert_lifetime`, but also generates a bound of the form
/// `OriginalType<A, B>: 'lifetime`. Used when generating the definition
/// of a projection type
pub(crate) fn insert_lifetime_and_bound(
    generics: &mut Generics,
    lifetime: Lifetime,
    orig_generics: &Generics,
    orig_ident: &Ident,
) -> WherePredicate {
    insert_lifetime(generics, lifetime.clone());

    let orig_type: Type = parse_quote!(#orig_ident #orig_generics);
    let mut punct = Punctuated::new();
    punct.push(TypeParamBound::Lifetime(lifetime));

    WherePredicate::Type(PredicateType {
        lifetimes: None,
        bounded_ty: orig_type,
        colon_token: <Token![:]>::default(),
        bounds: punct,
    })
}

/// Inserts a `lifetime` at position `0` of `generics.params`.
pub(crate) fn insert_lifetime(generics: &mut Generics, lifetime: Lifetime) {
    generics.lt_token.get_or_insert_with(<Token![<]>::default);
    generics.gt_token.get_or_insert_with(<Token![>]>::default);
    generics.params.insert(0, LifetimeParam::new(lifetime).into());
}

/// Determines the visibility of the projected types and projection methods.
///
/// If given visibility is `pub`, returned visibility is `pub(crate)`.
/// Otherwise, returned visibility is the same as given visibility.
pub(crate) fn determine_visibility(vis: &Visibility) -> Visibility {
    if let Visibility::Public(token) = vis {
        parse_quote_spanned!(token.span => pub(crate))
    } else {
        vis.clone()
    }
}

pub(crate) fn respan<T>(node: &T, span: Span) -> T
where
    T: ToTokens + Parse,
{
    let tokens = node.to_token_stream();
    let respanned = respan_tokens(tokens, span);
    syn::parse2(respanned).unwrap()
}

fn respan_tokens(tokens: TokenStream, span: Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|mut token| {
            token.set_span(span);
            token
        })
        .collect()
}

// -----------------------------------------------------------------------------
// extension traits

pub(crate) trait SliceExt {
    fn position_exact(&self, ident: &str) -> Result<Option<usize>>;
    fn find(&self, ident: &str) -> Option<&Attribute>;
}

impl SliceExt for [Attribute] {
    /// # Errors
    ///
    /// - There are multiple specified attributes.
    /// - The `Attribute::tokens` field of the specified attribute is not empty.
    fn position_exact(&self, ident: &str) -> Result<Option<usize>> {
        self.iter()
            .try_fold((0, None), |(i, mut prev), attr| {
                if attr.path().is_ident(ident) {
                    if prev.replace(i).is_some() {
                        bail!(attr, "duplicate #[{}] attribute", ident);
                    }
                    attr.meta.require_path_only()?;
                }
                Ok((i + 1, prev))
            })
            .map(|(_, pos)| pos)
    }

    fn find(&self, ident: &str) -> Option<&Attribute> {
        self.iter().position(|attr| attr.path().is_ident(ident)).map(|i| &self[i])
    }
}

pub(crate) trait ParseBufferExt<'a> {
    fn parenthesized(self) -> Result<ParseBuffer<'a>>;
}

impl<'a> ParseBufferExt<'a> for ParseStream<'a> {
    fn parenthesized(self) -> Result<ParseBuffer<'a>> {
        let content;
        let _: token::Paren = syn::parenthesized!(content in self);
        Ok(content)
    }
}

impl<'a> ParseBufferExt<'a> for ParseBuffer<'a> {
    fn parenthesized(self) -> Result<ParseBuffer<'a>> {
        let content;
        let _: token::Paren = syn::parenthesized!(content in self);
        Ok(content)
    }
}

// -----------------------------------------------------------------------------
// visitors

// Replace `self`/`Self` with `__self`/`self_ty`.
// Based on:
// - https://github.com/dtolnay/async-trait/blob/0.1.35/src/receiver.rs
// - https://github.com/dtolnay/async-trait/commit/6029cbf375c562ca98fa5748e9d950a8ff93b0e7

pub(crate) struct ReplaceReceiver<'a>(pub(crate) &'a TypePath);

impl ReplaceReceiver<'_> {
    fn self_ty(&self, span: Span) -> TypePath {
        respan(self.0, span)
    }

    fn self_to_qself(&self, qself: &mut Option<QSelf>, path: &mut Path) {
        if path.leading_colon.is_some() {
            return;
        }

        let first = &path.segments[0];
        if first.ident != "Self" || !first.arguments.is_empty() {
            return;
        }

        if path.segments.len() == 1 {
            self.self_to_expr_path(path);
            return;
        }

        let span = first.ident.span();
        *qself = Some(QSelf {
            lt_token: Token![<](span),
            ty: Box::new(self.self_ty(span).into()),
            position: 0,
            as_token: None,
            gt_token: Token![>](span),
        });

        path.leading_colon = Some(**path.segments.pairs().next().unwrap().punct().unwrap());

        let segments = mem::take(&mut path.segments);
        path.segments = segments.into_pairs().skip(1).collect();
    }

    fn self_to_expr_path(&self, path: &mut Path) {
        if path.leading_colon.is_some() {
            return;
        }

        let first = &path.segments[0];
        if first.ident != "Self" || !first.arguments.is_empty() {
            return;
        }

        let self_ty = self.self_ty(first.ident.span());
        let variant = mem::replace(path, self_ty.path);
        for segment in &mut path.segments {
            if let PathArguments::AngleBracketed(bracketed) = &mut segment.arguments {
                if bracketed.colon2_token.is_none() && !bracketed.args.is_empty() {
                    bracketed.colon2_token = Some(<Token![::]>::default());
                }
            }
        }
        if variant.segments.len() > 1 {
            path.segments.push_punct(<Token![::]>::default());
            path.segments.extend(variant.segments.into_pairs().skip(1));
        }
    }

    fn visit_token_stream(&self, tokens: &mut TokenStream) -> bool {
        let mut out = vec![];
        let mut modified = false;
        let mut iter = tokens.clone().into_iter().peekable();
        while let Some(tt) = iter.next() {
            match tt {
                TokenTree::Ident(mut ident) => {
                    modified |= prepend_underscore_to_self(&mut ident);
                    if ident == "Self" {
                        modified = true;
                        let self_ty = self.self_ty(ident.span());
                        match iter.peek() {
                            Some(TokenTree::Punct(p))
                                if p.as_char() == ':' && p.spacing() == Spacing::Joint =>
                            {
                                let next = iter.next().unwrap();
                                match iter.peek() {
                                    Some(TokenTree::Punct(p)) if p.as_char() == ':' => {
                                        let span = ident.span();
                                        out.extend(quote_spanned!(span=> <#self_ty>));
                                    }
                                    _ => out.extend(quote!(#self_ty)),
                                }
                                out.push(next);
                            }
                            _ => out.extend(quote!(#self_ty)),
                        }
                    } else {
                        out.push(TokenTree::Ident(ident));
                    }
                }
                TokenTree::Group(group) => {
                    let mut content = group.stream();
                    modified |= self.visit_token_stream(&mut content);
                    let mut new = Group::new(group.delimiter(), content);
                    new.set_span(group.span());
                    out.push(TokenTree::Group(new));
                }
                other => out.push(other),
            }
        }
        if modified {
            *tokens = TokenStream::from_iter(out);
        }
        modified
    }
}

impl VisitMut for ReplaceReceiver<'_> {
    // `Self` -> `Receiver`
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(node) = ty {
            if node.qself.is_none() && node.path.is_ident("Self") {
                *ty = self.self_ty(node.path.segments[0].ident.span()).into();
            } else {
                self.visit_type_path_mut(node);
            }
        } else {
            visit_mut::visit_type_mut(self, ty);
        }
    }

    // `Self::Assoc` -> `<Receiver>::Assoc`
    fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
        if ty.qself.is_none() {
            self.self_to_qself(&mut ty.qself, &mut ty.path);
        }
        visit_mut::visit_type_path_mut(self, ty);
    }

    // `Self::method` -> `<Receiver>::method`
    fn visit_expr_path_mut(&mut self, expr: &mut ExprPath) {
        if expr.qself.is_none() {
            self.self_to_qself(&mut expr.qself, &mut expr.path);
        }
        visit_mut::visit_expr_path_mut(self, expr);
    }

    fn visit_expr_struct_mut(&mut self, expr: &mut ExprStruct) {
        self.self_to_expr_path(&mut expr.path);
        visit_mut::visit_expr_struct_mut(self, expr);
    }

    fn visit_pat_struct_mut(&mut self, pat: &mut PatStruct) {
        self.self_to_expr_path(&mut pat.path);
        visit_mut::visit_pat_struct_mut(self, pat);
    }

    fn visit_pat_tuple_struct_mut(&mut self, pat: &mut PatTupleStruct) {
        self.self_to_expr_path(&mut pat.path);
        visit_mut::visit_pat_tuple_struct_mut(self, pat);
    }

    fn visit_path_mut(&mut self, path: &mut Path) {
        if path.segments.len() == 1 {
            // Replace `self`, but not `self::function`.
            prepend_underscore_to_self(&mut path.segments[0].ident);
        }
        for segment in &mut path.segments {
            self.visit_path_arguments_mut(&mut segment.arguments);
        }
    }

    fn visit_item_mut(&mut self, item: &mut Item) {
        match item {
            // Visit `macro_rules!` because locally defined macros can refer to `self`.
            Item::Macro(item) if item.mac.path.is_ident("macro_rules") => {
                self.visit_macro_mut(&mut item.mac);
            }
            // Otherwise, do not recurse into nested items.
            _ => {}
        }
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        // We can't tell in general whether `self` inside a macro invocation
        // refers to the self in the argument list or a different self
        // introduced within the macro. Heuristic: if the macro input contains
        // `fn`, then `self` is more likely to refer to something other than the
        // outer function's self argument.
        if !contains_fn(mac.tokens.clone()) {
            self.visit_token_stream(&mut mac.tokens);
        }
    }
}

fn contains_fn(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "fn",
        TokenTree::Group(group) => contains_fn(group.stream()),
        _ => false,
    })
}

pub(crate) fn prepend_underscore_to_self(ident: &mut Ident) -> bool {
    let modified = ident == "self";
    if modified {
        *ident = Ident::new("__self", ident.span());
    }
    modified
}

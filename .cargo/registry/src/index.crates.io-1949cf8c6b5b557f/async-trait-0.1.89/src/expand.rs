use crate::bound::{has_bound, InferredBound, Supertraits};
use crate::lifetime::{AddLifetimeToImplTrait, CollectLifetimes};
use crate::parse::Item;
use crate::receiver::{has_self_in_block, has_self_in_sig, mut_pat, ReplaceSelf};
use crate::verbatim::VerbatimFn;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use std::collections::BTreeSet as Set;
use std::mem;
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use syn::{
    parse_quote, parse_quote_spanned, Attribute, Block, FnArg, GenericArgument, GenericParam,
    Generics, Ident, ImplItem, Lifetime, LifetimeParam, Pat, PatIdent, PathArguments, Receiver,
    ReturnType, Signature, Token, TraitItem, Type, TypeInfer, TypePath, WhereClause,
};

impl ToTokens for Item {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Item::Trait(item) => item.to_tokens(tokens),
            Item::Impl(item) => item.to_tokens(tokens),
        }
    }
}

#[derive(Clone, Copy)]
enum Context<'a> {
    Trait {
        generics: &'a Generics,
        supertraits: &'a Supertraits,
    },
    Impl {
        impl_generics: &'a Generics,
        associated_type_impl_traits: &'a Set<Ident>,
    },
}

impl Context<'_> {
    fn lifetimes<'a>(&'a self, used: &'a [Lifetime]) -> impl Iterator<Item = &'a LifetimeParam> {
        let generics = match self {
            Context::Trait { generics, .. } => generics,
            Context::Impl { impl_generics, .. } => impl_generics,
        };
        generics.params.iter().filter_map(move |param| {
            if let GenericParam::Lifetime(param) = param {
                if used.contains(&param.lifetime) {
                    return Some(param);
                }
            }
            None
        })
    }
}

pub fn expand(input: &mut Item, is_local: bool) {
    match input {
        Item::Trait(input) => {
            let context = Context::Trait {
                generics: &input.generics,
                supertraits: &input.supertraits,
            };
            for inner in &mut input.items {
                if let TraitItem::Fn(method) = inner {
                    let sig = &mut method.sig;
                    if sig.asyncness.is_some() {
                        let block = &mut method.default;
                        let mut has_self = has_self_in_sig(sig);
                        method.attrs.push(parse_quote!(#[must_use]));
                        if let Some(block) = block {
                            has_self |= has_self_in_block(block);
                            transform_block(context, sig, block);
                            method.attrs.push(lint_suppress_with_body());
                        } else {
                            method.attrs.push(lint_suppress_without_body());
                        }
                        let has_default = method.default.is_some();
                        transform_sig(context, sig, has_self, has_default, is_local);
                    }
                }
            }
        }
        Item::Impl(input) => {
            let mut associated_type_impl_traits = Set::new();
            for inner in &input.items {
                if let ImplItem::Type(assoc) = inner {
                    if let Type::ImplTrait(_) = assoc.ty {
                        associated_type_impl_traits.insert(assoc.ident.clone());
                    }
                }
            }

            let context = Context::Impl {
                impl_generics: &input.generics,
                associated_type_impl_traits: &associated_type_impl_traits,
            };
            for inner in &mut input.items {
                match inner {
                    ImplItem::Fn(method) if method.sig.asyncness.is_some() => {
                        let sig = &mut method.sig;
                        let block = &mut method.block;
                        let has_self = has_self_in_sig(sig);
                        transform_block(context, sig, block);
                        transform_sig(context, sig, has_self, false, is_local);
                        method.attrs.push(lint_suppress_with_body());
                    }
                    ImplItem::Verbatim(tokens) => {
                        let mut method = match syn::parse2::<VerbatimFn>(tokens.clone()) {
                            Ok(method) if method.sig.asyncness.is_some() => method,
                            _ => continue,
                        };
                        let sig = &mut method.sig;
                        let has_self = has_self_in_sig(sig);
                        transform_sig(context, sig, has_self, false, is_local);
                        method.attrs.push(lint_suppress_with_body());
                        *tokens = quote!(#method);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn lint_suppress_with_body() -> Attribute {
    parse_quote! {
        #[allow(
            elided_named_lifetimes,
            clippy::async_yields_async,
            clippy::diverging_sub_expression,
            clippy::let_unit_value,
            clippy::needless_arbitrary_self_type,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
    }
}

fn lint_suppress_without_body() -> Attribute {
    parse_quote! {
        #[allow(
            elided_named_lifetimes,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds
        )]
    }
}

// Input:
//     async fn f<T>(&self, x: &T) -> Ret;
//
// Output:
//     fn f<'life0, 'life1, 'async_trait, T>(
//         &'life0 self,
//         x: &'life1 T,
//     ) -> Pin<Box<dyn Future<Output = Ret> + Send + 'async_trait>>
//     where
//         'life0: 'async_trait,
//         'life1: 'async_trait,
//         T: 'async_trait,
//         Self: Sync + 'async_trait;
fn transform_sig(
    context: Context,
    sig: &mut Signature,
    has_self: bool,
    has_default: bool,
    is_local: bool,
) {
    sig.fn_token.span = sig.asyncness.take().unwrap().span;

    let (ret_arrow, ret) = match &sig.output {
        ReturnType::Default => (quote!(->), quote!(())),
        ReturnType::Type(arrow, ret) => (quote!(#arrow), quote!(#ret)),
    };

    let mut lifetimes = CollectLifetimes::new();
    for arg in &mut sig.inputs {
        match arg {
            FnArg::Receiver(arg) => lifetimes.visit_receiver_mut(arg),
            FnArg::Typed(arg) => lifetimes.visit_type_mut(&mut arg.ty),
        }
    }

    for param in &mut sig.generics.params {
        match param {
            GenericParam::Type(param) => {
                let param_name = &param.ident;
                let span = match param.colon_token.take() {
                    Some(colon_token) => colon_token.span,
                    None => param_name.span(),
                };
                if param.attrs.is_empty() {
                    let bounds = mem::take(&mut param.bounds);
                    where_clause_or_default(&mut sig.generics.where_clause)
                        .predicates
                        .push(parse_quote_spanned!(span=> #param_name: 'async_trait + #bounds));
                } else {
                    param.bounds.push(parse_quote!('async_trait));
                }
            }
            GenericParam::Lifetime(param) => {
                let param_name = &param.lifetime;
                let span = match param.colon_token.take() {
                    Some(colon_token) => colon_token.span,
                    None => param_name.span(),
                };
                if param.attrs.is_empty() {
                    let bounds = mem::take(&mut param.bounds);
                    where_clause_or_default(&mut sig.generics.where_clause)
                        .predicates
                        .push(parse_quote_spanned!(span=> #param: 'async_trait + #bounds));
                } else {
                    param.bounds.push(parse_quote!('async_trait));
                }
            }
            GenericParam::Const(_) => {}
        }
    }

    for param in context.lifetimes(&lifetimes.explicit) {
        let param = &param.lifetime;
        let span = param.span();
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote_spanned!(span=> #param: 'async_trait));
    }

    if sig.generics.lt_token.is_none() {
        sig.generics.lt_token = Some(Token![<](sig.ident.span()));
    }
    if sig.generics.gt_token.is_none() {
        sig.generics.gt_token = Some(Token![>](sig.paren_token.span.join()));
    }

    for elided in lifetimes.elided {
        sig.generics.params.push(parse_quote!(#elided));
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote_spanned!(elided.span()=> #elided: 'async_trait));
    }

    sig.generics.params.push(parse_quote!('async_trait));

    if has_self {
        let bounds: &[InferredBound] = if is_local {
            &[]
        } else if let Some(receiver) = sig.receiver() {
            match receiver.ty.as_ref() {
                // self: &Self
                Type::Reference(ty) if ty.mutability.is_none() => &[InferredBound::Sync],
                // self: Arc<Self>
                Type::Path(ty)
                    if {
                        let segment = ty.path.segments.last().unwrap();
                        segment.ident == "Arc"
                            && match &segment.arguments {
                                PathArguments::AngleBracketed(arguments) => {
                                    arguments.args.len() == 1
                                        && match &arguments.args[0] {
                                            GenericArgument::Type(Type::Path(arg)) => {
                                                arg.path.is_ident("Self")
                                            }
                                            _ => false,
                                        }
                                }
                                _ => false,
                            }
                    } =>
                {
                    &[InferredBound::Sync, InferredBound::Send]
                }
                _ => &[InferredBound::Send],
            }
        } else {
            &[InferredBound::Send]
        };

        let bounds = bounds.iter().filter(|bound| match context {
            Context::Trait { supertraits, .. } => has_default && !has_bound(supertraits, bound),
            Context::Impl { .. } => false,
        });

        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote! {
                Self: #(#bounds +)* 'async_trait
            });
    }

    for (i, arg) in sig.inputs.iter_mut().enumerate() {
        match arg {
            FnArg::Receiver(receiver) => {
                if receiver.reference.is_none() {
                    receiver.mutability = None;
                }
            }
            FnArg::Typed(arg) => {
                if match *arg.ty {
                    Type::Reference(_) => false,
                    _ => true,
                } {
                    if let Pat::Ident(pat) = &mut *arg.pat {
                        pat.by_ref = None;
                        pat.mutability = None;
                    } else {
                        let positional = positional_arg(i, &arg.pat);
                        let m = mut_pat(&mut arg.pat);
                        arg.pat = parse_quote!(#m #positional);
                    }
                }
                AddLifetimeToImplTrait.visit_type_mut(&mut arg.ty);
            }
        }
    }

    let bounds = if is_local {
        quote!('async_trait)
    } else {
        quote!(::core::marker::Send + 'async_trait)
    };
    sig.output = parse_quote! {
        #ret_arrow ::core::pin::Pin<Box<
            dyn ::core::future::Future<Output = #ret> + #bounds
        >>
    };
}

// Input:
//     async fn f<T>(&self, x: &T, (a, b): (A, B)) -> Ret {
//         self + x + a + b
//     }
//
// Output:
//     Box::pin(async move {
//         let ___ret: Ret = {
//             let __self = self;
//             let x = x;
//             let (a, b) = __arg1;
//
//             __self + x + a + b
//         };
//
//         ___ret
//     })
fn transform_block(context: Context, sig: &mut Signature, block: &mut Block) {
    let mut replace_self = false;
    let decls = sig
        .inputs
        .iter()
        .enumerate()
        .map(|(i, arg)| match arg {
            FnArg::Receiver(Receiver {
                self_token,
                mutability,
                ..
            }) => {
                replace_self = true;
                let ident = Ident::new("__self", self_token.span);
                quote!(let #mutability #ident = #self_token;)
            }
            FnArg::Typed(arg) => {
                // If there is a #[cfg(...)] attribute that selectively enables
                // the parameter, forward it to the variable.
                //
                // This is currently not applied to the `self` parameter.
                let attrs = arg.attrs.iter().filter(|attr| attr.path().is_ident("cfg"));

                if let Type::Reference(_) = *arg.ty {
                    quote!()
                } else if let Pat::Ident(PatIdent {
                    ident, mutability, ..
                }) = &*arg.pat
                {
                    quote! {
                        #(#attrs)*
                        let #mutability #ident = #ident;
                    }
                } else {
                    let pat = &arg.pat;
                    let ident = positional_arg(i, pat);
                    if let Pat::Wild(_) = **pat {
                        quote! {
                            #(#attrs)*
                            let #ident = #ident;
                        }
                    } else {
                        quote! {
                            #(#attrs)*
                            let #pat = {
                                let #ident = #ident;
                                #ident
                            };
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    if replace_self {
        ReplaceSelf.visit_block_mut(block);
    }

    let let_ret = match &mut sig.output {
        ReturnType::Default => quote! {
            #(#decls)*
            let _: () = #block;
        },
        ReturnType::Type(_, ret) => {
            if contains_associated_type_impl_trait(context, ret) {
                if decls.is_empty() {
                    let stmts = &block.stmts;
                    quote!(#(#stmts)*)
                } else {
                    quote!(#(#decls)* #block)
                }
            } else {
                let mut ret = ret.clone();
                replace_impl_trait_with_infer(&mut ret);
                quote! {
                    if let ::core::option::Option::Some(__ret) = ::core::option::Option::None::<#ret> {
                        #[allow(unreachable_code)]
                        return __ret;
                    }
                    #(#decls)*
                    let __ret: #ret = #block;
                    #[allow(unreachable_code)]
                    __ret
                }
            }
        }
    };
    let box_pin = quote_spanned!(sig.asyncness.unwrap().span=>
        Box::pin(async move { #let_ret })
    );
    block.stmts = parse_quote!(#box_pin);
}

fn positional_arg(i: usize, pat: &Pat) -> Ident {
    let span = syn::spanned::Spanned::span(pat).resolved_at(Span::mixed_site());
    format_ident!("__arg{}", i, span = span)
}

fn contains_associated_type_impl_trait(context: Context, ret: &mut Type) -> bool {
    struct AssociatedTypeImplTraits<'a> {
        set: &'a Set<Ident>,
        contains: bool,
    }

    impl<'a> VisitMut for AssociatedTypeImplTraits<'a> {
        fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
            if ty.qself.is_none()
                && ty.path.segments.len() == 2
                && ty.path.segments[0].ident == "Self"
                && self.set.contains(&ty.path.segments[1].ident)
            {
                self.contains = true;
            }
            visit_mut::visit_type_path_mut(self, ty);
        }
    }

    match context {
        Context::Trait { .. } => false,
        Context::Impl {
            associated_type_impl_traits,
            ..
        } => {
            let mut visit = AssociatedTypeImplTraits {
                set: associated_type_impl_traits,
                contains: false,
            };
            visit.visit_type_mut(ret);
            visit.contains
        }
    }
}

fn where_clause_or_default(clause: &mut Option<WhereClause>) -> &mut WhereClause {
    clause.get_or_insert_with(|| WhereClause {
        where_token: Default::default(),
        predicates: Punctuated::new(),
    })
}

fn replace_impl_trait_with_infer(ty: &mut Type) {
    struct ReplaceImplTraitWithInfer;

    impl VisitMut for ReplaceImplTraitWithInfer {
        fn visit_type_mut(&mut self, ty: &mut Type) {
            if let Type::ImplTrait(impl_trait) = ty {
                *ty = Type::Infer(TypeInfer {
                    underscore_token: Token![_](impl_trait.impl_token.span),
                });
            }
            visit_mut::visit_type_mut(self, ty);
        }
    }

    ReplaceImplTraitWithInfer.visit_type_mut(ty);
}

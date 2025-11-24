// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Custom derives for `ZeroFrom` from the `zerofrom` crate.

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::exhaustive_structs,
        clippy::exhaustive_enums,
        clippy::trivially_copy_pass_by_ref,
        missing_debug_implementations,
    )
)]

use core::mem;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::collections::{HashMap, HashSet};
use syn::fold::{self, Fold};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, DeriveInput, Ident, Lifetime, MetaList, Token,
    TraitBoundModifier, Type, TypeParamBound, TypePath, WherePredicate,
};
use synstructure::Structure;
mod visitor;

/// Custom derive for `zerofrom::ZeroFrom`,
///
/// This implements `ZeroFrom<Ty> for Ty` for types
/// without a lifetime parameter, and `ZeroFrom<Ty<'data>> for Ty<'static>`
/// for types with a lifetime parameter.
///
/// Apply the `#[zerofrom(clone)]` attribute to a field if it doesn't implement
/// Copy or ZeroFrom; this data will be cloned when the struct is zero_from'ed.
///
/// Apply the `#[zerofrom(maybe_borrow(T, U, V))]` attribute to the struct to indicate
/// that certain type parameters may themselves contain borrows (by default
/// the derives assume that type parameters perform no borrows and can be copied or cloned).
///
/// In rust versions where [this issue](https://github.com/rust-lang/rust/issues/114393) is fixed,
/// `#[zerofrom(may_borrow)]` can be applied directly to type parameters.
#[proc_macro_derive(ZeroFrom, attributes(zerofrom))]
pub fn zf_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(zf_derive_impl(&input))
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|a| {
        if let Ok(i) = a.parse_args::<Ident>() {
            if i == name {
                return true;
            }
        }
        false
    })
}

// Collects all idents from #[zerofrom(may_borrow(A, B, C, D))]
// needed since #[zerofrom(may_borrow)] doesn't work yet
// (https://github.com/rust-lang/rust/issues/114393)
fn get_may_borrow_attr(attrs: &[syn::Attribute]) -> Result<HashSet<Ident>, Span> {
    let mut params = HashSet::new();
    for attr in attrs {
        if let Ok(list) = attr.parse_args::<MetaList>() {
            if list.path.is_ident("may_borrow") {
                if let Ok(list) =
                    list.parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
                {
                    params.extend(list)
                } else {
                    return Err(attr.span());
                }
            }
        }
    }
    Ok(params)
}

fn zf_derive_impl(input: &DeriveInput) -> TokenStream2 {
    let mut tybounds = input
        .generics
        .type_params()
        .map(|ty| {
            // Strip out param defaults, we don't need them in the impl
            let mut ty = ty.clone();
            ty.eq_token = None;
            ty.default = None;
            ty
        })
        .collect::<Vec<_>>();
    let typarams = tybounds
        .iter()
        .map(|ty| ty.ident.clone())
        .collect::<Vec<_>>();
    let lts = input.generics.lifetimes().count();
    let name = &input.ident;
    let structure = Structure::new(input);

    let may_borrow_attrs = match get_may_borrow_attr(&input.attrs) {
        Ok(mb) => mb,
        Err(span) => {
            return syn::Error::new(
            span,
            "#[zerofrom(may_borrow)] on the struct takes in a comma separated list of type parameters, like so: `#[zerofrom(may_borrow(A, B, C, D)]`",
        ).to_compile_error();
        }
    };

    // This contains every generic type introduced in this code.
    // If the gneeric type is may_borrow, this additionally contains the identifier corresponding to
    // a newly introduced mirror type parameter that we are borrowing from, similar to C in the original trait.
    // For convenience, we are calling these "C types"
    let generics_env: HashMap<Ident, Option<Ident>> = tybounds
        .iter_mut()
        .map(|param| {
            // First one doesn't work yet https://github.com/rust-lang/rust/issues/114393
            let maybe_new_param = if has_attr(&param.attrs, "may_borrow")
                || may_borrow_attrs.contains(&param.ident)
            {
                // Remove `?Sized`` bound because we need a param to be Sized in order to take a ZeroFrom of it.
                // This only applies to fields marked as `may_borrow`.
                let mut bounds = core::mem::take(&mut param.bounds);
                while let Some(bound_pair) = bounds.pop() {
                    let bound = bound_pair.into_value();
                    if let TypeParamBound::Trait(ref trait_bound) = bound {
                        if trait_bound.path.get_ident().map(|ident| ident == "Sized") == Some(true)
                            && matches!(trait_bound.modifier, TraitBoundModifier::Maybe(_))
                        {
                            continue;
                        }
                    }
                    param.bounds.push(bound);
                }
                Some(Ident::new(
                    &format!("{}ZFParamC", param.ident),
                    param.ident.span(),
                ))
            } else {
                None
            };
            (param.ident.clone(), maybe_new_param)
        })
        .collect();

    // Do any of the generics potentially borrow?
    let generics_may_borrow = generics_env.values().any(|x| x.is_some());

    if lts == 0 && !generics_may_borrow {
        let has_clone = structure
            .variants()
            .iter()
            .flat_map(|variant| variant.bindings().iter())
            .any(|binding| has_attr(&binding.ast().attrs, "clone"));
        let (clone, clone_trait) = if has_clone {
            (quote!(this.clone()), quote!(Clone))
        } else {
            (quote!(*this), quote!(Copy))
        };
        let bounds: Vec<WherePredicate> = typarams
            .iter()
            .map(|ty| parse_quote!(#ty: #clone_trait + 'static))
            .collect();
        quote! {
            impl<'zf, #(#tybounds),*> zerofrom::ZeroFrom<'zf, #name<#(#typarams),*>> for #name<#(#typarams),*> where #(#bounds),* {
                fn zero_from(this: &'zf Self) -> Self {
                    #clone
                }
            }
        }
    } else {
        if lts > 1 {
            return syn::Error::new(
                input.generics.span(),
                "derive(ZeroFrom) cannot have multiple lifetime parameters",
            )
            .to_compile_error();
        }

        let mut zf_bounds: Vec<WherePredicate> = vec![];
        let body = structure.each_variant(|vi| {
            vi.construct(|f, i| {
                let binding = format!("__binding_{i}");
                let field = Ident::new(&binding, Span::call_site());

                if has_attr(&f.attrs, "clone") {
                    quote! {
                        #field.clone()
                    }
                } else {
                    // the field type
                    let fty = replace_lifetime(&f.ty, custom_lt("'zf"));
                    // the corresponding lifetimey type we are borrowing from (effectively, the C type)
                    let lifetime_ty =
                        replace_lifetime_and_type(&f.ty, custom_lt("'zf_inner"), &generics_env);

                    let (has_ty, has_lt) = visitor::check_type_for_parameters(&f.ty, &generics_env);
                    if has_ty {
                        // For types without type parameters, the compiler can figure out that the field implements
                        // ZeroFrom on its own. However, if there are type parameters, there may be complex preconditions
                        // to `FieldTy: ZeroFrom` that need to be satisfied. We get them to be satisfied by requiring
                        // `FieldTy<'zf>: ZeroFrom<'zf, FieldTy<'zf_inner>>`
                        if has_lt {
                            zf_bounds
                                .push(parse_quote!(#fty: zerofrom::ZeroFrom<'zf, #lifetime_ty>));
                        } else {
                            zf_bounds.push(parse_quote!(#fty: zerofrom::ZeroFrom<'zf, #fty>));
                        }
                    }
                    if has_ty || has_lt {
                        // By doing this we essentially require ZF to be implemented
                        // on all fields
                        quote! {
                            <#fty as zerofrom::ZeroFrom<'zf, #lifetime_ty>>::zero_from(#field)
                        }
                    } else {
                        // No lifetimes, so we can just copy
                        quote! { *#field }
                    }
                }
            })
        });
        // Due to the possibility of generics_may_borrow, we might reach here with no lifetimes on self,
        // don't accidentally feed them to self later
        let (maybe_zf_lifetime, maybe_zf_inner_lifetime) = if lts == 0 {
            (quote!(), quote!())
        } else {
            (quote!('zf,), quote!('zf_inner,))
        };

        // Array of C types. Only different if generics are allowed to borrow
        let mut typarams_c = typarams.clone();

        if generics_may_borrow {
            for typaram_c in &mut typarams_c {
                if let Some(Some(replacement)) = generics_env.get(typaram_c) {
                    // we use mem::replace here so we can be really clear about the C vs the T type
                    let typaram_t = mem::replace(typaram_c, replacement.clone());
                    zf_bounds
                        .push(parse_quote!(#typaram_c: zerofrom::ZeroFrom<'zf_inner, #typaram_t>));
                    tybounds.push(parse_quote!(#typaram_c));
                }
            }
        }

        quote! {
            impl<'zf, 'zf_inner, #(#tybounds),*> zerofrom::ZeroFrom<'zf, #name<#maybe_zf_inner_lifetime #(#typarams_c),*>> for #name<#maybe_zf_lifetime #(#typarams),*>
                where
                #(#zf_bounds,)* {
                fn zero_from(this: &'zf #name<#maybe_zf_inner_lifetime #(#typarams_c),*>) -> Self {
                    match *this { #body }
                }
            }
        }
    }
}

fn custom_lt(s: &str) -> Lifetime {
    Lifetime::new(s, Span::call_site())
}

/// Replace all lifetimes in a type with a specified one
fn replace_lifetime(x: &Type, lt: Lifetime) -> Type {
    struct ReplaceLifetime(Lifetime);

    impl Fold for ReplaceLifetime {
        fn fold_lifetime(&mut self, _: Lifetime) -> Lifetime {
            self.0.clone()
        }
    }
    ReplaceLifetime(lt).fold_type(x.clone())
}

/// Replace all lifetimes in a type with a specified one, AND replace all types that have a corresponding C type
/// with the C type
fn replace_lifetime_and_type(
    x: &Type,
    lt: Lifetime,
    generics_env: &HashMap<Ident, Option<Ident>>,
) -> Type {
    struct ReplaceLifetimeAndTy<'a>(Lifetime, &'a HashMap<Ident, Option<Ident>>);

    impl Fold for ReplaceLifetimeAndTy<'_> {
        fn fold_lifetime(&mut self, _: Lifetime) -> Lifetime {
            self.0.clone()
        }
        fn fold_type_path(&mut self, i: TypePath) -> TypePath {
            if i.qself.is_none() {
                if let Some(ident) = i.path.get_ident() {
                    if let Some(Some(replacement)) = self.1.get(ident) {
                        return parse_quote!(#replacement);
                    }
                }
            }
            fold::fold_type_path(self, i)
        }
    }
    ReplaceLifetimeAndTy(lt, generics_env).fold_type(x.clone())
}

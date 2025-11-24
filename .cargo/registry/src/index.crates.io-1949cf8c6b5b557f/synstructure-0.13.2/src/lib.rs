//! This crate provides helper types for matching against enum variants, and
//! extracting bindings to each of the fields in the deriving Struct or Enum in
//! a generic way.
//!
//! If you are writing a `#[derive]` which needs to perform some operation on
//! every field, then you have come to the right place!
//!
//! # Example: `WalkFields`
//! ### Trait Implementation
//! ```
//! pub trait WalkFields: std::any::Any {
//!     fn walk_fields(&self, walk: &mut FnMut(&WalkFields));
//! }
//! impl WalkFields for i32 {
//!     fn walk_fields(&self, _walk: &mut FnMut(&WalkFields)) {}
//! }
//! ```
//!
//! ### Custom Derive
//! ```
//! # use quote::quote;
//! fn walkfields_derive(s: synstructure::Structure) -> proc_macro2::TokenStream {
//!     let body = s.each(|bi| quote!{
//!         walk(#bi)
//!     });
//!
//!     s.gen_impl(quote! {
//!         extern crate synstructure_test_traits;
//!
//!         gen impl synstructure_test_traits::WalkFields for @Self {
//!             fn walk_fields(&self, walk: &mut FnMut(&synstructure_test_traits::WalkFields)) {
//!                 match *self { #body }
//!             }
//!         }
//!     })
//! }
//! # const _IGNORE: &'static str = stringify!(
//! synstructure::decl_derive!([WalkFields] => walkfields_derive);
//! # );
//!
//! /*
//!  * Test Case
//!  */
//! fn main() {
//!     synstructure::test_derive! {
//!         walkfields_derive {
//!             enum A<T> {
//!                 B(i32, T),
//!                 C(i32),
//!             }
//!         }
//!         expands to {
//!             const _: () = {
//!                 extern crate synstructure_test_traits;
//!                 impl<T> synstructure_test_traits::WalkFields for A<T>
//!                     where T: synstructure_test_traits::WalkFields
//!                 {
//!                     fn walk_fields(&self, walk: &mut FnMut(&synstructure_test_traits::WalkFields)) {
//!                         match *self {
//!                             A::B(ref __binding_0, ref __binding_1,) => {
//!                                 { walk(__binding_0) }
//!                                 { walk(__binding_1) }
//!                             }
//!                             A::C(ref __binding_0,) => {
//!                                 { walk(__binding_0) }
//!                             }
//!                         }
//!                     }
//!                 }
//!             };
//!         }
//!     }
//! }
//! ```
//!
//! # Example: `Interest`
//! ### Trait Implementation
//! ```
//! pub trait Interest {
//!     fn interesting(&self) -> bool;
//! }
//! impl Interest for i32 {
//!     fn interesting(&self) -> bool { *self > 0 }
//! }
//! ```
//!
//! ### Custom Derive
//! ```
//! # use quote::quote;
//! fn interest_derive(mut s: synstructure::Structure) -> proc_macro2::TokenStream {
//!     let body = s.fold(false, |acc, bi| quote!{
//!         #acc || synstructure_test_traits::Interest::interesting(#bi)
//!     });
//!
//!     s.gen_impl(quote! {
//!         extern crate synstructure_test_traits;
//!         gen impl synstructure_test_traits::Interest for @Self {
//!             fn interesting(&self) -> bool {
//!                 match *self {
//!                     #body
//!                 }
//!             }
//!         }
//!     })
//! }
//! # const _IGNORE: &'static str = stringify!(
//! synstructure::decl_derive!([Interest] => interest_derive);
//! # );
//!
//! /*
//!  * Test Case
//!  */
//! fn main() {
//!     synstructure::test_derive!{
//!         interest_derive {
//!             enum A<T> {
//!                 B(i32, T),
//!                 C(i32),
//!             }
//!         }
//!         expands to {
//!             const _: () = {
//!                 extern crate synstructure_test_traits;
//!                 impl<T> synstructure_test_traits::Interest for A<T>
//!                     where T: synstructure_test_traits::Interest
//!                 {
//!                     fn interesting(&self) -> bool {
//!                         match *self {
//!                             A::B(ref __binding_0, ref __binding_1,) => {
//!                                 false ||
//!                                     synstructure_test_traits::Interest::interesting(__binding_0) ||
//!                                     synstructure_test_traits::Interest::interesting(__binding_1)
//!                             }
//!                             A::C(ref __binding_0,) => {
//!                                 false ||
//!                                     synstructure_test_traits::Interest::interesting(__binding_0)
//!                             }
//!                         }
//!                     }
//!                 }
//!             };
//!         }
//!     }
//! }
//! ```
//!
//! For more example usage, consider investigating the `abomonation_derive` crate,
//! which makes use of this crate, and is fairly simple.

#![allow(
    clippy::default_trait_access,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value
)]

#[cfg(all(
    not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
    feature = "proc-macro"
))]
extern crate proc_macro;

use std::collections::HashSet;

use syn::parse::{ParseStream, Parser};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    braced, punctuated, token, Attribute, Data, DeriveInput, Error, Expr, Field, Fields,
    FieldsNamed, FieldsUnnamed, GenericParam, Generics, Ident, PredicateType, Result, Token,
    TraitBound, Type, TypeMacro, TypeParamBound, TypePath, WhereClause, WherePredicate,
};

use quote::{format_ident, quote_spanned, ToTokens};
// re-export the quote! macro so we can depend on it being around in our macro's
// implementations.
#[doc(hidden)]
pub use quote::quote;

use proc_macro2::{Span, TokenStream, TokenTree};

// NOTE: This module has documentation hidden, as it only exports macros (which
// always appear in the root of the crate) and helper methods / re-exports used
// in the implementation of those macros.
#[doc(hidden)]
pub mod macros;

/// Changes how bounds are added
#[allow(clippy::manual_non_exhaustive)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AddBounds {
    /// Add for fields and generics
    Both,
    /// Fields only
    Fields,
    /// Generics only
    Generics,
    /// None
    None,
    #[doc(hidden)]
    __Nonexhaustive,
}

/// The type of binding to use when generating a pattern.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BindStyle {
    /// `x`
    Move,
    /// `mut x`
    MoveMut,
    /// `ref x`
    Ref,
    /// `ref mut x`
    RefMut,
}

impl ToTokens for BindStyle {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            BindStyle::Move => {}
            BindStyle::MoveMut => quote_spanned!(Span::call_site() => mut).to_tokens(tokens),
            BindStyle::Ref => quote_spanned!(Span::call_site() => ref).to_tokens(tokens),
            BindStyle::RefMut => quote_spanned!(Span::call_site() => ref mut).to_tokens(tokens),
        }
    }
}

// Internal method for merging seen_generics arrays together.
fn generics_fuse(res: &mut Vec<bool>, new: &[bool]) {
    for (i, &flag) in new.iter().enumerate() {
        if i == res.len() {
            res.push(false);
        }
        if flag {
            res[i] = true;
        }
    }
}

// Internal method for extracting the set of generics which have been matched.
fn fetch_generics<'a>(set: &[bool], generics: &'a Generics) -> Vec<&'a Ident> {
    let mut tys = vec![];
    for (&seen, param) in set.iter().zip(generics.params.iter()) {
        if seen {
            if let GenericParam::Type(tparam) = param {
                tys.push(&tparam.ident);
            }
        }
    }
    tys
}

// Internal method to merge two Generics objects together intelligently.
fn merge_generics(into: &mut Generics, from: &Generics) -> Result<()> {
    // Try to add the param into `into`, and merge parmas with identical names.
    for p in &from.params {
        for op in &into.params {
            match (op, p) {
                (GenericParam::Type(otp), GenericParam::Type(tp)) => {
                    // NOTE: This is only OK because syn ignores the span for equality purposes.
                    if otp.ident == tp.ident {
                        return Err(Error::new_spanned(
                            p,
                            format!(
                                "Attempted to merge conflicting generic parameters: {} and {}",
                                quote!(#op),
                                quote!(#p)
                            ),
                        ));
                    }
                }
                (GenericParam::Lifetime(olp), GenericParam::Lifetime(lp)) => {
                    // NOTE: This is only OK because syn ignores the span for equality purposes.
                    if olp.lifetime == lp.lifetime {
                        return Err(Error::new_spanned(
                            p,
                            format!(
                                "Attempted to merge conflicting generic parameters: {} and {}",
                                quote!(#op),
                                quote!(#p)
                            ),
                        ));
                    }
                }
                // We don't support merging Const parameters, because that wouldn't make much sense.
                _ => (),
            }
        }
        into.params.push(p.clone());
    }

    // Add any where clauses from the input generics object.
    if let Some(from_clause) = &from.where_clause {
        into.make_where_clause()
            .predicates
            .extend(from_clause.predicates.iter().cloned());
    }

    Ok(())
}

/// Helper method which does the same thing as rustc 1.20's
/// `Option::get_or_insert_with`. This method is used to keep backwards
/// compatibility with rustc 1.15.
fn get_or_insert_with<T, F>(opt: &mut Option<T>, f: F) -> &mut T
where
    F: FnOnce() -> T,
{
    if opt.is_none() {
        *opt = Some(f());
    }

    match opt {
        Some(v) => v,
        None => unreachable!(),
    }
}

/// Information about a specific binding. This contains both an `Ident`
/// reference to the given field, and the syn `&'a Field` descriptor for that
/// field.
///
/// This type supports `quote::ToTokens`, so can be directly used within the
/// `quote!` macro. It expands to a reference to the matched field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingInfo<'a> {
    /// The name which this `BindingInfo` will bind to.
    pub binding: Ident,

    /// The type of binding which this `BindingInfo` will create.
    pub style: BindStyle,

    field: &'a Field,

    // These are used to determine which type parameters are avaliable.
    generics: &'a Generics,
    seen_generics: Vec<bool>,
    // The original index of the binding
    // this will not change when .filter() is called
    index: usize,
}

impl ToTokens for BindingInfo<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.binding.to_tokens(tokens);
    }
}

impl<'a> BindingInfo<'a> {
    /// Returns a reference to the underlying `syn` AST node which this
    /// `BindingInfo` references
    pub fn ast(&self) -> &'a Field {
        self.field
    }

    /// Generates the pattern fragment for this field binding.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].bindings()[0].pat().to_string(),
    ///     quote! {
    ///         ref __binding_0
    ///     }.to_string()
    /// );
    /// ```
    pub fn pat(&self) -> TokenStream {
        let BindingInfo { binding, style, .. } = self;
        quote!(#style #binding)
    }

    /// Returns a list of the type parameters which are referenced in this
    /// field's type.
    ///
    /// # Caveat
    ///
    /// If the field contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     struct A<T, U> {
    ///         a: Option<T>,
    ///         b: U,
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].bindings()[0].referenced_ty_params(),
    ///     &[&quote::format_ident!("T")]
    /// );
    /// ```
    pub fn referenced_ty_params(&self) -> Vec<&'a Ident> {
        fetch_generics(&self.seen_generics, self.generics)
    }
}

/// This type is similar to `syn`'s `Variant` type, however each of the fields
/// are references rather than owned. When this is used as the AST for a real
/// variant, this struct simply borrows the fields of the `syn::Variant`,
/// however this type may also be used as the sole variant for a struct.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct VariantAst<'a> {
    pub attrs: &'a [Attribute],
    pub ident: &'a Ident,
    pub fields: &'a Fields,
    pub discriminant: &'a Option<(token::Eq, Expr)>,
}

/// A wrapper around a `syn::DeriveInput`'s variant which provides utilities
/// for destructuring `Variant`s with `match` expressions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariantInfo<'a> {
    pub prefix: Option<&'a Ident>,
    bindings: Vec<BindingInfo<'a>>,
    ast: VariantAst<'a>,
    generics: &'a Generics,
    // The original length of `bindings` before any `.filter()` calls
    original_length: usize,
}

/// Helper function used by the `VariantInfo` constructor. Walks all of the types
/// in `field` and returns a list of the type parameters from `ty_params` which
/// are referenced in the field.
fn get_ty_params(field: &Field, generics: &Generics) -> Vec<bool> {
    // Helper type. Discovers all identifiers inside of the visited type,
    // and calls a callback with them.
    struct BoundTypeLocator<'a> {
        result: Vec<bool>,
        generics: &'a Generics,
    }

    impl<'a> Visit<'a> for BoundTypeLocator<'a> {
        // XXX: This also (intentionally) captures paths like T::SomeType. Is
        // this desirable?
        fn visit_ident(&mut self, id: &Ident) {
            for (idx, i) in self.generics.params.iter().enumerate() {
                if let GenericParam::Type(tparam) = i {
                    if tparam.ident == *id {
                        self.result[idx] = true;
                    }
                }
            }
        }

        fn visit_type_macro(&mut self, x: &'a TypeMacro) {
            // If we see a type_mac declaration, then we can't know what type parameters
            // it might be binding, so we presume it binds all of them.
            for r in &mut self.result {
                *r = true;
            }
            visit::visit_type_macro(self, x);
        }
    }

    let mut btl = BoundTypeLocator {
        result: vec![false; generics.params.len()],
        generics,
    };

    btl.visit_type(&field.ty);

    btl.result
}

impl<'a> VariantInfo<'a> {
    fn new(ast: VariantAst<'a>, prefix: Option<&'a Ident>, generics: &'a Generics) -> Self {
        let bindings = match ast.fields {
            Fields::Unit => vec![],
            Fields::Unnamed(FieldsUnnamed {
                unnamed: fields, ..
            })
            | Fields::Named(FieldsNamed { named: fields, .. }) => {
                fields
                    .into_iter()
                    .enumerate()
                    .map(|(i, field)| {
                        // XXX: binding_span has to be call_site to avoid privacy
                        // when deriving on private fields, but be located at the field
                        // span for nicer diagnostics.
                        let binding_span = Span::call_site().located_at(field.span());
                        BindingInfo {
                            binding: format_ident!("__binding_{}", i, span = binding_span),
                            style: BindStyle::Ref,
                            field,
                            generics,
                            seen_generics: get_ty_params(field, generics),
                            index: i,
                        }
                    })
                    .collect::<Vec<_>>()
            }
        };

        let original_length = bindings.len();
        VariantInfo {
            prefix,
            bindings,
            ast,
            generics,
            original_length,
        }
    }

    /// Returns a slice of the bindings in this Variant.
    pub fn bindings(&self) -> &[BindingInfo<'a>] {
        &self.bindings
    }

    /// Returns a mut slice of the bindings in this Variant.
    pub fn bindings_mut(&mut self) -> &mut [BindingInfo<'a>] {
        &mut self.bindings
    }

    /// Returns a `VariantAst` object which contains references to the
    /// underlying `syn` AST node which this `Variant` was created from.
    pub fn ast(&self) -> VariantAst<'a> {
        self.ast
    }

    /// True if any bindings were omitted due to a `filter` call.
    pub fn omitted_bindings(&self) -> bool {
        self.original_length != self.bindings.len()
    }

    /// Generates the match-arm pattern which could be used to match against this Variant.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].pat().to_string(),
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,)
    ///     }.to_string()
    /// );
    /// ```
    pub fn pat(&self) -> TokenStream {
        let mut t = TokenStream::new();
        if let Some(prefix) = self.prefix {
            prefix.to_tokens(&mut t);
            quote!(::).to_tokens(&mut t);
        }
        self.ast.ident.to_tokens(&mut t);
        match self.ast.fields {
            Fields::Unit => {
                assert!(self.bindings.is_empty());
            }
            Fields::Unnamed(..) => token::Paren(Span::call_site()).surround(&mut t, |t| {
                let mut expected_index = 0;
                for binding in &self.bindings {
                    while expected_index < binding.index {
                        quote!(_,).to_tokens(t);
                        expected_index += 1;
                    }
                    binding.pat().to_tokens(t);
                    quote!(,).to_tokens(t);
                    expected_index += 1;
                }
                if expected_index != self.original_length {
                    quote!(..).to_tokens(t);
                }
            }),
            Fields::Named(..) => token::Brace(Span::call_site()).surround(&mut t, |t| {
                for binding in &self.bindings {
                    binding.field.ident.to_tokens(t);
                    quote!(:).to_tokens(t);
                    binding.pat().to_tokens(t);
                    quote!(,).to_tokens(t);
                }
                if self.omitted_bindings() {
                    quote!(..).to_tokens(t);
                }
            }),
        }
        t
    }

    /// Generates the token stream required to construct the current variant.
    ///
    /// The init array initializes each of the fields in the order they are
    /// written in `variant.ast().fields`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(usize, usize),
    ///         C{ v: usize },
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].construct(|_, i| quote!(#i)).to_string(),
    ///
    ///     quote!{
    ///         A::B(0usize, 1usize,)
    ///     }.to_string()
    /// );
    ///
    /// assert_eq!(
    ///     s.variants()[1].construct(|_, i| quote!(#i)).to_string(),
    ///
    ///     quote!{
    ///         A::C{ v: 0usize, }
    ///     }.to_string()
    /// );
    /// ```
    pub fn construct<F, T>(&self, mut func: F) -> TokenStream
    where
        F: FnMut(&Field, usize) -> T,
        T: ToTokens,
    {
        let mut t = TokenStream::new();
        if let Some(prefix) = self.prefix {
            quote!(#prefix ::).to_tokens(&mut t);
        }
        self.ast.ident.to_tokens(&mut t);

        match &self.ast.fields {
            Fields::Unit => (),
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
                token::Paren::default().surround(&mut t, |t| {
                    for (i, field) in unnamed.into_iter().enumerate() {
                        func(field, i).to_tokens(t);
                        quote!(,).to_tokens(t);
                    }
                });
            }
            Fields::Named(FieldsNamed { named, .. }) => {
                token::Brace::default().surround(&mut t, |t| {
                    for (i, field) in named.into_iter().enumerate() {
                        field.ident.to_tokens(t);
                        quote!(:).to_tokens(t);
                        func(field, i).to_tokens(t);
                        quote!(,).to_tokens(t);
                    }
                });
            }
        }
        t
    }

    /// Runs the passed-in function once for each bound field, passing in a `BindingInfo`.
    /// and generating a `match` arm which evaluates the returned tokens.
    ///
    /// This method will ignore fields which are ignored through the `filter`
    /// method.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,) => {
    ///             { println!("{:?}", __binding_0) }
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn each<F, R>(&self, mut f: F) -> TokenStream
    where
        F: FnMut(&BindingInfo<'_>) -> R,
        R: ToTokens,
    {
        let pat = self.pat();
        let mut body = TokenStream::new();
        for binding in &self.bindings {
            token::Brace::default().surround(&mut body, |body| {
                f(binding).to_tokens(body);
            });
        }
        quote!(#pat => { #body })
    }

    /// Runs the passed-in function once for each bound field, passing in the
    /// result of the previous call, and a `BindingInfo`. generating a `match`
    /// arm which evaluates to the resulting tokens.
    ///
    /// This method will ignore fields which are ignored through the `filter`
    /// method.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].fold(quote!(0), |acc, bi| quote!(#acc + #bi)).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,) => {
    ///             0 + __binding_0 + __binding_1
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn fold<F, I, R>(&self, init: I, mut f: F) -> TokenStream
    where
        F: FnMut(TokenStream, &BindingInfo<'_>) -> R,
        I: ToTokens,
        R: ToTokens,
    {
        let pat = self.pat();
        let body = self.bindings.iter().fold(quote!(#init), |i, bi| {
            let r = f(i, bi);
            quote!(#r)
        });
        quote!(#pat => { #body })
    }

    /// Filter the bindings created by this `Variant` object. This has 2 effects:
    ///
    /// * The bindings will no longer appear in match arms generated by methods
    ///   on this `Variant` or its subobjects.
    ///
    /// * Impl blocks created with the `bound_impl` or `unsafe_bound_impl`
    ///   method only consider type parameters referenced in the types of
    ///   non-filtered fields.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C{ a: u32 },
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.variants_mut()[0].filter(|bi| {
    ///     bi.ast().ident == Some(quote::format_ident!("b"))
    /// });
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ b: ref __binding_1, .. } => {
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///         A::C{ a: ref __binding_0, } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn filter<F>(&mut self, f: F) -> &mut Self
    where
        F: FnMut(&BindingInfo<'_>) -> bool,
    {
        self.bindings.retain(f);
        self
    }

    /// Iterates all the bindings of this `Variant` object and uses a closure to determine if a
    /// binding should be removed. If the closure returns `true` the binding is removed from the
    /// variant. If the closure returns `false`, the binding remains in the variant.
    ///
    /// All the removed bindings are moved to a new `Variant` object which is otherwise identical
    /// to the current one. To understand the effects of removing a binding from a variant check
    /// the [`VariantInfo::filter`] documentation.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C{ a: u32 },
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// let mut with_b = &mut s.variants_mut()[0];
    ///
    /// let with_a = with_b.drain_filter(|bi| {
    ///     bi.ast().ident == Some(quote::format_ident!("a"))
    /// });
    ///
    /// assert_eq!(
    ///     with_a.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ a: ref __binding_0, .. } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    ///
    /// assert_eq!(
    ///     with_b.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ b: ref __binding_1, .. } => {
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    #[allow(clippy::return_self_not_must_use)]
    pub fn drain_filter<F>(&mut self, mut f: F) -> Self
    where
        F: FnMut(&BindingInfo<'_>) -> bool,
    {
        let mut other = VariantInfo {
            prefix: self.prefix,
            bindings: vec![],
            ast: self.ast,
            generics: self.generics,
            original_length: self.original_length,
        };

        let (other_bindings, self_bindings) = self.bindings.drain(..).partition(&mut f);
        other.bindings = other_bindings;
        self.bindings = self_bindings;

        other
    }

    /// Remove the binding at the given index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of range.
    pub fn remove_binding(&mut self, idx: usize) -> &mut Self {
        self.bindings.remove(idx);
        self
    }

    /// Updates the `BindStyle` for each of the passed-in fields by calling the
    /// passed-in function for each `BindingInfo`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.variants_mut()[0].bind_with(|bi| BindStyle::RefMut);
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref mut __binding_0, ref mut __binding_1,) => {
    ///             { println!("{:?}", __binding_0) }
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///         A::C(ref __binding_0,) => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn bind_with<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(&BindingInfo<'_>) -> BindStyle,
    {
        for binding in &mut self.bindings {
            binding.style = f(binding);
        }
        self
    }

    /// Updates the binding name for each fo the passed-in fields by calling the
    /// passed-in function for each `BindingInfo`.
    ///
    /// The function will be called with the `BindingInfo` and its index in the
    /// enclosing variant.
    ///
    /// The default name is `__binding_{}` where `{}` is replaced with an
    /// increasing number.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C{ a: u32 },
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.variants_mut()[0].binding_name(|bi, i| bi.ident.clone().unwrap());
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ a: ref a, b: ref b, } => {
    ///             { println!("{:?}", a) }
    ///             { println!("{:?}", b) }
    ///         }
    ///         A::C{ a: ref __binding_0, } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn binding_name<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(&Field, usize) -> Ident,
    {
        for (it, binding) in self.bindings.iter_mut().enumerate() {
            binding.binding = f(binding.field, it);
        }
        self
    }

    /// Returns a list of the type parameters which are referenced in this
    /// field's type.
    ///
    /// # Caveat
    ///
    /// If the field contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     struct A<T, U> {
    ///         a: Option<T>,
    ///         b: U,
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.variants()[0].bindings()[0].referenced_ty_params(),
    ///     &[&quote::format_ident!("T")]
    /// );
    /// ```
    pub fn referenced_ty_params(&self) -> Vec<&'a Ident> {
        let mut flags = Vec::new();
        for binding in &self.bindings {
            generics_fuse(&mut flags, &binding.seen_generics);
        }
        fetch_generics(&flags, self.generics)
    }
}

/// A wrapper around a `syn::DeriveInput` which provides utilities for creating
/// custom derive trait implementations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Structure<'a> {
    variants: Vec<VariantInfo<'a>>,
    omitted_variants: bool,
    ast: &'a DeriveInput,
    extra_impl: Vec<GenericParam>,
    extra_predicates: Vec<WherePredicate>,
    add_bounds: AddBounds,
}

impl<'a> Structure<'a> {
    /// Create a new `Structure` with the variants and fields from the passed-in
    /// `DeriveInput`.
    ///
    /// # Panics
    ///
    /// This method will panic if the provided AST node represents an untagged
    /// union.
    pub fn new(ast: &'a DeriveInput) -> Self {
        Self::try_new(ast).expect("Unable to create synstructure::Structure")
    }

    /// Create a new `Structure` with the variants and fields from the passed-in
    /// `DeriveInput`.
    ///
    /// Unlike `Structure::new`, this method does not panic if the provided AST
    /// node represents an untagged union.
    pub fn try_new(ast: &'a DeriveInput) -> Result<Self> {
        let variants = match &ast.data {
            Data::Enum(data) => (&data.variants)
                .into_iter()
                .map(|v| {
                    VariantInfo::new(
                        VariantAst {
                            attrs: &v.attrs,
                            ident: &v.ident,
                            fields: &v.fields,
                            discriminant: &v.discriminant,
                        },
                        Some(&ast.ident),
                        &ast.generics,
                    )
                })
                .collect::<Vec<_>>(),
            Data::Struct(data) => {
                vec![VariantInfo::new(
                    VariantAst {
                        attrs: &ast.attrs,
                        ident: &ast.ident,
                        fields: &data.fields,
                        discriminant: &None,
                    },
                    None,
                    &ast.generics,
                )]
            }
            Data::Union(_) => {
                return Err(Error::new_spanned(
                    ast,
                    "unexpected unsupported untagged union",
                ));
            }
        };

        Ok(Structure {
            variants,
            omitted_variants: false,
            ast,
            extra_impl: vec![],
            extra_predicates: vec![],
            add_bounds: AddBounds::Both,
        })
    }

    /// Returns a slice of the variants in this Structure.
    pub fn variants(&self) -> &[VariantInfo<'a>] {
        &self.variants
    }

    /// Returns a mut slice of the variants in this Structure.
    pub fn variants_mut(&mut self) -> &mut [VariantInfo<'a>] {
        &mut self.variants
    }

    /// Returns a reference to the underlying `syn` AST node which this
    /// `Structure` was created from.
    pub fn ast(&self) -> &'a DeriveInput {
        self.ast
    }

    /// True if any variants were omitted due to a `filter_variants` call.
    pub fn omitted_variants(&self) -> bool {
        self.omitted_variants
    }

    /// Runs the passed-in function once for each bound field, passing in a `BindingInfo`.
    /// and generating `match` arms which evaluate the returned tokens.
    ///
    /// This method will ignore variants or fields which are ignored through the
    /// `filter` and `filter_variant` methods.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,) => {
    ///             { println!("{:?}", __binding_0) }
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///         A::C(ref __binding_0,) => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn each<F, R>(&self, mut f: F) -> TokenStream
    where
        F: FnMut(&BindingInfo<'_>) -> R,
        R: ToTokens,
    {
        let mut t = TokenStream::new();
        for variant in &self.variants {
            variant.each(&mut f).to_tokens(&mut t);
        }
        if self.omitted_variants {
            quote!(_ => {}).to_tokens(&mut t);
        }
        t
    }

    /// Runs the passed-in function once for each bound field, passing in the
    /// result of the previous call, and a `BindingInfo`. generating `match`
    /// arms which evaluate to the resulting tokens.
    ///
    /// This method will ignore variants or fields which are ignored through the
    /// `filter` and `filter_variant` methods.
    ///
    /// If a variant has been ignored, it will return the `init` value.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.fold(quote!(0), |acc, bi| quote!(#acc + #bi)).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,) => {
    ///             0 + __binding_0 + __binding_1
    ///         }
    ///         A::C(ref __binding_0,) => {
    ///             0 + __binding_0
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn fold<F, I, R>(&self, init: I, mut f: F) -> TokenStream
    where
        F: FnMut(TokenStream, &BindingInfo<'_>) -> R,
        I: ToTokens,
        R: ToTokens,
    {
        let mut t = TokenStream::new();
        for variant in &self.variants {
            variant.fold(&init, &mut f).to_tokens(&mut t);
        }
        if self.omitted_variants {
            quote!(_ => { #init }).to_tokens(&mut t);
        }
        t
    }

    /// Runs the passed-in function once for each variant, passing in a
    /// `VariantInfo`. and generating `match` arms which evaluate the returned
    /// tokens.
    ///
    /// This method will ignore variants and not bind fields which are ignored
    /// through the `filter` and `filter_variant` methods.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let s = Structure::new(&di);
    ///
    /// assert_eq!(
    ///     s.each_variant(|v| {
    ///         let name = &v.ast().ident;
    ///         quote!(println!(stringify!(#name)))
    ///     }).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,) => {
    ///             println!(stringify!(B))
    ///         }
    ///         A::C(ref __binding_0,) => {
    ///             println!(stringify!(C))
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn each_variant<F, R>(&self, mut f: F) -> TokenStream
    where
        F: FnMut(&VariantInfo<'_>) -> R,
        R: ToTokens,
    {
        let mut t = TokenStream::new();
        for variant in &self.variants {
            let pat = variant.pat();
            let body = f(variant);
            quote!(#pat => { #body }).to_tokens(&mut t);
        }
        if self.omitted_variants {
            quote!(_ => {}).to_tokens(&mut t);
        }
        t
    }

    /// Filter the bindings created by this `Structure` object. This has 2 effects:
    ///
    /// * The bindings will no longer appear in match arms generated by methods
    ///   on this `Structure` or its subobjects.
    ///
    /// * Impl blocks created with the `bound_impl` or `unsafe_bound_impl`
    ///   method only consider type parameters referenced in the types of
    ///   non-filtered fields.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C{ a: u32 },
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter(|bi| {
    ///     bi.ast().ident == Some(quote::format_ident!("a"))
    /// });
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ a: ref __binding_0, .. } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///         A::C{ a: ref __binding_0, } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn filter<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(&BindingInfo<'_>) -> bool,
    {
        for variant in &mut self.variants {
            variant.filter(&mut f);
        }
        self
    }

    /// Iterates all the bindings of this `Structure` object and uses a closure to determine if a
    /// binding should be removed. If the closure returns `true` the binding is removed from the
    /// structure. If the closure returns `false`, the binding remains in the structure.
    ///
    /// All the removed bindings are moved to a new `Structure` object which is otherwise identical
    /// to the current one. To understand the effects of removing a binding from a structure check
    /// the [`Structure::filter`] documentation.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C{ a: u32 },
    ///     }
    /// };
    /// let mut with_b = Structure::new(&di);
    ///
    /// let with_a = with_b.drain_filter(|bi| {
    ///     bi.ast().ident == Some(quote::format_ident!("a"))
    /// });
    ///
    /// assert_eq!(
    ///     with_a.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ a: ref __binding_0, .. } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///         A::C{ a: ref __binding_0, } => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    ///
    /// assert_eq!(
    ///     with_b.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ b: ref __binding_1, .. } => {
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///         A::C{ .. } => {
    ///
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    #[allow(clippy::return_self_not_must_use)]
    pub fn drain_filter<F>(&mut self, mut f: F) -> Self
    where
        F: FnMut(&BindingInfo<'_>) -> bool,
    {
        Self {
            variants: self
                .variants
                .iter_mut()
                .map(|variant| variant.drain_filter(&mut f))
                .collect(),
            omitted_variants: self.omitted_variants,
            ast: self.ast,
            extra_impl: self.extra_impl.clone(),
            extra_predicates: self.extra_predicates.clone(),
            add_bounds: self.add_bounds,
        }
    }

    /// Specify additional where predicate bounds which should be generated by
    /// impl-generating functions such as `gen_impl`, `bound_impl`, and
    /// `unsafe_bound_impl`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// // Add an additional where predicate.
    /// s.add_where_predicate(syn::parse_quote!(T: std::fmt::Display));
    ///
    /// assert_eq!(
    ///     s.bound_impl(quote!(krate::Trait), quote!{
    ///         fn a() {}
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U> krate::Trait for A<T, U>
    ///                 where T: std::fmt::Display,
    ///                       T: krate::Trait,
    ///                       Option<U>: krate::Trait,
    ///                       U: krate::Trait
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    pub fn add_where_predicate(&mut self, pred: WherePredicate) -> &mut Self {
        self.extra_predicates.push(pred);
        self
    }

    /// Specify which bounds should be generated by impl-generating functions
    /// such as `gen_impl`, `bound_impl`, and `unsafe_bound_impl`.
    ///
    /// The default behaviour is to generate both field and generic bounds from
    /// type parameters.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// // Limit bounds to only generics.
    /// s.add_bounds(AddBounds::Generics);
    ///
    /// assert_eq!(
    ///     s.bound_impl(quote!(krate::Trait), quote!{
    ///         fn a() {}
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U> krate::Trait for A<T, U>
    ///                 where T: krate::Trait,
    ///                       U: krate::Trait
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    pub fn add_bounds(&mut self, mode: AddBounds) -> &mut Self {
        self.add_bounds = mode;
        self
    }

    /// Filter the variants matched by this `Structure` object. This has 2 effects:
    ///
    /// * Match arms destructuring these variants will no longer be generated by
    ///   methods on this `Structure`
    ///
    /// * Impl blocks created with the `bound_impl` or `unsafe_bound_impl`
    ///   method only consider type parameters referenced in the types of
    ///   fields in non-fitered variants.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    ///
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "B");
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::C(ref __binding_0,) => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///         _ => {}
    ///     }.to_string()
    /// );
    /// ```
    pub fn filter_variants<F>(&mut self, f: F) -> &mut Self
    where
        F: FnMut(&VariantInfo<'_>) -> bool,
    {
        let before_len = self.variants.len();
        self.variants.retain(f);
        if self.variants.len() != before_len {
            self.omitted_variants = true;
        }
        self
    }
    /// Iterates all the variants of this `Structure` object and uses a closure to determine if a
    /// variant should be removed. If the closure returns `true` the variant is removed from the
    /// structure. If the closure returns `false`, the variant remains in the structure.
    ///
    /// All the removed variants are moved to a new `Structure` object which is otherwise identical
    /// to the current one. To understand the effects of removing a variant from a structure check
    /// the [`Structure::filter_variants`] documentation.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    ///
    /// let mut with_c = Structure::new(&di);
    ///
    /// let with_b = with_c.drain_filter_variants(|v| v.ast().ident == "B");
    ///
    /// assert_eq!(
    ///     with_c.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::C(ref __binding_0,) => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    ///
    /// assert_eq!(
    ///     with_b.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref __binding_0, ref __binding_1,) => {
    ///             { println!("{:?}", __binding_0) }
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///     }.to_string()
    /// );
    #[allow(clippy::return_self_not_must_use)]
    pub fn drain_filter_variants<F>(&mut self, mut f: F) -> Self
    where
        F: FnMut(&VariantInfo<'_>) -> bool,
    {
        let mut other = Self {
            variants: vec![],
            omitted_variants: self.omitted_variants,
            ast: self.ast,
            extra_impl: self.extra_impl.clone(),
            extra_predicates: self.extra_predicates.clone(),
            add_bounds: self.add_bounds,
        };

        let (other_variants, self_variants) = self.variants.drain(..).partition(&mut f);
        other.variants = other_variants;
        self.variants = self_variants;

        other
    }

    /// Remove the variant at the given index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of range.
    pub fn remove_variant(&mut self, idx: usize) -> &mut Self {
        self.variants.remove(idx);
        self.omitted_variants = true;
        self
    }

    /// Updates the `BindStyle` for each of the passed-in fields by calling the
    /// passed-in function for each `BindingInfo`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B(i32, i32),
    ///         C(u32),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.bind_with(|bi| BindStyle::RefMut);
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B(ref mut __binding_0, ref mut __binding_1,) => {
    ///             { println!("{:?}", __binding_0) }
    ///             { println!("{:?}", __binding_1) }
    ///         }
    ///         A::C(ref mut __binding_0,) => {
    ///             { println!("{:?}", __binding_0) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn bind_with<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(&BindingInfo<'_>) -> BindStyle,
    {
        for variant in &mut self.variants {
            variant.bind_with(&mut f);
        }
        self
    }

    /// Updates the binding name for each fo the passed-in fields by calling the
    /// passed-in function for each `BindingInfo`.
    ///
    /// The function will be called with the `BindingInfo` and its index in the
    /// enclosing variant.
    ///
    /// The default name is `__binding_{}` where `{}` is replaced with an
    /// increasing number.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A {
    ///         B{ a: i32, b: i32 },
    ///         C{ a: u32 },
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.binding_name(|bi, i| bi.ident.clone().unwrap());
    ///
    /// assert_eq!(
    ///     s.each(|bi| quote!(println!("{:?}", #bi))).to_string(),
    ///
    ///     quote!{
    ///         A::B{ a: ref a, b: ref b, } => {
    ///             { println!("{:?}", a) }
    ///             { println!("{:?}", b) }
    ///         }
    ///         A::C{ a: ref a, } => {
    ///             { println!("{:?}", a) }
    ///         }
    ///     }.to_string()
    /// );
    /// ```
    pub fn binding_name<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(&Field, usize) -> Ident,
    {
        for variant in &mut self.variants {
            variant.binding_name(&mut f);
        }
        self
    }

    /// Returns a list of the type parameters which are refrenced in the types
    /// of non-filtered fields / variants.
    ///
    /// # Caveat
    ///
    /// If the struct contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T, i32),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "C");
    ///
    /// assert_eq!(
    ///     s.referenced_ty_params(),
    ///     &[&quote::format_ident!("T")]
    /// );
    /// ```
    pub fn referenced_ty_params(&self) -> Vec<&'a Ident> {
        let mut flags = Vec::new();
        for variant in &self.variants {
            for binding in &variant.bindings {
                generics_fuse(&mut flags, &binding.seen_generics);
            }
        }
        fetch_generics(&flags, &self.ast.generics)
    }

    /// Adds an `impl<>` generic parameter.
    /// This can be used when the trait to be derived needs some extra generic parameters.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    /// let generic: syn::GenericParam = syn::parse_quote!(X: krate::AnotherTrait);
    ///
    /// assert_eq!(
    ///     s.add_impl_generic(generic)
    ///         .bound_impl(quote!(krate::Trait<X>),
    ///         quote!{
    ///                 fn a() {}
    ///         }
    ///     ).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U, X: krate::AnotherTrait> krate::Trait<X> for A<T, U>
    ///                 where T : krate :: Trait < X >,
    ///                       Option<U>: krate::Trait<X>,
    ///                       U: krate::Trait<X>
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    pub fn add_impl_generic(&mut self, param: GenericParam) -> &mut Self {
        self.extra_impl.push(param);
        self
    }

    /// Add trait bounds for a trait with the given path for each type parmaeter
    /// referenced in the types of non-filtered fields.
    ///
    /// # Caveat
    ///
    /// If the method contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    pub fn add_trait_bounds(
        &self,
        bound: &TraitBound,
        where_clause: &mut Option<WhereClause>,
        mode: AddBounds,
    ) {
        // If we have any explicit where predicates, make sure to add them first.
        if !self.extra_predicates.is_empty() {
            let clause = get_or_insert_with(&mut *where_clause, || WhereClause {
                where_token: Default::default(),
                predicates: punctuated::Punctuated::new(),
            });
            clause
                .predicates
                .extend(self.extra_predicates.iter().cloned());
        }

        let mut seen = HashSet::new();
        let mut pred = |ty: Type| {
            if !seen.contains(&ty) {
                seen.insert(ty.clone());

                // Add a predicate.
                let clause = get_or_insert_with(&mut *where_clause, || WhereClause {
                    where_token: Default::default(),
                    predicates: punctuated::Punctuated::new(),
                });
                clause.predicates.push(WherePredicate::Type(PredicateType {
                    lifetimes: None,
                    bounded_ty: ty,
                    colon_token: Default::default(),
                    bounds: Some(punctuated::Pair::End(TypeParamBound::Trait(bound.clone())))
                        .into_iter()
                        .collect(),
                }));
            }
        };

        for variant in &self.variants {
            for binding in &variant.bindings {
                match mode {
                    AddBounds::Both | AddBounds::Fields => {
                        for &seen in &binding.seen_generics {
                            if seen {
                                pred(binding.ast().ty.clone());
                                break;
                            }
                        }
                    }
                    _ => {}
                }

                match mode {
                    AddBounds::Both | AddBounds::Generics => {
                        for param in binding.referenced_ty_params() {
                            pred(Type::Path(TypePath {
                                qself: None,
                                path: (*param).clone().into(),
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// This method is a no-op, underscore consts are used by default now.
    pub fn underscore_const(&mut self, _enabled: bool) -> &mut Self {
        self
    }

    /// > NOTE: This methods' features are superceded by `Structure::gen_impl`.
    ///
    /// Creates an `impl` block with the required generic type fields filled in
    /// to implement the trait `path`.
    ///
    /// This method also adds where clauses to the impl requiring that all
    /// referenced type parmaeters implement the trait `path`.
    ///
    /// # Hygiene and Paths
    ///
    /// This method wraps the impl block inside of a `const` (see the example
    /// below). In this scope, the first segment of the passed-in path is
    /// `extern crate`-ed in. If you don't want to generate that `extern crate`
    /// item, use a global path.
    ///
    /// This means that if you are implementing `my_crate::Trait`, you simply
    /// write `s.bound_impl(quote!(my_crate::Trait), quote!(...))`, and for the
    /// entirety of the definition, you can refer to your crate as `my_crate`.
    ///
    /// # Caveat
    ///
    /// If the method contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    ///
    /// # Panics
    ///
    /// Panics if the path string parameter is not a valid `TraitBound`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "B");
    ///
    /// assert_eq!(
    ///     s.bound_impl(quote!(krate::Trait), quote!{
    ///         fn a() {}
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U> krate::Trait for A<T, U>
    ///                 where Option<U>: krate::Trait,
    ///                       U: krate::Trait
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    pub fn bound_impl<P: ToTokens, B: ToTokens>(&self, path: P, body: B) -> TokenStream {
        self.impl_internal(
            path.into_token_stream(),
            body.into_token_stream(),
            quote!(),
            None,
        )
    }

    /// > NOTE: This methods' features are superceded by `Structure::gen_impl`.
    ///
    /// Creates an `impl` block with the required generic type fields filled in
    /// to implement the unsafe trait `path`.
    ///
    /// This method also adds where clauses to the impl requiring that all
    /// referenced type parmaeters implement the trait `path`.
    ///
    /// # Hygiene and Paths
    ///
    /// This method wraps the impl block inside of a `const` (see the example
    /// below). In this scope, the first segment of the passed-in path is
    /// `extern crate`-ed in. If you don't want to generate that `extern crate`
    /// item, use a global path.
    ///
    /// This means that if you are implementing `my_crate::Trait`, you simply
    /// write `s.bound_impl(quote!(my_crate::Trait), quote!(...))`, and for the
    /// entirety of the definition, you can refer to your crate as `my_crate`.
    ///
    /// # Caveat
    ///
    /// If the method contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    ///
    /// # Panics
    ///
    /// Panics if the path string parameter is not a valid `TraitBound`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "B");
    ///
    /// assert_eq!(
    ///     s.unsafe_bound_impl(quote!(krate::Trait), quote!{
    ///         fn a() {}
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             unsafe impl<T, U> krate::Trait for A<T, U>
    ///                 where Option<U>: krate::Trait,
    ///                       U: krate::Trait
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    pub fn unsafe_bound_impl<P: ToTokens, B: ToTokens>(&self, path: P, body: B) -> TokenStream {
        self.impl_internal(
            path.into_token_stream(),
            body.into_token_stream(),
            quote!(unsafe),
            None,
        )
    }

    /// > NOTE: This methods' features are superceded by `Structure::gen_impl`.
    ///
    /// Creates an `impl` block with the required generic type fields filled in
    /// to implement the trait `path`.
    ///
    /// This method will not add any where clauses to the impl.
    ///
    /// # Hygiene and Paths
    ///
    /// This method wraps the impl block inside of a `const` (see the example
    /// below). In this scope, the first segment of the passed-in path is
    /// `extern crate`-ed in. If you don't want to generate that `extern crate`
    /// item, use a global path.
    ///
    /// This means that if you are implementing `my_crate::Trait`, you simply
    /// write `s.bound_impl(quote!(my_crate::Trait), quote!(...))`, and for the
    /// entirety of the definition, you can refer to your crate as `my_crate`.
    ///
    /// # Panics
    ///
    /// Panics if the path string parameter is not a valid `TraitBound`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "B");
    ///
    /// assert_eq!(
    ///     s.unbound_impl(quote!(krate::Trait), quote!{
    ///         fn a() {}
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U> krate::Trait for A<T, U> {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    pub fn unbound_impl<P: ToTokens, B: ToTokens>(&self, path: P, body: B) -> TokenStream {
        self.impl_internal(
            path.into_token_stream(),
            body.into_token_stream(),
            quote!(),
            Some(AddBounds::None),
        )
    }

    /// > NOTE: This methods' features are superceded by `Structure::gen_impl`.
    ///
    /// Creates an `impl` block with the required generic type fields filled in
    /// to implement the unsafe trait `path`.
    ///
    /// This method will not add any where clauses to the impl.
    ///
    /// # Hygiene and Paths
    ///
    /// This method wraps the impl block inside of a `const` (see the example
    /// below). In this scope, the first segment of the passed-in path is
    /// `extern crate`-ed in. If you don't want to generate that `extern crate`
    /// item, use a global path.
    ///
    /// This means that if you are implementing `my_crate::Trait`, you simply
    /// write `s.bound_impl(quote!(my_crate::Trait), quote!(...))`, and for the
    /// entirety of the definition, you can refer to your crate as `my_crate`.
    ///
    /// # Panics
    ///
    /// Panics if the path string parameter is not a valid `TraitBound`.
    ///
    /// # Example
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "B");
    ///
    /// assert_eq!(
    ///     s.unsafe_unbound_impl(quote!(krate::Trait), quote!{
    ///         fn a() {}
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             unsafe impl<T, U> krate::Trait for A<T, U> {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    #[deprecated]
    pub fn unsafe_unbound_impl<P: ToTokens, B: ToTokens>(&self, path: P, body: B) -> TokenStream {
        self.impl_internal(
            path.into_token_stream(),
            body.into_token_stream(),
            quote!(unsafe),
            Some(AddBounds::None),
        )
    }

    fn impl_internal(
        &self,
        path: TokenStream,
        body: TokenStream,
        safety: TokenStream,
        mode: Option<AddBounds>,
    ) -> TokenStream {
        let mode = mode.unwrap_or(self.add_bounds);
        let name = &self.ast.ident;
        let mut gen_clone = self.ast.generics.clone();
        gen_clone.params.extend(self.extra_impl.iter().cloned());
        let (impl_generics, _, _) = gen_clone.split_for_impl();
        let (_, ty_generics, where_clause) = self.ast.generics.split_for_impl();

        let bound = syn::parse2::<TraitBound>(path)
            .expect("`path` argument must be a valid rust trait bound");

        let mut where_clause = where_clause.cloned();
        self.add_trait_bounds(&bound, &mut where_clause, mode);

        // This function is smart. If a global path is passed, no extern crate
        // statement will be generated, however, a relative path will cause the
        // crate which it is relative to to be imported within the current
        // scope.
        let mut extern_crate = quote!();
        if bound.path.leading_colon.is_none() {
            if let Some(seg) = bound.path.segments.first() {
                let seg = &seg.ident;
                extern_crate = quote! { extern crate #seg; };
            }
        }

        let generated = quote! {
            #extern_crate
            #safety impl #impl_generics #bound for #name #ty_generics #where_clause {
                #body
            }
        };

        quote! {
            const _: () = { #generated };
        }
    }

    /// Generate an impl block for the given struct. This impl block will
    /// automatically use hygiene tricks to avoid polluting the caller's
    /// namespace, and will automatically add trait bounds for generic type
    /// parameters.
    ///
    /// # Syntax
    ///
    /// This function accepts its arguments as a `TokenStream`. The recommended way
    /// to call this function is passing the result of invoking the `quote!`
    /// macro to it.
    ///
    /// ```ignore
    /// s.gen_impl(quote! {
    ///     // You can write any items which you want to import into scope here.
    ///     // For example, you may want to include an `extern crate` for the
    ///     // crate which implements your trait. These items will only be
    ///     // visible to the code you generate, and won't be exposed to the
    ///     // consuming crate
    ///     extern crate krate;
    ///
    ///     // You can also add `use` statements here to bring types or traits
    ///     // into scope.
    ///     //
    ///     // WARNING: Try not to use common names here, because the stable
    ///     // version of syn does not support hygiene and you could accidentally
    ///     // shadow types from the caller crate.
    ///     use krate::Trait as MyTrait;
    ///
    ///     // The actual impl block is a `gen impl` or `gen unsafe impl` block.
    ///     // You can use `@Self` to refer to the structure's type.
    ///     gen impl MyTrait for @Self {
    ///         fn f(&self) { ... }
    ///     }
    /// })
    /// ```
    ///
    /// The most common usage of this trait involves loading the crate the
    /// target trait comes from with `extern crate`, and then invoking a `gen
    /// impl` block.
    ///
    /// # Hygiene
    ///
    /// This method tries to handle hygiene intelligently for both stable and
    /// unstable proc-macro implementations, however there are visible
    /// differences.
    ///
    /// The output of every `gen_impl` function is wrapped in a dummy `const`
    /// value, to ensure that it is given its own scope, and any values brought
    /// into scope are not leaked to the calling crate.
    ///
    /// By default, the above invocation may generate an output like the
    /// following:
    ///
    /// ```ignore
    /// const _: () = {
    ///     extern crate krate;
    ///     use krate::Trait as MyTrait;
    ///     impl<T> MyTrait for Struct<T> where T: MyTrait {
    ///         fn f(&self) { ... }
    ///     }
    /// };
    /// ```
    ///
    /// ### Using the `std` crate
    ///
    /// If you are using `quote!()` to implement your trait, with the
    /// `proc-macro2/nightly` feature, `std` isn't considered to be in scope for
    /// your macro. This means that if you use types from `std` in your
    /// procedural macro, you'll want to explicitly load it with an `extern
    /// crate std;`.
    ///
    /// ### Absolute paths
    ///
    /// You should generally avoid using absolute paths in your generated code,
    /// as they will resolve very differently when using the stable and nightly
    /// versions of `proc-macro2`. Instead, load the crates you need to use
    /// explictly with `extern crate` and
    ///
    /// # Trait Bounds
    ///
    /// This method will automatically add trait bounds for any type parameters
    /// which are referenced within the types of non-ignored fields.
    ///
    /// Additional type parameters may be added with the generics syntax after
    /// the `impl` keyword.
    ///
    /// ### Type Macro Caveat
    ///
    /// If the method contains any macros in type position, all parameters will
    /// be considered bound. This is because we cannot determine which type
    /// parameters are bound by type macros.
    ///
    /// # Errors
    ///
    /// This function will generate a `compile_error!` if additional type
    /// parameters added by `impl<..>` conflict with generic type parameters on
    /// the original struct.
    ///
    /// # Panics
    ///
    /// This function will panic if the input `TokenStream` is not well-formed.
    ///
    /// # Example Usage
    ///
    /// ```
    /// # use synstructure::*;
    /// let di: syn::DeriveInput = syn::parse_quote! {
    ///     enum A<T, U> {
    ///         B(T),
    ///         C(Option<U>),
    ///     }
    /// };
    /// let mut s = Structure::new(&di);
    ///
    /// s.filter_variants(|v| v.ast().ident != "B");
    ///
    /// assert_eq!(
    ///     s.gen_impl(quote! {
    ///         extern crate krate;
    ///         gen impl krate::Trait for @Self {
    ///             fn a() {}
    ///         }
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U> krate::Trait for A<T, U>
    ///             where
    ///                 Option<U>: krate::Trait,
    ///                 U: krate::Trait
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    ///
    /// // NOTE: You can also add extra generics after the impl
    /// assert_eq!(
    ///     s.gen_impl(quote! {
    ///         extern crate krate;
    ///         gen impl<X: krate::OtherTrait> krate::Trait<X> for @Self
    ///         where
    ///             X: Send + Sync,
    ///         {
    ///             fn a() {}
    ///         }
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<X: krate::OtherTrait, T, U> krate::Trait<X> for A<T, U>
    ///             where
    ///                 X: Send + Sync,
    ///                 Option<U>: krate::Trait<X>,
    ///                 U: krate::Trait<X>
    ///             {
    ///                 fn a() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    ///
    /// // NOTE: you can generate multiple traits with a single call
    /// assert_eq!(
    ///     s.gen_impl(quote! {
    ///         extern crate krate;
    ///
    ///         gen impl krate::Trait for @Self {
    ///             fn a() {}
    ///         }
    ///
    ///         gen impl krate::OtherTrait for @Self {
    ///             fn b() {}
    ///         }
    ///     }).to_string(),
    ///     quote!{
    ///         const _: () = {
    ///             extern crate krate;
    ///             impl<T, U> krate::Trait for A<T, U>
    ///             where
    ///                 Option<U>: krate::Trait,
    ///                 U: krate::Trait
    ///             {
    ///                 fn a() {}
    ///             }
    ///
    ///             impl<T, U> krate::OtherTrait for A<T, U>
    ///             where
    ///                 Option<U>: krate::OtherTrait,
    ///                 U: krate::OtherTrait
    ///             {
    ///                 fn b() {}
    ///             }
    ///         };
    ///     }.to_string()
    /// );
    /// ```
    ///
    /// Use `add_bounds` to change which bounds are generated.
    pub fn gen_impl(&self, cfg: TokenStream) -> TokenStream {
        Parser::parse2(
            |input: ParseStream<'_>| -> Result<TokenStream> { self.gen_impl_parse(input, true) },
            cfg,
        )
        .expect("Failed to parse gen_impl")
    }

    fn gen_impl_parse(&self, input: ParseStream<'_>, wrap: bool) -> Result<TokenStream> {
        fn parse_prefix(input: ParseStream<'_>) -> Result<Option<Token![unsafe]>> {
            if input.parse::<Ident>()? != "gen" {
                return Err(input.error("Expected keyword `gen`"));
            }
            let safety = input.parse::<Option<Token![unsafe]>>()?;
            let _ = input.parse::<Token![impl]>()?;
            Ok(safety)
        }

        let mut before = vec![];
        loop {
            if parse_prefix(&input.fork()).is_ok() {
                break;
            }
            before.push(input.parse::<TokenTree>()?);
        }

        // Parse the prefix "for real"
        let safety = parse_prefix(input)?;

        // optional `<>`
        let mut generics = input.parse::<Generics>()?;

        // @bound
        let bound = input.parse::<TraitBound>()?;

        // `for @Self`
        let _ = input.parse::<Token![for]>()?;
        let _ = input.parse::<Token![@]>()?;
        let _ = input.parse::<Token![Self]>()?;

        // optional `where ...`
        generics.where_clause = input.parse()?;

        // Body of the impl
        let body;
        braced!(body in input);
        let body = body.parse::<TokenStream>()?;

        // Try to parse the next entry in sequence. If this fails, we'll fall
        // back to just parsing the entire rest of the TokenStream.
        let maybe_next_impl = self.gen_impl_parse(&input.fork(), false);

        // Eat tokens to the end. Whether or not our speculative nested parse
        // succeeded, we're going to want to consume the rest of our input.
        let mut after = input.parse::<TokenStream>()?;
        if let Ok(stream) = maybe_next_impl {
            after = stream;
        }
        assert!(input.is_empty(), "Should've consumed the rest of our input");

        /* Codegen Logic */
        let name = &self.ast.ident;

        // Add the generics from the original struct in, and then add any
        // additional trait bounds which we need on the type.
        if let Err(err) = merge_generics(&mut generics, &self.ast.generics) {
            // Report the merge error as a `compile_error!`, as it may be
            // triggerable by an end-user.
            return Ok(err.to_compile_error());
        }

        self.add_trait_bounds(&bound, &mut generics.where_clause, self.add_bounds);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let (_, ty_generics, _) = self.ast.generics.split_for_impl();

        let generated = quote! {
            #(#before)*
            #safety impl #impl_generics #bound for #name #ty_generics #where_clause {
                #body
            }
            #after
        };

        if wrap {
            Ok(quote! {
                const _: () = { #generated };
            })
        } else {
            Ok(generated)
        }
    }
}

/// Dumps an unpretty version of a tokenstream. Takes any type which implements
/// `Display`.
///
/// This is mostly useful for visualizing the output of a procedural macro, as
/// it makes it marginally more readable. It is used in the implementation of
/// `test_derive!` to unprettily print the output.
///
/// # Stability
///
/// The stability of the output of this function is not guaranteed. Do not
/// assert that the output of this function does not change between minor
/// versions.
///
/// # Example
///
/// ```
/// # use quote::quote;
/// assert_eq!(
///     synstructure::unpretty_print(quote! {
///         const _: () = {
///             extern crate krate;
///             impl<T, U> krate::Trait for A<T, U>
///             where
///                 Option<U>: krate::Trait,
///                 U: krate::Trait
///             {
///                 fn a() {}
///             }
///         };
///     }),
///     "const _ : (
///     )
/// = {
///     extern crate krate ;
///     impl < T , U > krate :: Trait for A < T , U > where Option < U > : krate :: Trait , U : krate :: Trait {
///         fn a (
///             )
///         {
///             }
///         }
///     }
/// ;
/// "
/// )
/// ```
pub fn unpretty_print<T: std::fmt::Display>(ts: T) -> String {
    let mut res = String::new();

    let raw_s = ts.to_string();
    let mut s = &raw_s[..];
    let mut indent = 0;
    while let Some(i) = s.find(&['(', '{', '[', ')', '}', ']', ';'][..]) {
        match &s[i..=i] {
            "(" | "{" | "[" => indent += 1,
            ")" | "}" | "]" => indent -= 1,
            _ => {}
        }
        res.push_str(&s[..=i]);
        res.push('\n');
        for _ in 0..indent {
            res.push_str("    ");
        }
        s = trim_start_matches(&s[i + 1..], ' ');
    }
    res.push_str(s);
    res
}

/// `trim_left_matches` has been deprecated in favor of `trim_start_matches`.
/// This helper silences the warning, as we need to continue using
/// `trim_left_matches` for rust 1.15 support.
#[allow(deprecated)]
fn trim_start_matches(s: &str, c: char) -> &str {
    s.trim_left_matches(c)
}

/// Helper trait describing values which may be returned by macro implementation
/// methods used by this crate's macros.
pub trait MacroResult {
    /// Convert this result into a `Result` for further processing / validation.
    fn into_result(self) -> Result<TokenStream>;

    /// Convert this result into a `proc_macro::TokenStream`, ready to return
    /// from a native `proc_macro` implementation.
    ///
    /// If `into_result()` would return an `Err`, this method should instead
    /// generate a `compile_error!` invocation to nicely report the error.
    ///
    /// *This method is available if `synstructure` is built with the
    /// `"proc-macro"` feature.*
    #[cfg(all(
        not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
        feature = "proc-macro"
    ))]
    fn into_stream(self) -> proc_macro::TokenStream
    where
        Self: Sized,
    {
        match self.into_result() {
            Ok(ts) => ts.into(),
            Err(err) => err.to_compile_error().into(),
        }
    }
}

#[cfg(all(
    not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
    feature = "proc-macro"
))]
impl MacroResult for proc_macro::TokenStream {
    fn into_result(self) -> Result<TokenStream> {
        Ok(self.into())
    }

    fn into_stream(self) -> proc_macro::TokenStream {
        self
    }
}

impl MacroResult for TokenStream {
    fn into_result(self) -> Result<TokenStream> {
        Ok(self)
    }
}

impl<T: MacroResult> MacroResult for Result<T> {
    fn into_result(self) -> Result<TokenStream> {
        match self {
            Ok(v) => v.into_result(),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression test for #48
    #[test]
    fn test_each_enum() {
        let di: syn::DeriveInput = syn::parse_quote! {
         enum A {
             Foo(usize, bool),
             Bar(bool, usize),
             Baz(usize, bool, usize),
             Quux(bool, usize, bool)
         }
        };
        let mut s = Structure::new(&di);

        s.filter(|bi| bi.ast().ty.to_token_stream().to_string() == "bool");

        assert_eq!(
            s.each(|bi| quote!(do_something(#bi))).to_string(),
            quote! {
                A::Foo(_, ref __binding_1,) => { { do_something(__binding_1) } }
                A::Bar(ref __binding_0, ..) => { { do_something(__binding_0) } }
                A::Baz(_, ref __binding_1, ..) => { { do_something(__binding_1) } }
                A::Quux(ref __binding_0, _, ref __binding_2,) => {
                    {
                        do_something(__binding_0)
                    }
                    {
                        do_something(__binding_2)
                    }
                }
            }
            .to_string()
        );
    }
}

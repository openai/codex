//! Custom derive support for `zeroize`

#![crate_type = "proc-macro"]
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, trivial_casts, unused_qualifications)]

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    token::Comma,
    visit::Visit,
    Attribute, Data, DeriveInput, Expr, ExprLit, Field, Fields, Lit, Meta, Result, Variant,
    WherePredicate,
};

/// Name of zeroize-related attributes
const ZEROIZE_ATTR: &str = "zeroize";

/// Derive the `Zeroize` trait.
///
/// Supports the following attributes:
///
/// On the item level:
/// - `#[zeroize(drop)]`: *deprecated* use `ZeroizeOnDrop` instead
/// - `#[zeroize(bound = "T: MyTrait")]`: this replaces any trait bounds
///   inferred by zeroize-derive
///
/// On the field level:
/// - `#[zeroize(skip)]`: skips this field or variant when calling `zeroize()`
#[proc_macro_derive(Zeroize, attributes(zeroize))]
pub fn derive_zeroize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_zeroize_impl(syn::parse_macro_input!(input as DeriveInput)).into()
}

fn derive_zeroize_impl(input: DeriveInput) -> TokenStream {
    let attributes = ZeroizeAttrs::parse(&input);

    let mut generics = input.generics.clone();

    let extra_bounds = match attributes.bound {
        Some(bounds) => bounds.0,
        None => attributes
            .auto_params
            .iter()
            .map(|type_param| -> WherePredicate {
                parse_quote! {#type_param: Zeroize}
            })
            .collect(),
    };

    generics.make_where_clause().predicates.extend(extra_bounds);

    let ty_name = &input.ident;

    let (impl_gen, type_gen, where_) = generics.split_for_impl();

    let drop_impl = if attributes.drop {
        quote! {
            #[doc(hidden)]
            impl #impl_gen Drop for #ty_name #type_gen #where_ {
                fn drop(&mut self) {
                    self.zeroize()
                }
            }
        }
    } else {
        quote! {}
    };

    let zeroizers = generate_fields(&input, quote! { zeroize });
    let zeroize_impl = quote! {
        impl #impl_gen ::zeroize::Zeroize for #ty_name #type_gen #where_ {
            fn zeroize(&mut self) {
                #zeroizers
            }
        }
    };

    quote! {
        #zeroize_impl
        #drop_impl
    }
}

/// Derive the `ZeroizeOnDrop` trait.
///
/// Supports the following attributes:
///
/// On the field level:
/// - `#[zeroize(skip)]`: skips this field or variant when calling `zeroize()`
#[proc_macro_derive(ZeroizeOnDrop, attributes(zeroize))]
pub fn derive_zeroize_on_drop(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_zeroize_on_drop_impl(syn::parse_macro_input!(input as DeriveInput)).into()
}

fn derive_zeroize_on_drop_impl(input: DeriveInput) -> TokenStream {
    let zeroizers = generate_fields(&input, quote! { zeroize_or_on_drop });

    let (impl_gen, type_gen, where_) = input.generics.split_for_impl();
    let name = input.ident.clone();

    let drop_impl = quote! {
        impl #impl_gen Drop for #name #type_gen #where_ {
            fn drop(&mut self) {
                use ::zeroize::__internal::AssertZeroize;
                use ::zeroize::__internal::AssertZeroizeOnDrop;
                #zeroizers
            }
        }
    };
    let zeroize_on_drop_impl = impl_zeroize_on_drop(&input);

    quote! {
        #drop_impl
        #zeroize_on_drop_impl
    }
}

/// Custom derive attributes for `Zeroize`
#[derive(Default)]
struct ZeroizeAttrs {
    /// Derive a `Drop` impl which calls zeroize on this type
    drop: bool,
    /// Custom bounds as defined by the user
    bound: Option<Bounds>,
    /// Type parameters in use by fields
    auto_params: Vec<Ident>,
}

/// Parsing helper for custom bounds
struct Bounds(Punctuated<WherePredicate, Comma>);

impl Parse for Bounds {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
    }
}

struct BoundAccumulator<'a> {
    generics: &'a syn::Generics,
    params: Vec<Ident>,
}

impl<'ast> Visit<'ast> for BoundAccumulator<'ast> {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path.segments.len() != 1 {
            return;
        }

        if let Some(segment) = path.segments.first() {
            for param in &self.generics.params {
                if let syn::GenericParam::Type(type_param) = param {
                    if type_param.ident == segment.ident && !self.params.contains(&segment.ident) {
                        self.params.push(type_param.ident.clone());
                    }
                }
            }
        }
    }
}

impl ZeroizeAttrs {
    /// Parse attributes from the incoming AST
    fn parse(input: &DeriveInput) -> Self {
        let mut result = Self::default();
        let mut bound_accumulator = BoundAccumulator {
            generics: &input.generics,
            params: Vec::new(),
        };

        for attr in &input.attrs {
            result.parse_attr(attr, None, None);
        }

        match &input.data {
            syn::Data::Enum(enum_) => {
                for variant in &enum_.variants {
                    for attr in &variant.attrs {
                        result.parse_attr(attr, Some(variant), None);
                    }
                    for field in &variant.fields {
                        for attr in &field.attrs {
                            result.parse_attr(attr, Some(variant), Some(field));
                        }
                        if !attr_skip(&field.attrs) {
                            bound_accumulator.visit_type(&field.ty);
                        }
                    }
                }
            }
            syn::Data::Struct(struct_) => {
                for field in &struct_.fields {
                    for attr in &field.attrs {
                        result.parse_attr(attr, None, Some(field));
                    }
                    if !attr_skip(&field.attrs) {
                        bound_accumulator.visit_type(&field.ty);
                    }
                }
            }
            syn::Data::Union(union_) => panic!("Unsupported untagged union {:?}", union_),
        }

        result.auto_params = bound_accumulator.params;

        result
    }

    /// Parse attribute and handle `#[zeroize(...)]` attributes
    fn parse_attr(&mut self, attr: &Attribute, variant: Option<&Variant>, binding: Option<&Field>) {
        let meta_list = match &attr.meta {
            Meta::List(list) => list,
            _ => return,
        };

        // Ignore any non-zeroize attributes
        if !meta_list.path.is_ident(ZEROIZE_ATTR) {
            return;
        }

        for meta in attr
            .parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)
            .unwrap_or_else(|e| panic!("error parsing attribute: {:?} ({})", attr, e))
        {
            self.parse_meta(&meta, variant, binding);
        }
    }

    /// Parse `#[zeroize(...)]` attribute metadata (e.g. `drop`)
    fn parse_meta(&mut self, meta: &Meta, variant: Option<&Variant>, binding: Option<&Field>) {
        if meta.path().is_ident("drop") {
            assert!(!self.drop, "duplicate #[zeroize] drop flags");

            match (variant, binding) {
                (_variant, Some(_binding)) => {
                    // structs don't have a variant prefix, and only structs have bindings outside of a variant
                    let item_kind = match variant {
                        Some(_) => "enum",
                        None => "struct",
                    };
                    panic!(
                        concat!(
                            "The #[zeroize(drop)] attribute is not allowed on {} fields. ",
                            "Use it on the containing {} instead.",
                        ),
                        item_kind, item_kind,
                    )
                }
                (Some(_variant), None) => panic!(concat!(
                    "The #[zeroize(drop)] attribute is not allowed on enum variants. ",
                    "Use it on the containing enum instead.",
                )),
                (None, None) => (),
            };

            self.drop = true;
        } else if meta.path().is_ident("bound") {
            assert!(self.bound.is_none(), "duplicate #[zeroize] bound flags");

            match (variant, binding) {
                (_variant, Some(_binding)) => {
                    // structs don't have a variant prefix, and only structs have bindings outside of a variant
                    let item_kind = match variant {
                        Some(_) => "enum",
                        None => "struct",
                    };
                    panic!(
                        concat!(
                            "The #[zeroize(bound)] attribute is not allowed on {} fields. ",
                            "Use it on the containing {} instead.",
                        ),
                        item_kind, item_kind,
                    )
                }
                (Some(_variant), None) => panic!(concat!(
                    "The #[zeroize(bound)] attribute is not allowed on enum variants. ",
                    "Use it on the containing enum instead.",
                )),
                (None, None) => {
                    if let Meta::NameValue(meta_name_value) = meta {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(lit), ..
                        }) = &meta_name_value.value
                        {
                            if lit.value().is_empty() {
                                self.bound = Some(Bounds(Punctuated::new()));
                            } else {
                                self.bound = Some(lit.parse().unwrap_or_else(|e| {
                                    panic!("error parsing bounds: {:?} ({})", lit, e)
                                }));
                            }

                            return;
                        }
                    }

                    panic!(concat!(
                        "The #[zeroize(bound)] attribute expects a name-value syntax with a string literal value.",
                        "E.g. #[zeroize(bound = \"T: MyTrait\")]."
                    ))
                }
            }
        } else if meta.path().is_ident("skip") {
            if variant.is_none() && binding.is_none() {
                panic!(concat!(
                    "The #[zeroize(skip)] attribute is not allowed on a `struct` or `enum`. ",
                    "Use it on a field or variant instead.",
                ))
            }
        } else {
            panic!("unknown #[zeroize] attribute type: {:?}", meta.path());
        }
    }
}

fn field_ident(n: usize, field: &Field) -> Ident {
    if let Some(ref name) = field.ident {
        name.clone()
    } else {
        format_ident!("__zeroize_field_{}", n)
    }
}

fn generate_fields(input: &DeriveInput, method: TokenStream) -> TokenStream {
    let input_id = &input.ident;
    let fields: Vec<_> = match input.data {
        Data::Enum(ref enum_) => enum_
            .variants
            .iter()
            .filter_map(|variant| {
                if attr_skip(&variant.attrs) {
                    if variant.fields.iter().any(|field| attr_skip(&field.attrs)) {
                        panic!("duplicate #[zeroize] skip flags")
                    }
                    None
                } else {
                    let variant_id = &variant.ident;
                    Some((quote! { #input_id :: #variant_id }, &variant.fields))
                }
            })
            .collect(),
        Data::Struct(ref struct_) => vec![(quote! { #input_id }, &struct_.fields)],
        Data::Union(ref union_) => panic!("Cannot generate fields for untagged union {:?}", union_),
    };

    let arms = fields.into_iter().map(|(name, fields)| {
        let method_field = fields.iter().enumerate().filter_map(|(n, field)| {
            if attr_skip(&field.attrs) {
                None
            } else {
                let name = field_ident(n, field);
                Some(quote! { #name.#method() })
            }
        });

        let field_bindings = fields
            .iter()
            .enumerate()
            .map(|(n, field)| field_ident(n, field));

        let binding = match fields {
            Fields::Named(_) => quote! {
                #name { #(#field_bindings),* }
            },
            Fields::Unnamed(_) => quote! {
                #name ( #(#field_bindings),* )
            },
            Fields::Unit => quote! {
                #name
            },
        };

        quote! {
            #[allow(unused_variables)]
            #binding => {
                #(#method_field);*
            }
        }
    });

    quote! {
        match self {
            #(#arms),*
            _ => {}
        }
    }
}

fn attr_skip(attrs: &[Attribute]) -> bool {
    let mut result = false;
    for attr in attrs.iter().map(|attr| &attr.meta) {
        if let Meta::List(list) = attr {
            if list.path.is_ident(ZEROIZE_ATTR) {
                for meta in list
                    .parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)
                    .unwrap_or_else(|e| panic!("error parsing attribute: {:?} ({})", list, e))
                {
                    if let Meta::Path(path) = meta {
                        if path.is_ident("skip") {
                            assert!(!result, "duplicate #[zeroize] skip flags");
                            result = true;
                        }
                    }
                }
            }
        }
    }
    result
}

fn impl_zeroize_on_drop(input: &DeriveInput) -> TokenStream {
    let name = input.ident.clone();
    let (impl_gen, type_gen, where_) = input.generics.split_for_impl();
    quote! {
        #[doc(hidden)]
        impl #impl_gen ::zeroize::ZeroizeOnDrop for #name #type_gen #where_ {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn test_derive(
        f: impl Fn(DeriveInput) -> TokenStream,
        input: TokenStream,
        expected_output: TokenStream,
    ) {
        let output = f(syn::parse2(input).unwrap());
        assert_eq!(format!("{output}"), format!("{expected_output}"));
    }

    #[track_caller]
    fn parse_zeroize_test(unparsed: &str) -> TokenStream {
        derive_zeroize_impl(syn::parse_str(unparsed).expect("Failed to parse test input"))
    }

    #[test]
    fn zeroize_without_drop() {
        test_derive(
            derive_zeroize_impl,
            quote! {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                }
            },
            quote! {
                impl ::zeroize::Zeroize for Z {
                    fn zeroize(&mut self) {
                        match self {
                            #[allow(unused_variables)]
                            Z { a, b, c } => {
                                a.zeroize();
                                b.zeroize();
                                c.zeroize()
                            }
                            _ => {}
                        }
                    }
                }
            },
        )
    }

    #[test]
    fn zeroize_with_drop() {
        test_derive(
            derive_zeroize_impl,
            quote! {
                #[zeroize(drop)]
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                }
            },
            quote! {
                impl ::zeroize::Zeroize for Z {
                    fn zeroize(&mut self) {
                        match self {
                            #[allow(unused_variables)]
                            Z { a, b, c } => {
                                a.zeroize();
                                b.zeroize();
                                c.zeroize()
                            }
                            _ => {}
                        }
                    }
                }
                #[doc(hidden)]
                impl Drop for Z {
                    fn drop(&mut self) {
                        self.zeroize()
                    }
                }
            },
        )
    }

    #[test]
    fn zeroize_with_skip() {
        test_derive(
            derive_zeroize_impl,
            quote! {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    #[zeroize(skip)]
                    c: [u8; 3],
                }
            },
            quote! {
                impl ::zeroize::Zeroize for Z {
                    fn zeroize(&mut self) {
                        match self {
                            #[allow(unused_variables)]
                            Z { a, b, c } => {
                                a.zeroize();
                                b.zeroize()
                            }
                            _ => {}
                        }
                    }
                }
            },
        )
    }

    #[test]
    fn zeroize_with_bound() {
        test_derive(
            derive_zeroize_impl,
            quote! {
                #[zeroize(bound = "T: MyTrait")]
                struct Z<T>(T);
            },
            quote! {
                impl<T> ::zeroize::Zeroize for Z<T> where T: MyTrait {
                    fn zeroize(&mut self) {
                        match self {
                            #[allow(unused_variables)]
                            Z(__zeroize_field_0) => {
                                __zeroize_field_0.zeroize()
                            }
                            _ => {}
                        }
                    }
                }
            },
        )
    }

    #[test]
    fn zeroize_only_drop() {
        test_derive(
            derive_zeroize_on_drop_impl,
            quote! {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                }
            },
            quote! {
                impl Drop for Z {
                    fn drop(&mut self) {
                        use ::zeroize::__internal::AssertZeroize;
                        use ::zeroize::__internal::AssertZeroizeOnDrop;
                        match self {
                            #[allow(unused_variables)]
                            Z { a, b, c } => {
                                a.zeroize_or_on_drop();
                                b.zeroize_or_on_drop();
                                c.zeroize_or_on_drop()
                            }
                            _ => {}
                        }
                    }
                }
                #[doc(hidden)]
                impl ::zeroize::ZeroizeOnDrop for Z {}
            },
        )
    }

    #[test]
    fn zeroize_on_struct() {
        parse_zeroize_test(stringify!(
            #[zeroize(drop)]
            struct Z {
                a: String,
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    fn zeroize_on_enum() {
        parse_zeroize_test(stringify!(
            #[zeroize(drop)]
            enum Z {
                Variant1 { a: String, b: Vec<u8>, c: [u8; 3] },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on struct fields")]
    fn zeroize_on_struct_field() {
        parse_zeroize_test(stringify!(
            struct Z {
                #[zeroize(drop)]
                a: String,
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on struct fields")]
    fn zeroize_on_tuple_struct_field() {
        parse_zeroize_test(stringify!(
            struct Z(#[zeroize(drop)] String);
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on struct fields")]
    fn zeroize_on_second_field() {
        parse_zeroize_test(stringify!(
            struct Z {
                a: String,
                #[zeroize(drop)]
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum fields")]
    fn zeroize_on_tuple_enum_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant(#[zeroize(drop)] String),
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum fields")]
    fn zeroize_on_enum_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant {
                    #[zeroize(drop)]
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum fields")]
    fn zeroize_on_enum_second_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant1 {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                },
                Variant2 {
                    #[zeroize(drop)]
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum variants")]
    fn zeroize_on_enum_variant() {
        parse_zeroize_test(stringify!(
            enum Z {
                #[zeroize(drop)]
                Variant,
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum variants")]
    fn zeroize_on_enum_second_variant() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant1,
                #[zeroize(drop)]
                Variant2,
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(skip)] attribute is not allowed on a `struct` or `enum`. Use it on a field or variant instead."
    )]
    fn zeroize_skip_on_struct() {
        parse_zeroize_test(stringify!(
            #[zeroize(skip)]
            struct Z {
                a: String,
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(skip)] attribute is not allowed on a `struct` or `enum`. Use it on a field or variant instead."
    )]
    fn zeroize_skip_on_enum() {
        parse_zeroize_test(stringify!(
            #[zeroize(skip)]
            enum Z {
                Variant1,
                Variant2,
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] skip flags")]
    fn zeroize_duplicate_skip() {
        parse_zeroize_test(stringify!(
            struct Z {
                a: String,
                #[zeroize(skip)]
                #[zeroize(skip)]
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] skip flags")]
    fn zeroize_duplicate_skip_list() {
        parse_zeroize_test(stringify!(
            struct Z {
                a: String,
                #[zeroize(skip, skip)]
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] skip flags")]
    fn zeroize_duplicate_skip_enum() {
        parse_zeroize_test(stringify!(
            enum Z {
                #[zeroize(skip)]
                Variant {
                    a: String,
                    #[zeroize(skip)]
                    b: Vec<u8>,
                    c: [u8; 3],
                },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] bound flags")]
    fn zeroize_duplicate_bound() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound = "T: MyTrait")]
            #[zeroize(bound = "")]
            struct Z<T>(T);
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] bound flags")]
    fn zeroize_duplicate_bound_list() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound = "T: MyTrait", bound = "")]
            struct Z<T>(T);
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute is not allowed on struct fields. Use it on the containing struct instead."
    )]
    fn zeroize_bound_struct() {
        parse_zeroize_test(stringify!(
            struct Z<T> {
                #[zeroize(bound = "T: MyTrait")]
                a: T,
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute is not allowed on enum variants. Use it on the containing enum instead."
    )]
    fn zeroize_bound_enum() {
        parse_zeroize_test(stringify!(
            enum Z<T> {
                #[zeroize(bound = "T: MyTrait")]
                A(T),
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute is not allowed on enum fields. Use it on the containing enum instead."
    )]
    fn zeroize_bound_enum_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z<T> {
                A {
                    #[zeroize(bound = "T: MyTrait")]
                    a: T,
                },
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute expects a name-value syntax with a string literal value.E.g. #[zeroize(bound = \"T: MyTrait\")]."
    )]
    fn zeroize_bound_no_value() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound)]
            struct Z<T>(T);
        ));
    }

    #[test]
    #[should_panic(expected = "error parsing bounds: LitStr { token: \"T\" } (expected `:`)")]
    fn zeroize_bound_no_where_predicate() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound = "T")]
            struct Z<T>(T);
        ));
    }
}

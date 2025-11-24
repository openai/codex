use std::borrow::Cow;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::ast::{Data, Fields, Style};
use crate::codegen::{Field, OuterFromImpl, TraitImpl, Variant};
use crate::util::Callable;

pub struct FromMetaImpl<'a> {
    pub base: TraitImpl<'a>,
    pub from_word: Option<Cow<'a, Callable>>,
    pub from_none: Option<&'a Callable>,
    pub from_expr: Option<&'a Callable>,
    pub derive_syn_parse: bool,
}

impl ToTokens for FromMetaImpl<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let base = &self.base;

        let from_word = self.from_word.as_ref().map(|body| {
            quote_spanned! {body.span()=>
                fn from_word() -> ::darling::Result<Self> {
                    ::darling::export::identity::<fn() -> ::darling::Result<Self>>(#body)()
                }
            }
        });

        let from_none = self.from_none.map(|body| {
            quote_spanned! {body.span()=>
                fn from_none() -> ::darling::export::Option<Self> {
                    ::darling::export::identity::<fn() -> ::darling::export::Option<Self>>(#body)()
                }
            }
        });

        let from_expr = self.from_expr.map(|body| {
            quote_spanned! {body.span()=>
                fn from_expr(expr: &::darling::export::syn::Expr) -> ::darling::Result<Self> {
                    ::darling::export::identity::<fn(&::darling::export::syn::Expr) -> ::darling::Result<Self>>(#body)(expr)
                }
            }
        });

        let impl_block = match base.data {
            // Unit structs allow empty bodies only.
            Data::Struct(ref vd) if vd.style.is_unit() => {
                let ty_ident = base.ident;
                quote!(
                    fn from_word() -> ::darling::Result<Self> {
                        ::darling::export::Ok(#ty_ident)
                    }
                )
            }

            // Newtype structs proxy to the sole value they contain.
            Data::Struct(Fields {
                ref fields,
                style: Style::Tuple,
                ..
            }) if fields.len() == 1 => {
                let ty_ident = base.ident;
                quote!(
                    fn from_meta(__item: &::darling::export::syn::Meta) -> ::darling::Result<Self> {
                        ::darling::FromMeta::from_meta(__item)
                            .map_err(|e| e.with_span(&__item))
                            .map(#ty_ident)
                    }
                )
            }
            Data::Struct(Fields {
                style: Style::Tuple,
                ..
            }) => {
                panic!("Multi-field tuples are not supported");
            }
            Data::Struct(ref data) => {
                let inits = data.fields.iter().map(Field::as_initializer);
                let declare_errors = base.declare_errors();
                let require_fields = base.require_fields();
                let check_errors = base.check_errors();
                let decls = base.local_declarations();
                let core_loop = base.core_loop();
                let default = base.fallback_decl();
                let post_transform = base.post_transform_call();

                quote!(
                    #from_word

                    #from_none

                    #from_expr

                    fn from_list(__items: &[::darling::export::NestedMeta]) -> ::darling::Result<Self> {

                        #decls

                        #declare_errors

                        #core_loop

                        #require_fields

                        #check_errors

                        #default

                        ::darling::export::Ok(Self {
                            #(#inits),*
                        }) #post_transform
                    }
                )
            }
            Data::Enum(ref variants) => {
                let unit_arms = variants.iter().map(Variant::as_unit_match_arm);

                let (unknown_variant_err, unknown_unit_variant_err) = if !variants.is_empty() {
                    let names = variants.iter().map(Variant::as_name);
                    let names = quote!(&[#(#names),*]);
                    (
                        quote! {
                            unknown_field_with_alts(__other, #names)
                        },
                        quote! {
                            unknown_value_with_alts(__other, #names)
                        },
                    )
                } else {
                    (
                        quote! {
                            unknown_field(__other)
                        },
                        quote!(unknown_value(__other)),
                    )
                };

                let data_variants = variants.iter().map(Variant::as_data_match_arm);

                quote!(
                    fn from_list(__outer: &[::darling::export::NestedMeta]) -> ::darling::Result<Self> {
                        // An enum must have exactly one value inside the parentheses if it's not a unit
                        // match arm.
                        match __outer.len() {
                            0 => ::darling::export::Err(::darling::Error::too_few_items(1)),
                            1 => {
                                if let ::darling::export::NestedMeta::Meta(ref __nested) = __outer[0] {
                                    match ::darling::util::path_to_string(__nested.path()).as_ref() {
                                        #(#data_variants)*
                                        __other => ::darling::export::Err(::darling::Error::#unknown_variant_err.with_span(__nested))
                                    }
                                } else {
                                    ::darling::export::Err(::darling::Error::unsupported_format("literal"))
                                }
                            }
                            _ => ::darling::export::Err(::darling::Error::too_many_items(1)),
                        }
                    }

                    fn from_string(lit: &str) -> ::darling::Result<Self> {
                        match lit {
                            #(#unit_arms)*
                            __other => ::darling::export::Err(::darling::Error::#unknown_unit_variant_err)
                        }
                    }

                    #from_word

                    #from_none

                    #from_expr
                )
            }
        };

        self.wrap(impl_block, tokens);
        if self.derive_syn_parse {
            ParseImpl(self).to_tokens(tokens);
        }
    }
}

impl<'a> OuterFromImpl<'a> for FromMetaImpl<'a> {
    fn trait_path(&self) -> syn::Path {
        path!(::darling::FromMeta)
    }

    fn base(&'a self) -> &'a TraitImpl<'a> {
        &self.base
    }
}

struct ParseImpl<'a>(&'a FromMetaImpl<'a>);

impl<'a> OuterFromImpl<'a> for ParseImpl<'a> {
    fn trait_path(&self) -> syn::Path {
        path!(::darling::export::syn::parse::Parse)
    }

    fn base(&'a self) -> &'a TraitImpl<'a> {
        &self.0.base
    }

    fn trait_bound(&self) -> syn::Path {
        // Since the Parse impl delegates to FromMeta, that's the
        // trait bound we need to apply.
        self.0.trait_path()
    }
}

impl ToTokens for ParseImpl<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let from_meta = self.0.trait_path();
        let impl_block = quote! {
            fn parse(input: ::darling::export::syn::parse::ParseStream<'_>) -> ::darling::export::syn::Result<Self> {
                use ::darling::export::IntoIterator;

                let items = ::darling::export::syn::punctuated::Punctuated::<::darling::export::NestedMeta, ::darling::export::syn::Token![,]>::parse_terminated(input)?
                    .into_iter()
                    .collect::<::darling::export::Vec<_>>();
                <Self as #from_meta>::from_list(&items).map_err(::darling::export::Into::into)
            }
        };

        self.wrap(impl_block, tokens);
    }
}

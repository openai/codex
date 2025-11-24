#![recursion_limit = "2048"]
extern crate proc_macro;
#[macro_use]
extern crate quote;

use proc_macro2::{Span, TokenStream};
use std::convert::TryFrom;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    Expr, Ident, DeriveInput, Data, Token, Variant,
};

struct Flag<'a> {
    name: Ident,
    span: Span,
    value: FlagValue<'a>,
}

enum FlagValue<'a> {
    Literal(u128),
    Deferred,
    Inferred(&'a mut Variant),
}

impl FlagValue<'_> {
    fn is_inferred(&self) -> bool {
        matches!(self, FlagValue::Inferred(_))
    }
}

struct Parameters {
    default: Vec<Ident>,
}

impl Parse for Parameters {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        if input.is_empty() {
            return Ok(Parameters { default: vec![] });
        }

        input.parse::<Token![default]>()?;
        input.parse::<Token![=]>()?;
        let mut default = vec![input.parse()?];
        while !input.is_empty() {
            input.parse::<Token![|]>()?;
            default.push(input.parse()?);
        }

        Ok(Parameters { default })
    }
}

#[proc_macro_attribute]
pub fn bitflags_internal(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let Parameters { default } = parse_macro_input!(attr as Parameters);
    let mut ast = parse_macro_input!(input as DeriveInput);
    let output = gen_enumflags(&mut ast, default);

    output
        .unwrap_or_else(|err| {
            let error = err.to_compile_error();
            quote! {
                #ast
                #error
            }
        })
        .into()
}

/// Try to evaluate the expression given.
fn fold_expr(expr: &syn::Expr) -> Option<u128> {
    match expr {
        Expr::Lit(ref expr_lit) => match expr_lit.lit {
            syn::Lit::Int(ref lit_int) => lit_int.base10_parse().ok(),
            _ => None,
        },
        Expr::Binary(ref expr_binary) => {
            let l = fold_expr(&expr_binary.left)?;
            let r = fold_expr(&expr_binary.right)?;
            match &expr_binary.op {
                syn::BinOp::Shl(_) => u32::try_from(r).ok().and_then(|r| l.checked_shl(r)),
                _ => None,
            }
        }
        Expr::Paren(syn::ExprParen { expr, .. }) | Expr::Group(syn::ExprGroup { expr, .. }) => {
            fold_expr(expr)
        }
        _ => None,
    }
}

fn collect_flags<'a>(
    variants: impl Iterator<Item = &'a mut Variant>,
) -> Result<Vec<Flag<'a>>, syn::Error> {
    variants
        .map(|variant| {
            if !matches!(variant.fields, syn::Fields::Unit) {
                return Err(syn::Error::new_spanned(
                    &variant.fields,
                    "Bitflag variants cannot contain additional data",
                ));
            }

            let name = variant.ident.clone();
            let span = variant.span();
            let value = if let Some(ref expr) = variant.discriminant {
                if let Some(n) = fold_expr(&expr.1) {
                    FlagValue::Literal(n)
                } else {
                    FlagValue::Deferred
                }
            } else {
                FlagValue::Inferred(variant)
            };

            Ok(Flag { name, span, value })
        })
        .collect()
}

fn inferred_value(type_name: &Ident, previous_variants: &[Ident], repr: &Ident) -> Expr {
    let tokens = if previous_variants.is_empty() {
        quote!(1)
    } else {
        quote!(::enumflags2::_internal::next_bit(
                #(#type_name::#previous_variants as u128)|*
        ) as #repr)
    };

    syn::parse2(tokens).expect("couldn't parse inferred value")
}

fn infer_values(flags: &mut [Flag], type_name: &Ident, repr: &Ident) {
    let mut previous_variants: Vec<Ident> = flags
        .iter()
        .filter(|flag| !flag.value.is_inferred())
        .map(|flag| flag.name.clone())
        .collect();

    for flag in flags {
        if let FlagValue::Inferred(ref mut variant) = flag.value {
            variant.discriminant = Some((
                <Token![=]>::default(),
                inferred_value(type_name, &previous_variants, repr),
            ));
            previous_variants.push(flag.name.clone());
        }
    }
}

/// Given a list of attributes, find the `repr`, if any, and return the integer
/// type specified.
fn extract_repr(attrs: &[syn::Attribute]) -> Result<Option<Ident>, syn::Error> {
    let mut res = None;
    for attr in attrs {
        if attr.path().is_ident("repr") {
            attr.parse_nested_meta(|meta| {
                if let Some(ident) = meta.path.get_ident() {
                    res = Some(ident.clone());
                }
                Ok(())
            })?;
        }
    }
    Ok(res)
}

/// Check the repr and return the number of bits available
fn type_bits(ty: &Ident) -> Result<u8, syn::Error> {
    // This would be so much easier if we could just match on an Ident...
    if ty == "usize" {
        Err(syn::Error::new_spanned(
            ty,
            "#[repr(usize)] is not supported. Use u32 or u64 instead.",
        ))
    } else if ty == "i8"
        || ty == "i16"
        || ty == "i32"
        || ty == "i64"
        || ty == "i128"
        || ty == "isize"
    {
        Err(syn::Error::new_spanned(
            ty,
            "Signed types in a repr are not supported.",
        ))
    } else if ty == "u8" {
        Ok(8)
    } else if ty == "u16" {
        Ok(16)
    } else if ty == "u32" {
        Ok(32)
    } else if ty == "u64" {
        Ok(64)
    } else if ty == "u128" {
        Ok(128)
    } else {
        Err(syn::Error::new_spanned(
            ty,
            "repr must be an integer type for #[bitflags].",
        ))
    }
}

/// Returns deferred checks
fn check_flag(type_name: &Ident, flag: &Flag, bits: u8) -> Result<Option<TokenStream>, syn::Error> {
    use FlagValue::*;
    match flag.value {
        Literal(n) => {
            if !n.is_power_of_two() {
                Err(syn::Error::new(
                    flag.span,
                    "Flags must have exactly one set bit",
                ))
            } else if bits < 128 && n >= 1 << bits {
                Err(syn::Error::new(
                    flag.span,
                    format!("Flag value out of range for u{}", bits),
                ))
            } else {
                Ok(None)
            }
        }
        Inferred(_) => Ok(None),
        Deferred => {
            let variant_name = &flag.name;
            Ok(Some(quote_spanned!(flag.span =>
                const _:
                    <<[(); (
                        (#type_name::#variant_name as u128).is_power_of_two()
                    ) as usize] as ::enumflags2::_internal::AssertionHelper>
                        ::Status as ::enumflags2::_internal::ExactlyOneBitSet>::X
                    = ();
            )))
        }
    }
}

fn gen_enumflags(ast: &mut DeriveInput, default: Vec<Ident>) -> Result<TokenStream, syn::Error> {
    let ident = &ast.ident;

    let span = Span::call_site();

    let ast_variants = match &mut ast.data {
        Data::Enum(ref mut data) => &mut data.variants,
        Data::Struct(data) => {
            return Err(syn::Error::new_spanned(&data.struct_token,
                "expected enum for #[bitflags], found struct"));
        }
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(&data.union_token,
                "expected enum for #[bitflags], found union"));
        }
    };

    if ast.generics.lt_token.is_some() || ast.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(&ast.generics,
            "bitflags cannot be generic"));
    }

    let repr = extract_repr(&ast.attrs)?
        .ok_or_else(|| syn::Error::new_spanned(ident,
                        "repr attribute missing. Add #[repr(u64)] or a similar attribute to specify the size of the bitfield."))?;
    let bits = type_bits(&repr)?;

    let mut variants = collect_flags(ast_variants.iter_mut())?;
    let deferred = variants
        .iter()
        .flat_map(|variant| check_flag(ident, variant, bits).transpose())
        .collect::<Result<Vec<_>, _>>()?;

    infer_values(&mut variants, ident, &repr);

    if (bits as usize) < variants.len() {
        return Err(syn::Error::new_spanned(
            &repr,
            format!("Not enough bits for {} flags", variants.len()),
        ));
    }

    let std = quote_spanned!(span => ::enumflags2::_internal::core);
    let ast_variants = match &ast.data {
        Data::Enum(ref data) => &data.variants,
        _ => unreachable!(),
    };

    let variant_names = ast_variants.iter().map(|v| &v.ident).collect::<Vec<_>>();

    Ok(quote_spanned! {
        span =>
            #ast
            #(#deferred)*
            impl #std::ops::Not for #ident {
                type Output = ::enumflags2::BitFlags<Self>;
                #[inline(always)]
                fn not(self) -> Self::Output {
                    use ::enumflags2::BitFlags;
                    BitFlags::from_flag(self).not()
                }
            }

            impl #std::ops::BitOr for #ident {
                type Output = ::enumflags2::BitFlags<Self>;
                #[inline(always)]
                fn bitor(self, other: Self) -> Self::Output {
                    use ::enumflags2::BitFlags;
                    BitFlags::from_flag(self) | other
                }
            }

            impl #std::ops::BitAnd for #ident {
                type Output = ::enumflags2::BitFlags<Self>;
                #[inline(always)]
                fn bitand(self, other: Self) -> Self::Output {
                    use ::enumflags2::BitFlags;
                    BitFlags::from_flag(self) & other
                }
            }

            impl #std::ops::BitXor for #ident {
                type Output = ::enumflags2::BitFlags<Self>;
                #[inline(always)]
                fn bitxor(self, other: Self) -> Self::Output {
                    use ::enumflags2::BitFlags;
                    BitFlags::from_flag(self) ^ other
                }
            }

            unsafe impl ::enumflags2::_internal::RawBitFlags for #ident {
                type Numeric = #repr;

                const EMPTY: <Self as ::enumflags2::_internal::RawBitFlags>::Numeric = 0;

                const DEFAULT: <Self as ::enumflags2::_internal::RawBitFlags>::Numeric =
                    0 #(| (Self::#default as #repr))*;

                const ALL_BITS: <Self as ::enumflags2::_internal::RawBitFlags>::Numeric =
                    0 #(| (Self::#variant_names as #repr))*;

                const BITFLAGS_TYPE_NAME : &'static str =
                    concat!("BitFlags<", stringify!(#ident), ">");

                fn bits(self) -> <Self as ::enumflags2::_internal::RawBitFlags>::Numeric {
                    self as #repr
                }
            }

            impl ::enumflags2::BitFlag for #ident {}
    })
}

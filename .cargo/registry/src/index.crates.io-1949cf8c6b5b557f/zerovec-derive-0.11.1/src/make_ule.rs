// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::utils::{self, FieldInfo, ZeroVecAttrs};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::{parse_quote, Data, DataEnum, DataStruct, DeriveInput, Error, Expr, Fields, Ident, Lit};

pub fn make_ule_impl(ule_name: Ident, mut input: DeriveInput) -> TokenStream2 {
    if input.generics.type_params().next().is_some()
        || input.generics.lifetimes().next().is_some()
        || input.generics.const_params().next().is_some()
    {
        return Error::new(
            input.generics.span(),
            "#[make_ule] must be applied to a struct without any generics",
        )
        .to_compile_error();
    }
    let sp = input.span();
    let attrs = match utils::extract_attributes_common(&mut input.attrs, sp, false) {
        Ok(val) => val,
        Err(e) => return e.to_compile_error(),
    };

    let name = &input.ident;

    let ule_stuff = match input.data {
        Data::Struct(ref s) => make_ule_struct_impl(name, &ule_name, &input, s, &attrs),
        Data::Enum(ref e) => make_ule_enum_impl(name, &ule_name, &input, e, &attrs),
        _ => {
            return Error::new(input.span(), "#[make_ule] must be applied to a struct")
                .to_compile_error();
        }
    };

    let zmkv = if attrs.skip_kv {
        quote!()
    } else {
        quote!(
            impl<'a> zerovec::maps::ZeroMapKV<'a> for #name {
                type Container = zerovec::ZeroVec<'a, #name>;
                type Slice = zerovec::ZeroSlice<#name>;
                type GetType = #ule_name;
                type OwnedType = #name;
            }
        )
    };

    let maybe_debug = if attrs.debug {
        quote!(
            impl core::fmt::Debug for #ule_name {
                fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                    let this = <#name as zerovec::ule::AsULE>::from_unaligned(*self);
                    <#name as core::fmt::Debug>::fmt(&this, f)
                }
            }
        )
    } else {
        quote!()
    };

    quote!(
        #input

        #ule_stuff

        #maybe_debug

        #zmkv
    )
}

fn make_ule_enum_impl(
    name: &Ident,
    ule_name: &Ident,
    input: &DeriveInput,
    enu: &DataEnum,
    attrs: &ZeroVecAttrs,
) -> TokenStream2 {
    // We could support more int reprs in the future if needed
    if !utils::ReprInfo::compute(&input.attrs).u8 {
        return Error::new(
            input.span(),
            "#[make_ule] can only be applied to #[repr(u8)] enums",
        )
        .to_compile_error();
    }

    if enu.variants.is_empty() {
        return Error::new(input.span(), "#[make_ule] cannot be applied to empty enums")
            .to_compile_error();
    }

    // the smallest discriminant seen
    let mut min = None;
    // the largest discriminant seen
    let mut max = None;
    // Discriminants that have not been found in series (we might find them later)
    let mut not_found = HashSet::new();

    for (i, variant) in enu.variants.iter().enumerate() {
        if !matches!(variant.fields, Fields::Unit) {
            // This can be supported in the future, see zerovec/design_doc.md
            return Error::new(
                variant.span(),
                "#[make_ule] can only be applied to enums with dataless variants",
            )
            .to_compile_error();
        }

        if let Some((_, ref discr)) = variant.discriminant {
            if let Some(n) = get_expr_int(discr) {
                let n = match u8::try_from(n) {
                    Ok(n) => n,
                    Err(_) => {
                        return Error::new(
                            variant.span(),
                            "#[make_ule] only supports discriminants from 0 to 255",
                        )
                        .to_compile_error();
                    }
                };
                match min {
                    Some(x) if x < n => {}
                    _ => {
                        min = Some(n);
                    }
                }
                match max {
                    Some(x) if x >= n => {}
                    _ => {
                        let old_max = max.unwrap_or(0u8);
                        for missing in (old_max + 1)..n {
                            not_found.insert(missing);
                        }
                        max = Some(n);
                    }
                }

                not_found.remove(&n);

                // We require explicit discriminants so that it is clear that reordering
                // fields would be a breaking change. Furthermore, using explicit discriminants helps ensure that
                // platform-specific C ABI choices do not matter.
                // We could potentially add in explicit discriminants on the user's behalf in the future, or support
                // more complicated sets of explicit discriminant values.
                if n as usize != i {}
            } else {
                return Error::new(
                    discr.span(),
                    "#[make_ule] must be applied to enums with explicit integer discriminants",
                )
                .to_compile_error();
            }
        } else {
            return Error::new(
                variant.span(),
                "#[make_ule] must be applied to enums with explicit discriminants",
            )
            .to_compile_error();
        }
    }

    let not_found = not_found.iter().collect::<Vec<_>>();
    let min = min.unwrap();
    let max = max.unwrap();

    if not_found.len() > min as usize {
        return Error::new(input.span(), format!("#[make_ule] must be applied to enums with discriminants \
                                                  filling the range from a minimum to a maximum; could not find {not_found:?}"))
            .to_compile_error();
    }

    let maybe_ord_derives = if attrs.skip_ord {
        quote!()
    } else {
        quote!(#[derive(Ord, PartialOrd)])
    };

    let vis = &input.vis;

    let doc = format!("[`ULE`](zerovec::ule::ULE) type for {name}");

    // Safety (based on the safety checklist on the ULE trait):
    //  1. ULE type does not include any uninitialized or padding bytes.
    //     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant
    //  2. ULE type is aligned to 1 byte.
    //     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
    //  3. The impl of validate_bytes() returns an error if any byte is not valid.
    //     (Guarantees that the byte is in range of the corresponding enum.)
    //  4. The impl of validate_bytes() returns an error if there are extra bytes.
    //     (This does not happen since we are backed by 1 byte.)
    //  5. The other ULE methods use the default impl.
    //  6. ULE type byte equality is semantic equality
    quote!(
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, Eq)]
        #maybe_ord_derives
        #[doc = #doc]
        #vis struct #ule_name(u8);

        unsafe impl zerovec::ule::ULE for #ule_name {
            #[inline]
            fn validate_bytes(bytes: &[u8]) -> Result<(), zerovec::ule::UleError> {
                for byte in bytes {
                    if *byte < #min || *byte > #max {
                        return Err(zerovec::ule::UleError::parse::<Self>())
                    }
                }
                Ok(())
            }
        }

        impl zerovec::ule::AsULE for #name {
            type ULE = #ule_name;

            fn to_unaligned(self) -> Self::ULE {
                // safety: the enum is repr(u8) and can be cast to a u8
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            fn from_unaligned(other: Self::ULE) -> Self {
                // safety: the enum is repr(u8) and can be cast from a u8,
                // and `#ule_name` guarantees a valid value for this enum.
                unsafe {
                    ::core::mem::transmute(other)
                }
            }
        }

        impl #name {
            /// Attempt to construct the value from its corresponding integer,
            /// returning `None` if not possible
            pub(crate) fn new_from_u8(value: u8) -> Option<Self> {
                if value <= #max {
                    unsafe {
                        Some(::core::mem::transmute(value))
                    }
                } else {
                    None
                }
            }
        }
    )
}

fn get_expr_int(e: &Expr) -> Option<u64> {
    if let Ok(Lit::Int(ref i)) = syn::parse2(quote!(#e)) {
        return i.base10_parse().ok();
    }

    None
}

fn make_ule_struct_impl(
    name: &Ident,
    ule_name: &Ident,
    input: &DeriveInput,
    struc: &DataStruct,
    attrs: &ZeroVecAttrs,
) -> TokenStream2 {
    if struc.fields.iter().next().is_none() {
        return Error::new(
            input.span(),
            "#[make_ule] must be applied to a non-empty struct",
        )
        .to_compile_error();
    }
    let sized_fields = FieldInfo::make_list(struc.fields.iter());
    let field_inits = crate::ule::make_ule_fields(&sized_fields);
    let field_inits = utils::wrap_field_inits(&field_inits, &struc.fields);

    let semi = utils::semi_for(&struc.fields);
    let repr_attr = utils::repr_for(&struc.fields);
    let vis = &input.vis;

    let doc = format!("[`ULE`](zerovec::ule::ULE) type for [`{name}`]");

    let ule_struct: DeriveInput = parse_quote!(
        #[repr(#repr_attr)]
        #[derive(Copy, Clone, PartialEq, Eq)]
        #[doc = #doc]
        // We suppress the `missing_docs` lint for the fields of the struct.
        #[allow(missing_docs)]
        #vis struct #ule_name #field_inits #semi
    );
    let derived = crate::ule::derive_impl(&ule_struct);

    let mut as_ule_conversions = vec![];
    let mut from_ule_conversions = vec![];

    for (i, field) in struc.fields.iter().enumerate() {
        let ty = &field.ty;
        let i = syn::Index::from(i);
        if let Some(ref ident) = field.ident {
            as_ule_conversions
                .push(quote!(#ident: <#ty as zerovec::ule::AsULE>::to_unaligned(self.#ident)));
            from_ule_conversions.push(
                quote!(#ident: <#ty as zerovec::ule::AsULE>::from_unaligned(unaligned.#ident)),
            );
        } else {
            as_ule_conversions.push(quote!(<#ty as zerovec::ule::AsULE>::to_unaligned(self.#i)));
            from_ule_conversions
                .push(quote!(<#ty as zerovec::ule::AsULE>::from_unaligned(unaligned.#i)));
        };
    }

    let as_ule_conversions = utils::wrap_field_inits(&as_ule_conversions, &struc.fields);
    let from_ule_conversions = utils::wrap_field_inits(&from_ule_conversions, &struc.fields);
    let asule_impl = quote!(
        impl zerovec::ule::AsULE for #name {
            type ULE = #ule_name;
            fn to_unaligned(self) -> Self::ULE {
                #ule_name #as_ule_conversions
            }
            fn from_unaligned(unaligned: Self::ULE) -> Self {
                Self #from_ule_conversions
            }
        }
    );

    let maybe_ord_impls = if attrs.skip_ord {
        quote!()
    } else {
        quote!(
            impl core::cmp::PartialOrd for #ule_name {
                fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl core::cmp::Ord for #ule_name {
                fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                    let this = <#name as zerovec::ule::AsULE>::from_unaligned(*self);
                    let other = <#name as zerovec::ule::AsULE>::from_unaligned(*other);
                    <#name as core::cmp::Ord>::cmp(&this, &other)
                }
            }
        )
    };

    let maybe_hash = if attrs.hash {
        quote!(
            #[allow(clippy::derive_hash_xor_eq)]
            impl core::hash::Hash for #ule_name {
                fn hash<H>(&self, state: &mut H) where H: core::hash::Hasher {
                    state.write(<#ule_name as zerovec::ule::ULE>::slice_as_bytes(&[*self]));
                }
            }
        )
    } else {
        quote!()
    };

    quote!(
        #asule_impl

        #ule_struct

        #derived

        #maybe_ord_impls

        #maybe_hash
    )
}

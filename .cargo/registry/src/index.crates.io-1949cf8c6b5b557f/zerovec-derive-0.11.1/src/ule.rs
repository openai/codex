// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::utils::{self, FieldInfo};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error};

pub fn derive_impl(input: &DeriveInput) -> TokenStream2 {
    if !utils::ReprInfo::compute(&input.attrs).cpacked_or_transparent() {
        return Error::new(
            input.span(),
            "derive(ULE) must be applied to a #[repr(C, packed)] or #[repr(transparent)] type",
        )
        .to_compile_error();
    }
    if input.generics.type_params().next().is_some()
        || input.generics.lifetimes().next().is_some()
        || input.generics.const_params().next().is_some()
    {
        return Error::new(
            input.generics.span(),
            "derive(ULE) must be applied to a struct without any generics",
        )
        .to_compile_error();
    }
    let struc = if let Data::Struct(ref s) = input.data {
        if s.fields.iter().next().is_none() {
            return Error::new(
                input.span(),
                "derive(ULE) must be applied to a non-empty struct",
            )
            .to_compile_error();
        }
        s
    } else {
        return Error::new(input.span(), "derive(ULE) must be applied to a struct")
            .to_compile_error();
    };

    let fields = FieldInfo::make_list(struc.fields.iter());
    let (validators, remaining_offset) = generate_ule_validators(&fields);

    let name = &input.ident;

    // Safety (based on the safety checklist on the ULE trait):
    //  1. #name does not include any uninitialized or padding bytes.
    //     (achieved by enforcing #[repr(transparent)] or #[repr(C, packed)] on a struct of only ULE types)
    //  2. #name is aligned to 1 byte.
    //     (achieved by enforcing #[repr(transparent)] or #[repr(C, packed)] on a struct of only ULE types)
    //  3. The impl of validate_bytes() returns an error if any byte is not valid.
    //  4. The impl of validate_bytes() returns an error if there are extra bytes.
    //  5. The other ULE methods use the default impl.
    //  6. [This impl does not enforce the non-safety equality constraint, it is up to the user to do so, ideally via a custom derive]
    quote! {
        unsafe impl zerovec::ule::ULE for #name {
            #[inline]
            fn validate_bytes(bytes: &[u8]) -> Result<(), zerovec::ule::UleError> {
                const SIZE: usize = ::core::mem::size_of::<#name>();
                #[allow(clippy::modulo_one)]
                if bytes.len() % SIZE != 0 {
                    return Err(zerovec::ule::UleError::length::<Self>(bytes.len()));
                }
                // Validate the bytes
                #[allow(clippy::indexing_slicing)] // We're slicing a chunk of known size
                for chunk in bytes.chunks_exact(SIZE) {
                    #validators
                    debug_assert_eq!(#remaining_offset, SIZE);
                }
                Ok(())
            }
        }
    }
}

/// Given an slice over ULE struct fields, returns code validating that a slice variable `bytes` contains valid instances of those ULE types
/// in order, plus the byte offset of any remaining unvalidated bytes. ULE types should not have any remaining bytes, but VarULE types will since
/// the last field is the unsized one.
pub(crate) fn generate_ule_validators(
    fields: &[FieldInfo],
    // (validators, remaining_offset)
) -> (TokenStream2, syn::Ident) {
    utils::generate_per_field_offsets(fields, false, |field, prev_offset_ident, size_ident| {
        let ty = &field.field.ty;
        quote! {
            if let Some(bytes) = bytes.get(#prev_offset_ident .. #prev_offset_ident + #size_ident) {
                <#ty as zerovec::ule::ULE>::validate_bytes(bytes)?;
            } else {
                return Err(zerovec::ule::UleError::parse::<Self>());
            }
        }
    })
}

/// Make corresponding ULE fields for each field
pub(crate) fn make_ule_fields(fields: &[FieldInfo]) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|f| {
            let ty = &f.field.ty;
            let ty = quote!(<#ty as zerovec::ule::AsULE>::ULE);
            let setter = f.setter();
            let vis = &f.field.vis;
            quote!(#vis #setter #ty)
        })
        .collect::<Vec<_>>()
}

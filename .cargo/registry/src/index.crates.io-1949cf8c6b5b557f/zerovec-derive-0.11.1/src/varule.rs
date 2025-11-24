// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::utils::{self, FieldInfo};
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Ident};

/// Implementation for derive(VarULE). `custom_varule_validator` validates the last field bytes `last_field_bytes`
/// if specified, if not, the VarULE implementation will be used.
pub fn derive_impl(
    input: &DeriveInput,
    custom_varule_validator: Option<TokenStream2>,
) -> TokenStream2 {
    if !utils::ReprInfo::compute(&input.attrs).cpacked_or_transparent() {
        return Error::new(
            input.span(),
            "derive(VarULE) must be applied to a #[repr(C, packed)] or #[repr(transparent)] type",
        )
        .to_compile_error();
    }
    if input.generics.type_params().next().is_some()
        || input.generics.lifetimes().next().is_some()
        || input.generics.const_params().next().is_some()
    {
        return Error::new(
            input.generics.span(),
            "derive(VarULE) must be applied to a struct without any generics",
        )
        .to_compile_error();
    }
    let struc = if let Data::Struct(ref s) = input.data {
        if s.fields.iter().next().is_none() {
            return Error::new(
                input.span(),
                "derive(VarULE) must be applied to a non-empty struct",
            )
            .to_compile_error();
        }
        s
    } else {
        return Error::new(input.span(), "derive(VarULE) must be applied to a struct")
            .to_compile_error();
    };

    let n_fields = struc.fields.len();

    let ule_fields = FieldInfo::make_list(struc.fields.iter().take(n_fields - 1));

    let sizes = ule_fields.iter().map(|f| {
        let ty = &f.field.ty;
        quote!(::core::mem::size_of::<#ty>())
    });
    let (validators, remaining_offset) = if n_fields > 1 {
        // generate ULE validators
        crate::ule::generate_ule_validators(&ule_fields)
    } else {
        // no ULE subfields
        (
            quote!(
                const ZERO: usize = 0;
            ),
            Ident::new("ZERO", Span::call_site()),
        )
    };

    let unsized_field = &struc
        .fields
        .iter()
        .next_back()
        .expect("Already verified that struct is not empty")
        .ty;

    let name = &input.ident;
    let ule_size = Ident::new(
        &format!("__IMPL_VarULE_FOR_{name}_ULE_SIZE"),
        Span::call_site(),
    );

    let last_field_validator = if let Some(custom_varule_validator) = custom_varule_validator {
        custom_varule_validator
    } else {
        quote!(<#unsized_field as zerovec::ule::VarULE>::validate_bytes(last_field_bytes)?;)
    };

    // Safety (based on the safety checklist on the ULE trait):
    //  1. #name does not include any uninitialized or padding bytes
    //     (achieved by enforcing #[repr(transparent)] or #[repr(C, packed)] on a struct of only ULE types)
    //  2. #name is aligned to 1 byte.
    //     (achieved by enforcing #[repr(transparent)] or #[repr(C, packed)] on a struct of only ULE types)
    //  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
    //  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
    //  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
    //  6. The other VarULE methods use the default impl
    //  7. [This impl does not enforce the non-safety equality constraint, it is up to the user to do so, ideally via a custom derive]
    quote! {
        // The size of the ULE section of this type
        const #ule_size: usize = 0 #(+ #sizes)*;
        unsafe impl zerovec::ule::VarULE for #name {
            #[inline]
            fn validate_bytes(bytes: &[u8]) -> Result<(), zerovec::ule::UleError> {
                debug_assert_eq!(#remaining_offset, #ule_size);

                let Some(last_field_bytes) = bytes.get(#remaining_offset..) else {
                    return Err(zerovec::ule::UleError::parse::<Self>());
                };
                #validators
                #last_field_validator
                Ok(())
            }
            #[inline]
            unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
                // just the unsized part
                // Safety: The invariants of this function allow us to assume bytes is valid, and
                // having at least #ule_size bytes is a validity constraint for the ULE type.
                let unsized_bytes = bytes.get_unchecked(#ule_size..);
                let unsized_ref = <#unsized_field as zerovec::ule::VarULE>::from_bytes_unchecked(unsized_bytes);
                // We should use the pointer metadata APIs here when they are stable: https://github.com/rust-lang/rust/issues/81513
                // For now we rely on all DST metadata being a usize to extract it via a fake slice pointer
                let (_ptr, metadata): (usize, usize) = ::core::mem::transmute(unsized_ref);
                let entire_struct_as_slice: *const [u8] = ::core::slice::from_raw_parts(bytes.as_ptr(), metadata);
                &*(entire_struct_as_slice as *const Self)
            }
        }
    }
}

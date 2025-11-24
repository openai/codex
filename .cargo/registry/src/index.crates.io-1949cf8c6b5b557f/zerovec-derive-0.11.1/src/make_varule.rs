// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::utils::{self, FieldInfo};
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{
    parse_quote, Data, DeriveInput, Error, Field, Fields, GenericArgument, Ident, Lifetime,
    PathArguments, Type, TypePath,
};

pub fn make_varule_impl(ule_name: Ident, mut input: DeriveInput) -> TokenStream2 {
    if input.generics.type_params().next().is_some()
        || input.generics.const_params().next().is_some()
        || input.generics.lifetimes().count() > 1
    {
        return Error::new(
            input.generics.span(),
            "#[make_varule] must be applied to a struct without any type or const parameters and at most one lifetime",
        )
        .to_compile_error();
    }

    let sp = input.span();
    let attrs = match utils::extract_attributes_common(&mut input.attrs, sp, true) {
        Ok(val) => val,
        Err(e) => return e.to_compile_error(),
    };

    let lt = input.generics.lifetimes().next();

    if let Some(lt) = lt {
        if lt.colon_token.is_some() || !lt.bounds.is_empty() {
            return Error::new(
                input.generics.span(),
                "#[make_varule] must be applied to a struct without lifetime bounds",
            )
            .to_compile_error();
        }
    }

    let lt = lt.map(|l| &l.lifetime);

    let name = &input.ident;
    let input_span = input.span();

    let fields = match input.data {
        Data::Struct(ref mut s) => &mut s.fields,
        _ => {
            return Error::new(input.span(), "#[make_varule] must be applied to a struct")
                .to_compile_error();
        }
    };

    if fields.is_empty() {
        return Error::new(
            input.span(),
            "#[make_varule] must be applied to a struct with at least one field",
        )
        .to_compile_error();
    }

    let mut sized_fields = vec![];
    let mut unsized_fields = vec![];

    let mut custom_varule_idents = vec![];

    for field in fields.iter_mut() {
        match utils::extract_field_attributes(&mut field.attrs) {
            Ok(i) => custom_varule_idents.push(i),
            Err(e) => return e.to_compile_error(),
        }
    }

    for (i, field) in fields.iter().enumerate() {
        match UnsizedField::new(field, i, custom_varule_idents[i].clone()) {
            Ok(o) => unsized_fields.push(o),
            Err(_) => sized_fields.push(FieldInfo::new_for_field(field, i)),
        }
    }

    if unsized_fields.is_empty() {
        let last_field_index = fields.len() - 1;
        let last_field = fields.iter().next_back().unwrap();

        let e = UnsizedField::new(
            last_field,
            last_field_index,
            custom_varule_idents[last_field_index].clone(),
        )
        .unwrap_err();
        return Error::new(last_field.span(), e).to_compile_error();
    }

    if unsized_fields[0].field.index != fields.len() - unsized_fields.len()
        && unsized_fields[0].field.field.ident.is_none()
    {
        return Error::new(
            unsized_fields.first().unwrap().field.field.span(),
            "#[make_varule] requires its unsized fields to be at the end for tuple structs",
        )
        .to_compile_error();
    }

    let unsized_field_info = UnsizedFields::new(unsized_fields, attrs.vzv_format);

    let mut field_inits = crate::ule::make_ule_fields(&sized_fields);
    let last_field_ule = unsized_field_info.varule_ty();

    let setter = unsized_field_info.varule_setter();
    let vis = &unsized_field_info.varule_vis();
    field_inits.push(quote!(#vis #setter #last_field_ule));

    let semi = utils::semi_for(fields);
    let repr_attr = utils::repr_for(fields);
    let field_inits = utils::wrap_field_inits(&field_inits, fields);
    let vis = &input.vis;

    let doc = format!(
        "[`VarULE`](zerovec::ule::VarULE) type for [`{name}`]. See [`{name}`] for documentation."
    );
    let varule_struct: DeriveInput = parse_quote!(
        #[repr(#repr_attr)]
        #[doc = #doc]
        #[allow(missing_docs)]
        #vis struct #ule_name #field_inits #semi
    );

    let derived = crate::varule::derive_impl(&varule_struct, unsized_field_info.varule_validator());

    let maybe_lt_bound = lt.as_ref().map(|lt| quote!(<#lt>));

    let encode_impl = make_encode_impl(
        &sized_fields,
        &unsized_field_info,
        name,
        &ule_name,
        &maybe_lt_bound,
    );

    let zf_and_from_impl = make_zf_and_from_impl(
        &sized_fields,
        &unsized_field_info,
        fields,
        name,
        &ule_name,
        lt,
        input_span,
        attrs.skip_from,
    );

    let eq_impl = quote!(
        impl core::cmp::PartialEq for #ule_name {
            fn eq(&self, other: &Self) -> bool {
                // The VarULE invariants allow us to assume that equality is byte equality
                // in non-safety-critical contexts
                <Self as zerovec::ule::VarULE>::as_bytes(&self)
                == <Self as zerovec::ule::VarULE>::as_bytes(&other)
            }
        }

        impl core::cmp::Eq for #ule_name {}
    );

    let zerofrom_fq_path =
        quote!(<#name as zerovec::__zerovec_internal_reexport::ZeroFrom<#ule_name>>);

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
                    let this = #zerofrom_fq_path::zero_from(self);
                    let other = #zerofrom_fq_path::zero_from(other);
                    <#name as core::cmp::Ord>::cmp(&this, &other)
                }
            }
        )
    };

    let maybe_debug = if attrs.debug {
        quote!(
            impl core::fmt::Debug for #ule_name {
                fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                    let this = #zerofrom_fq_path::zero_from(self);
                    <#name as core::fmt::Debug>::fmt(&this, f)
                }
            }
        )
    } else {
        quote!()
    };

    let maybe_toowned = if !attrs.skip_toowned {
        quote!(
            impl zerovec::__zerovec_internal_reexport::borrow::ToOwned for #ule_name {
                type Owned = zerovec::__zerovec_internal_reexport::boxed::Box<Self>;
                fn to_owned(&self) -> Self::Owned {
                    zerovec::ule::encode_varule_to_box(self)
                }
            }
        )
    } else {
        quote!()
    };

    let zmkv = if attrs.skip_kv {
        quote!()
    } else {
        quote!(
            impl<'a> zerovec::maps::ZeroMapKV<'a> for #ule_name {
                type Container = zerovec::VarZeroVec<'a, #ule_name>;
                type Slice = zerovec::VarZeroSlice<#ule_name>;
                type GetType = #ule_name;
                type OwnedType = zerovec::__zerovec_internal_reexport::boxed::Box<#ule_name>;
            }
        )
    };

    let serde_path = quote!(zerovec::__zerovec_internal_reexport::serde);

    let maybe_ser = if attrs.serialize {
        quote!(
            impl #serde_path::Serialize for #ule_name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: #serde_path::Serializer {
                    if serializer.is_human_readable() {
                        let this = #zerofrom_fq_path::zero_from(self);
                        <#name as #serde_path::Serialize>::serialize(&this, serializer)
                    } else {
                        serializer.serialize_bytes(zerovec::ule::VarULE::as_bytes(self))
                    }
                }
            }
        )
    } else {
        quote!()
    };

    let deserialize_error = format!("&{ule_name} can only deserialize in zero-copy ways");

    let maybe_de = if attrs.deserialize {
        quote!(
            impl<'de> #serde_path::Deserialize<'de> for zerovec::__zerovec_internal_reexport::boxed::Box<#ule_name> {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: #serde_path::Deserializer<'de> {
                    if deserializer.is_human_readable() {
                        let this = <#name as #serde_path::Deserialize>::deserialize(deserializer)?;
                        Ok(zerovec::ule::encode_varule_to_box(&this))
                    } else {
                        // This branch should usually not be hit, since Cow-like use cases will hit the Deserialize impl for &'a ULEType instead.
                        let deserialized = <& #ule_name>::deserialize(deserializer)?;
                        Ok(zerovec::ule::VarULE::to_boxed(deserialized))
                    }
                }
            }
            impl<'a, 'de: 'a> #serde_path::Deserialize<'de> for &'a #ule_name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: #serde_path::Deserializer<'de> {
                    if !deserializer.is_human_readable() {
                        let bytes = <&[u8]>::deserialize(deserializer)?;
                        <#ule_name as zerovec::ule::VarULE>::parse_bytes(bytes).map_err(#serde_path::de::Error::custom)
                    } else {
                        Err(#serde_path::de::Error::custom(#deserialize_error))
                    }
                }
            }
        )
    } else {
        quote!()
    };

    let maybe_hash = if attrs.hash {
        quote!(
            #[allow(clippy::derive_hash_xor_eq)]
            impl core::hash::Hash for #ule_name {
                fn hash<H>(&self, state: &mut H) where H: core::hash::Hasher {
                    state.write(<#ule_name as zerovec::ule::VarULE>::as_bytes(&self));
                }
            }
        )
    } else {
        quote!()
    };

    let maybe_multi_getters = if let Some(getters) = unsized_field_info.maybe_multi_getters() {
        quote! {
            impl #ule_name {
                #getters
            }
        }
    } else {
        quote!()
    };

    quote!(
        #input

        #varule_struct

        #maybe_multi_getters

        #encode_impl

        #zf_and_from_impl

        #derived

        #maybe_ord_impls

        #eq_impl

        #zmkv

        #maybe_ser

        #maybe_de

        #maybe_debug

        #maybe_toowned

        #maybe_hash
    )
}

#[allow(clippy::too_many_arguments)] // Internal function. Could refactor later to use some kind of context type.
fn make_zf_and_from_impl(
    sized_fields: &[FieldInfo],
    unsized_field_info: &UnsizedFields,
    fields: &Fields,
    name: &Ident,
    ule_name: &Ident,
    maybe_lt: Option<&Lifetime>,
    span: Span,
    skip_from: bool,
) -> TokenStream2 {
    if !unsized_field_info.has_zf() {
        return quote!();
    }

    let lt = if let Some(ref lt) = maybe_lt {
        lt
    } else {
        return Error::new(
            span,
            "Can only generate ZeroFrom impls for types with lifetimes",
        )
        .to_compile_error();
    };

    let mut field_inits = sized_fields
        .iter()
        .map(|f| {
            let ty = &f.field.ty;
            let accessor = &f.accessor;
            let setter = f.setter();
            quote!(#setter <#ty as zerovec::ule::AsULE>::from_unaligned(other.#accessor))
        })
        .collect::<Vec<_>>();

    unsized_field_info.push_zf_setters(lt, &mut field_inits);

    let field_inits = utils::wrap_field_inits(&field_inits, fields);
    let zerofrom_trait = quote!(zerovec::__zerovec_internal_reexport::ZeroFrom);

    let maybe_from = if skip_from {
        quote!()
    } else {
        quote!(
            impl<#lt> From<&#lt #ule_name> for #name<#lt> {
                fn from(other: &#lt #ule_name) -> Self {
                    <Self as #zerofrom_trait<#lt, #ule_name>>::zero_from(other)
                }
            }
        )
    };
    quote!(
        impl <#lt> #zerofrom_trait <#lt, #ule_name> for #name <#lt> {
            fn zero_from(other: &#lt #ule_name) -> Self {
                Self #field_inits
            }
        }

        #maybe_from
    )
}

fn make_encode_impl(
    sized_fields: &[FieldInfo],
    unsized_field_info: &UnsizedFields,
    name: &Ident,
    ule_name: &Ident,
    maybe_lt_bound: &Option<TokenStream2>,
) -> TokenStream2 {
    let mut lengths = vec![];

    for field in sized_fields {
        let ty = &field.field.ty;
        lengths.push(quote!(::core::mem::size_of::<<#ty as zerovec::ule::AsULE>::ULE>()));
    }

    let (encoders, remaining_offset) = utils::generate_per_field_offsets(
        sized_fields,
        true,
        |field, prev_offset_ident, size_ident| {
            let ty = &field.field.ty;
            let accessor = &field.accessor;
            quote!(
                // generate_per_field_offsets produces valid indices,
                // and we don't care about panics in Encode impls
                #[allow(clippy::indexing_slicing)]
                let out = &mut dst[#prev_offset_ident .. #prev_offset_ident + #size_ident];
                let unaligned = zerovec::ule::AsULE::to_unaligned(self.#accessor);
                let unaligned_slice = &[unaligned];
                let src = <<#ty as zerovec::ule::AsULE>::ULE as zerovec::ule::ULE>::slice_as_bytes(unaligned_slice);
                out.copy_from_slice(src);
            )
        },
    );

    let last_encode_len = unsized_field_info.encode_len();
    let last_encode_write = unsized_field_info.encode_write(quote!(out));
    quote!(
        unsafe impl #maybe_lt_bound zerovec::ule::EncodeAsVarULE<#ule_name> for #name #maybe_lt_bound {
            // Safety: unimplemented as the other two are implemented
            fn encode_var_ule_as_slices<R>(&self, cb: impl FnOnce(&[&[u8]]) -> R) -> R {
                unreachable!("other two methods implemented")
            }

            // Safety: returns the total length of the ULE form by adding up the lengths of each element's ULE forms
            fn encode_var_ule_len(&self) -> usize {
                #(#lengths +)* #last_encode_len
            }

            // Safety: converts each element to ULE form and writes them in sequence
            fn encode_var_ule_write(&self, mut dst: &mut [u8]) {
                debug_assert_eq!(self.encode_var_ule_len(), dst.len());
                #encoders


                // generate_per_field_offsets produces valid remainders,
                // and we don't care about panics in Encode impls
                #[allow(clippy::indexing_slicing)]
                let out = &mut dst[#remaining_offset..];
                #last_encode_write
            }
        }

        // This second impl exists to allow for using EncodeAsVarULE without cloning
        //
        // A blanket impl cannot exist without coherence issues
        unsafe impl #maybe_lt_bound zerovec::ule::EncodeAsVarULE<#ule_name> for &'_ #name #maybe_lt_bound {
            // Safety: unimplemented as the other two are implemented
            fn encode_var_ule_as_slices<R>(&self, cb: impl FnOnce(&[&[u8]]) -> R) -> R {
                unreachable!("other two methods implemented")
            }

            // Safety: returns the total length of the ULE form by adding up the lengths of each element's ULE forms
            fn encode_var_ule_len(&self) -> usize {
                (**self).encode_var_ule_len()
            }

            // Safety: converts each element to ULE form and writes them in sequence
            fn encode_var_ule_write(&self, mut dst: &mut [u8]) {
                (**self).encode_var_ule_write(dst)
            }
        }
    )
}

/// Represents a VarULE-compatible type that would typically
/// be found behind a `Cow<'a, _>` in the last field, and is represented
/// roughly the same in owned and borrowed versions
#[derive(Copy, Clone, Debug)]
enum OwnULETy<'a> {
    /// [T] where T: AsULE<ULE = Self>
    Slice(&'a Type),
    /// str
    Str,
}

/// Represents the type of the last field of the struct
#[derive(Clone, Debug)]
enum UnsizedFieldKind<'a> {
    Cow(OwnULETy<'a>),
    VarZeroCow(OwnULETy<'a>),
    ZeroVec(&'a Type),
    VarZeroVec(&'a Type),
    /// Custom VarULE type, and the identifier corresponding to the VarULE type
    Custom(&'a TypePath, Ident),

    // Generally you should be using the above ones for maximum zero-copy, but these will still work
    Growable(OwnULETy<'a>),
    Boxed(OwnULETy<'a>),
    Ref(OwnULETy<'a>),
}

#[derive(Clone, Debug)]
struct UnsizedField<'a> {
    kind: UnsizedFieldKind<'a>,
    field: FieldInfo<'a>,
}

struct UnsizedFields<'a> {
    fields: Vec<UnsizedField<'a>>,
    format_param: TokenStream2,
}

impl<'a> UnsizedFields<'a> {
    /// The format_param is an optional tokenstream describing a VZVFormat argument needed by MultiFieldsULE
    fn new(fields: Vec<UnsizedField<'a>>, format_param: Option<TokenStream2>) -> Self {
        assert!(!fields.is_empty(), "Must have at least one unsized field");

        let format_param = format_param.unwrap_or_else(|| quote!(zerovec::vecs::Index16));
        Self {
            fields,
            format_param,
        }
    }

    // Get the corresponding VarULE type that can store all of these
    fn varule_ty(&self) -> TokenStream2 {
        let len = self.fields.len();
        let format_param = &self.format_param;
        if len == 1 {
            self.fields[0].kind.varule_ty()
        } else {
            quote!(zerovec::ule::MultiFieldsULE::<#len, #format_param>)
        }
    }

    // Get the accessor field name in the VarULE type
    fn varule_accessor(&self) -> TokenStream2 {
        if self.fields.len() == 1 {
            self.fields[0].field.accessor.clone()
        } else if self.fields[0].field.field.ident.is_some() {
            quote!(unsized_fields)
        } else {
            // first unsized field
            self.fields[0].field.accessor.clone()
        }
    }

    // Get the setter for this type for use in struct definition/creation syntax
    fn varule_setter(&self) -> TokenStream2 {
        if self.fields.len() == 1 {
            self.fields[0].field.setter()
        } else if self.fields[0].field.field.ident.is_some() {
            quote!(unsized_fields: )
        } else {
            quote!()
        }
    }

    fn varule_vis(&self) -> TokenStream2 {
        if self.fields.len() == 1 {
            self.fields[0].field.field.vis.to_token_stream()
        } else {
            // Always private
            quote!()
        }
    }

    // Check if the type has a ZeroFrom impl
    fn has_zf(&self) -> bool {
        self.fields.iter().all(|f| f.kind.has_zf())
    }

    // Takes all unsized fields on self and encodes them into a byte slice `out`
    fn encode_write(&self, out: TokenStream2) -> TokenStream2 {
        let len = self.fields.len();
        let format_param = &self.format_param;
        if len == 1 {
            self.fields[0].encode_func(quote!(encode_var_ule_write), quote!(#out))
        } else {
            let mut lengths = vec![];
            let mut writers = vec![];
            for (i, field) in self.fields.iter().enumerate() {
                lengths.push(field.encode_func(quote!(encode_var_ule_len), quote!()));
                let (encodeable_ty, encodeable) = field.encodeable_tokens();
                let varule_ty = field.kind.varule_ty();
                writers
                    .push(quote!(multi.set_field_at::<#varule_ty, #encodeable_ty>(#i, #encodeable)))
            }

            quote!(
                let lengths = [#(#lengths),*];
                // Todo: index type should be settable by attribute
                let mut multi = zerovec::ule::MultiFieldsULE::<#len, #format_param>::new_from_lengths_partially_initialized(lengths, #out);
                unsafe {
                    #(#writers;)*
                }
            )
        }
    }

    // Takes all unsized fields on self and returns the length needed for encoding into a byte slice
    fn encode_len(&self) -> TokenStream2 {
        let len = self.fields.len();
        let format_param = &self.format_param;
        if len == 1 {
            self.fields[0].encode_func(quote!(encode_var_ule_len), quote!())
        } else {
            let mut lengths = vec![];
            for field in self.fields.iter() {
                lengths.push(field.encode_func(quote!(encode_var_ule_len), quote!()));
            }
            // Todo: index type should be settable by attribute
            quote!(zerovec::ule::MultiFieldsULE::<#len, #format_param>::compute_encoded_len_for([#(#lengths),*]))
        }
    }

    /// Constructs ZeroFrom setters for each field of the stack type
    fn push_zf_setters(&self, lt: &Lifetime, field_inits: &mut Vec<TokenStream2>) {
        let zerofrom_trait = quote!(zerovec::__zerovec_internal_reexport::ZeroFrom);
        if self.fields.len() == 1 {
            let accessor = self.fields[0].field.accessor.clone();
            let setter = self.fields[0].field.setter();
            let last_field_ty = &self.fields[0].field.field.ty;
            let last_field_ule_ty = self.fields[0].kind.varule_ty();
            field_inits.push(quote!(#setter <#last_field_ty as #zerofrom_trait <#lt, #last_field_ule_ty>>::zero_from(&other.#accessor) ));
        } else {
            for field in self.fields.iter() {
                let setter = field.field.setter();
                let getter = field.field.getter();
                let field_ty = &field.field.field.ty;
                let field_ule_ty = field.kind.varule_ty();

                field_inits.push(quote!(#setter
                    <#field_ty as #zerofrom_trait <#lt, #field_ule_ty>>::zero_from(&other.#getter())
                ));
            }
        }
    }

    fn maybe_multi_getters(&self) -> Option<TokenStream2> {
        if self.fields.len() == 1 {
            None
        } else {
            let multi_accessor = self.varule_accessor();
            let field_getters = self.fields.iter().enumerate().map(|(i, field)| {
                let getter = field.field.getter();

                let field_ule_ty = field.kind.varule_ty();
                let doc_name = field.field.getter_doc_name();
                let doc = format!("Access the VarULE type behind {doc_name}");
                quote!(
                    #[doc = #doc]
                    pub fn #getter<'a>(&'a self) -> &'a #field_ule_ty {
                        unsafe {
                            self.#multi_accessor.get_field::<#field_ule_ty>(#i)
                        }
                    }
                )
            });

            Some(quote!(#(#field_getters)*))
        }
    }

    /// In case this needs custom validation code, return it
    ///
    /// The code will validate a variable known as `last_field_bytes`
    fn varule_validator(&self) -> Option<TokenStream2> {
        let len = self.fields.len();
        let format_param = &self.format_param;
        if len == 1 {
            None
        } else {
            let mut validators = vec![];
            for (i, field) in self.fields.iter().enumerate() {
                let varule_ty = field.kind.varule_ty();
                validators.push(quote!(multi.validate_field::<#varule_ty>(#i)?;));
            }

            Some(quote!(
                let multi = zerovec::ule::MultiFieldsULE::<#len, #format_param>::parse_bytes(last_field_bytes)?;
                unsafe {
                    #(#validators)*
                }
            ))
        }
    }
}

impl<'a> UnsizedField<'a> {
    fn new(
        field: &'a Field,
        index: usize,
        custom_varule_ident: Option<Ident>,
    ) -> Result<Self, String> {
        Ok(UnsizedField {
            kind: UnsizedFieldKind::new(&field.ty, custom_varule_ident)?,
            field: FieldInfo::new_for_field(field, index),
        })
    }

    /// Call `<Self as EncodeAsVarULE<V>>::#method(self.accessor #additional_args)` after adjusting
    /// Self and self.accessor to be the right types
    fn encode_func(&self, method: TokenStream2, additional_args: TokenStream2) -> TokenStream2 {
        let encodeas_trait = quote!(zerovec::ule::EncodeAsVarULE);
        let (encodeable_ty, encodeable) = self.encodeable_tokens();
        let varule_ty = self.kind.varule_ty();
        quote!(<#encodeable_ty as #encodeas_trait<#varule_ty>>::#method(#encodeable, #additional_args))
    }

    /// Returns (encodeable_ty, encodeable)
    fn encodeable_tokens(&self) -> (TokenStream2, TokenStream2) {
        let accessor = self.field.accessor.clone();
        let value = quote!(self.#accessor);
        let encodeable = self.kind.encodeable_value(value);
        let encodeable_ty = self.kind.encodeable_ty();
        (encodeable_ty, encodeable)
    }
}

impl<'a> UnsizedFieldKind<'a> {
    /// Construct a UnsizedFieldKind for the type of a UnsizedFieldKind if possible
    fn new(
        ty: &'a Type,
        custom_varule_ident: Option<Ident>,
    ) -> Result<UnsizedFieldKind<'a>, String> {
        static PATH_TYPE_IDENTITY_ERROR: &str =
            "Can only automatically detect corresponding VarULE types for path types \
            that are Cow, ZeroVec, VarZeroVec, Box, String, or Vec";
        static PATH_TYPE_GENERICS_ERROR: &str =
            "Can only automatically detect corresponding VarULE types for path \
            types with at most one lifetime and at most one generic parameter. VarZeroVecFormat
            types are not currently supported";
        match *ty {
            Type::Reference(ref tyref) => OwnULETy::new(&tyref.elem, "reference").map(UnsizedFieldKind::Ref),
            Type::Path(ref typath) => {
                if let Some(custom_varule_ident) = custom_varule_ident {
                    return Ok(UnsizedFieldKind::Custom(typath, custom_varule_ident));
                }
                if typath.path.segments.len() != 1 {
                    return Err("Can only automatically detect corresponding VarULE types for \
                                path types with a single path segment".into());
                }
                let segment = typath.path.segments.first().unwrap();
                match segment.arguments {
                    PathArguments::None => {
                        if segment.ident == "String" {
                            Ok(UnsizedFieldKind::Growable(OwnULETy::Str))
                        } else {
                            Err(PATH_TYPE_IDENTITY_ERROR.into())
                        }
                    }
                    PathArguments::AngleBracketed(ref params) => {
                        // At most one lifetime and exactly one generic parameter
                        let mut lifetime = None;
                        let mut generic = None;
                        for param in &params.args {
                            match param {
                                GenericArgument::Lifetime(ref lt) if lifetime.is_none() => {
                                    lifetime = Some(lt)
                                }
                                GenericArgument::Type(ref ty) if generic.is_none() => {
                                    generic = Some(ty)
                                }
                                _ => return Err(PATH_TYPE_GENERICS_ERROR.into()),
                            }
                        }

                        // Must be exactly one generic parameter
                        // (we've handled the zero generics case already)
                        let generic = if let Some(g) = generic {
                            g
                        } else {
                            return Err(PATH_TYPE_GENERICS_ERROR.into());
                        };

                        let ident = segment.ident.to_string();

                        if lifetime.is_some() {
                            match &*ident {
                                "ZeroVec" => Ok(UnsizedFieldKind::ZeroVec(generic)),
                                "VarZeroVec" => Ok(UnsizedFieldKind::VarZeroVec(generic)),
                                "Cow" => OwnULETy::new(generic, "Cow").map(UnsizedFieldKind::Cow),
                                "VarZeroCow" => OwnULETy::new(generic, "VarZeroCow").map(UnsizedFieldKind::VarZeroCow),
                                _ => Err(PATH_TYPE_IDENTITY_ERROR.into()),
                            }
                        } else {
                            match &*ident {
                                "Vec" => Ok(UnsizedFieldKind::Growable(OwnULETy::Slice(generic))),
                                "Box" => OwnULETy::new(generic, "Box").map(UnsizedFieldKind::Boxed),
                                _ => Err(PATH_TYPE_IDENTITY_ERROR.into()),
                            }
                        }
                    }
                    _ => Err("Can only automatically detect corresponding VarULE types for path types \
                              with none or angle bracketed generics".into()),
                }
            }
            _ => Err("Can only automatically detect corresponding VarULE types for path and reference types".into()),
        }
    }
    /// Get the tokens for the corresponding VarULE type
    fn varule_ty(&self) -> TokenStream2 {
        match *self {
            Self::Ref(ref inner)
            | Self::Cow(ref inner)
            | Self::VarZeroCow(ref inner)
            | Self::Boxed(ref inner)
            | Self::Growable(ref inner) => {
                let inner_ule = inner.varule_ty();
                quote!(#inner_ule)
            }
            Self::Custom(_, ref name) => quote!(#name),
            Self::ZeroVec(ref inner) => quote!(zerovec::ZeroSlice<#inner>),
            Self::VarZeroVec(ref inner) => quote!(zerovec::VarZeroSlice<#inner>),
        }
    }

    // Takes expr `value` and returns it as a value that can be encoded via EncodeAsVarULE
    fn encodeable_value(&self, value: TokenStream2) -> TokenStream2 {
        match *self {
            Self::Ref(_)
            | Self::Cow(_)
            | Self::VarZeroCow(_)
            | Self::Growable(_)
            | Self::Boxed(_) => quote!(&*#value),

            Self::Custom(..) => quote!(&#value),
            Self::ZeroVec(_) | Self::VarZeroVec(_) => quote!(&*#value),
        }
    }

    /// Returns the EncodeAsVarULE type this can be represented as, the same returned by encodeable_value()
    fn encodeable_ty(&self) -> TokenStream2 {
        match *self {
            Self::Ref(ref inner)
            | Self::Cow(ref inner)
            | Self::VarZeroCow(ref inner)
            | Self::Growable(ref inner)
            | Self::Boxed(ref inner) => inner.varule_ty(),

            Self::Custom(ref path, _) => quote!(#path),
            Self::ZeroVec(ref ty) => quote!(zerovec::ZeroSlice<#ty>),
            Self::VarZeroVec(ref ty) => quote!(zerovec::VarZeroSlice<#ty>),
        }
    }

    fn has_zf(&self) -> bool {
        matches!(
            *self,
            Self::Ref(_)
                | Self::Cow(_)
                | Self::VarZeroCow(_)
                | Self::ZeroVec(_)
                | Self::VarZeroVec(_)
                | Self::Custom(..)
        )
    }
}

impl<'a> OwnULETy<'a> {
    fn new(ty: &'a Type, context: &str) -> Result<Self, String> {
        match *ty {
            Type::Slice(ref slice) => Ok(OwnULETy::Slice(&slice.elem)),
            Type::Path(ref typath) => {
                if typath.path.is_ident("str") {
                    Ok(OwnULETy::Str)
                } else {
                    Err(format!("Cannot automatically detect corresponding VarULE type for non-str path type inside a {context}"))
                }
            }
            _ => Err(format!("Cannot automatically detect corresponding VarULE type for non-slice/path type inside a {context}")),
        }
    }

    /// Get the tokens for the corresponding VarULE type
    fn varule_ty(&self) -> TokenStream2 {
        match *self {
            OwnULETy::Slice(s) => quote!([#s]),
            OwnULETy::Str => quote!(str),
        }
    }
}

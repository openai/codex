// Cleanup when workspace lints can be overridden
// https://github.com/rust-lang/cargo/issues/13157
#![forbid(unsafe_code)]
#![warn(missing_copy_implementations, missing_debug_implementations)]
#![doc(test(attr(
    // Problematic handling for foreign From<T> impls in tests
    // https://github.com/rust-lang/rust/issues/121621
    allow(unknown_lints, non_local_definitions),
    deny(
        missing_debug_implementations,
        rust_2018_idioms,
        trivial_casts,
        trivial_numeric_casts,
        unused_extern_crates,
        unused_import_braces,
        unused_qualifications,
        warnings,
    ),
    forbid(unsafe_code),
)))]
// Not needed for 2018 edition and conflicts with `rust_2018_idioms`
#![doc(test(no_crate_inject))]
#![doc(html_root_url = "https://docs.rs/serde_with_macros/3.14.0/")]
// Tarpaulin does not work well with proc macros and marks most of the lines as uncovered.
#![cfg(not(tarpaulin_include))]

//! proc-macro extensions for [`serde_with`].
//!
//! This crate should **NEVER** be used alone.
//! All macros **MUST** be used via the re-exports in the [`serde_with`] crate.
//!
//! [`serde_with`]: https://crates.io/crates/serde_with/

mod apply;
mod lazy_bool;
mod utils;

use crate::utils::{
    split_with_de_lifetime, DeriveOptions, IteratorExt as _, SchemaFieldCondition,
    SchemaFieldConfig,
};
use darling::{
    ast::NestedMeta,
    util::{Flag, Override},
    Error as DarlingError, FromField, FromMeta,
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::Parser,
    parse_macro_input, parse_quote,
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
    DeriveInput, Error, Field, Fields, GenericArgument, ItemEnum, ItemStruct, Meta, Path,
    PathArguments, ReturnType, Token, Type,
};

/// Apply function on every field of structs or enums
fn apply_function_to_struct_and_enum_fields<F>(
    input: TokenStream,
    function: F,
) -> Result<TokenStream2, Error>
where
    F: Copy,
    F: Fn(&mut Field) -> Result<(), String>,
{
    /// Handle a single struct or a single enum variant
    fn apply_on_fields<F>(fields: &mut Fields, function: F) -> Result<(), Error>
    where
        F: Fn(&mut Field) -> Result<(), String>,
    {
        match fields {
            // simple, no fields, do nothing
            Fields::Unit => Ok(()),
            Fields::Named(fields) => fields
                .named
                .iter_mut()
                .map(|field| function(field).map_err(|err| Error::new(field.span(), err)))
                .collect_error(),
            Fields::Unnamed(fields) => fields
                .unnamed
                .iter_mut()
                .map(|field| function(field).map_err(|err| Error::new(field.span(), err)))
                .collect_error(),
        }
    }

    // For each field in the struct given by `input`, add the `skip_serializing_if` attribute,
    // if and only if, it is of type `Option`
    if let Ok(mut input) = syn::parse::<ItemStruct>(input.clone()) {
        apply_on_fields(&mut input.fields, function)?;
        Ok(quote!(#input))
    } else if let Ok(mut input) = syn::parse::<ItemEnum>(input) {
        input
            .variants
            .iter_mut()
            .map(|variant| apply_on_fields(&mut variant.fields, function))
            .collect_error()?;
        Ok(quote!(#input))
    } else {
        Err(Error::new(
            Span::call_site(),
            "The attribute can only be applied to struct or enum definitions.",
        ))
    }
}

/// Like [`apply_function_to_struct_and_enum_fields`] but for darling errors
fn apply_function_to_struct_and_enum_fields_darling<F>(
    input: TokenStream,
    serde_with_crate_path: &Path,
    function: F,
) -> Result<TokenStream2, DarlingError>
where
    F: Copy,
    F: Fn(&mut Field) -> Result<(), DarlingError>,
{
    /// Handle a single struct or a single enum variant
    fn apply_on_fields<F>(fields: &mut Fields, function: F) -> Result<(), DarlingError>
    where
        F: Fn(&mut Field) -> Result<(), DarlingError>,
    {
        match fields {
            // simple, no fields, do nothing
            Fields::Unit => Ok(()),
            Fields::Named(ref mut fields) => {
                let errors: Vec<DarlingError> = fields
                    .named
                    .iter_mut()
                    .map(|field| function(field).map_err(|err| err.with_span(&field)))
                    // turn the Err variant into the Some, such that we only collect errors
                    .filter_map(std::result::Result::err)
                    .collect();
                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(DarlingError::multiple(errors))
                }
            }
            Fields::Unnamed(fields) => {
                let errors: Vec<DarlingError> = fields
                    .unnamed
                    .iter_mut()
                    .map(|field| function(field).map_err(|err| err.with_span(&field)))
                    // turn the Err variant into the Some, such that we only collect errors
                    .filter_map(std::result::Result::err)
                    .collect();
                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(DarlingError::multiple(errors))
                }
            }
        }
    }

    // Add a dummy derive macro which consumes (makes inert) all field attributes
    let consume_serde_as_attribute = parse_quote!(
        #[derive(#serde_with_crate_path::__private_consume_serde_as_attributes)]
    );

    // For each field in the struct given by `input`, add the `skip_serializing_if` attribute,
    // if and only if, it is of type `Option`
    if let Ok(mut input) = syn::parse::<ItemStruct>(input.clone()) {
        apply_on_fields(&mut input.fields, function)?;
        input.attrs.push(consume_serde_as_attribute);
        Ok(quote!(#input))
    } else if let Ok(mut input) = syn::parse::<ItemEnum>(input) {
        // Prevent serde_as on enum variants
        let mut errors: Vec<DarlingError> = input
            .variants
            .iter()
            .flat_map(|variant| {
                variant.attrs.iter().filter_map(|attr| {
                    if attr.path().is_ident("serde_as") {
                        Some(
                            DarlingError::custom(
                                "serde_as attribute is not allowed on enum variants",
                            )
                            .with_span(&attr),
                        )
                    } else {
                        None
                    }
                })
            })
            .collect();
        // Process serde_as on all fields
        errors.extend(
            input
                .variants
                .iter_mut()
                .map(|variant| apply_on_fields(&mut variant.fields, function))
                // turn the Err variant into the Some, such that we only collect errors
                .filter_map(std::result::Result::err),
        );

        if errors.is_empty() {
            input.attrs.push(consume_serde_as_attribute);
            Ok(quote!(#input))
        } else {
            Err(DarlingError::multiple(errors))
        }
    } else {
        Err(DarlingError::custom(
            "The attribute can only be applied to struct or enum definitions.",
        )
        .with_span(&Span::call_site()))
    }
}

/// Add `skip_serializing_if` annotations to [`Option`] fields.
///
/// The attribute can be added to structs and enums.
/// The `#[skip_serializing_none]` attribute must be placed *before* the `#[derive]` attribute.
///
/// # Example
///
/// JSON APIs sometimes have many optional values.
/// Missing values should not be serialized, to keep the serialized format smaller.
/// Such a data type might look like:
///
/// ```rust
/// # use serde::Serialize;
/// #
/// # #[allow(dead_code)]
/// #[derive(Serialize)]
/// struct Data {
///     #[serde(skip_serializing_if = "Option::is_none")]
///     a: Option<String>,
///     #[serde(skip_serializing_if = "Option::is_none")]
///     b: Option<u64>,
///     #[serde(skip_serializing_if = "Option::is_none")]
///     c: Option<String>,
///     #[serde(skip_serializing_if = "Option::is_none")]
///     d: Option<bool>,
/// }
/// ```
///
/// The `skip_serializing_if` annotation is repetitive and harms readability.
/// Instead, the same struct can be written as:
///
/// ```rust
/// # use serde::Serialize;
/// # use serde_with_macros::skip_serializing_none;
/// #
/// # #[allow(dead_code)]
/// #[skip_serializing_none]
/// #[derive(Serialize)]
/// struct Data {
///     a: Option<String>,
///     b: Option<u64>,
///     c: Option<String>,
///     // Always serialize field d even if None
///     #[serialize_always]
///     d: Option<bool>,
/// }
/// ```
///
/// Existing `skip_serializing_if` annotations will not be altered.
///
/// If some values should always be serialized, then `serialize_always` can be used.
///
/// # Limitations
///
/// The `serialize_always` cannot be used together with a manual `skip_serializing_if` annotations,
/// as these conflict in their meaning. A compile error will be generated if this occurs.
///
/// The `skip_serializing_none` only works if the type is called `Option`,
/// `std::option::Option`, or `core::option::Option`. Type aliasing an [`Option`] and giving it
/// another name, will cause this field to be ignored. This cannot be supported, as proc-macros run
/// before type checking, thus it is not possible to determine if a type alias refers to an
/// [`Option`].
///
/// ```rust
/// # use serde::Serialize;
/// # use serde_with_macros::skip_serializing_none;
/// # #[allow(dead_code)]
/// type MyOption<T> = Option<T>;
///
/// # #[allow(dead_code)]
/// #[skip_serializing_none]
/// #[derive(Serialize)]
/// struct Data {
///     a: MyOption<String>, // This field will not be skipped
/// }
/// ```
///
/// Likewise, if you import a type and name it `Option`, the `skip_serializing_if` attributes will
/// be added and compile errors will occur, if `Option::is_none` is not a valid function.
/// Here the function `Vec::is_none` does not exist, and therefore the example fails to compile.
///
/// ```rust,compile_fail
/// # use serde::Serialize;
/// # use serde_with_macros::skip_serializing_none;
/// use std::vec::Vec as Option;
///
/// #[skip_serializing_none]
/// #[derive(Serialize)]
/// struct Data {
///     a: Option<String>,
/// }
/// ```
#[proc_macro_attribute]
pub fn skip_serializing_none(_args: TokenStream, input: TokenStream) -> TokenStream {
    let res =
        apply_function_to_struct_and_enum_fields(input, skip_serializing_none_add_attr_to_field)
            .unwrap_or_else(|err| err.to_compile_error());
    TokenStream::from(res)
}

/// Add the `skip_serializing_if` annotation to each field of the struct
fn skip_serializing_none_add_attr_to_field(field: &mut Field) -> Result<(), String> {
    if is_std_option(&field.ty) {
        let has_skip_serializing_if = field_has_attribute(field, "serde", "skip_serializing_if");

        // Remove the `serialize_always` attribute
        let mut has_always_attr = false;
        field.attrs.retain(|attr| {
            let has_attr = attr.path().is_ident("serialize_always");
            has_always_attr |= has_attr;
            !has_attr
        });

        // Error on conflicting attributes
        if has_always_attr && has_skip_serializing_if {
            let mut msg = r#"The attributes `serialize_always` and `serde(skip_serializing_if = "...")` cannot be used on the same field"#.to_string();
            if let Some(ident) = &field.ident {
                msg += ": `";
                msg += &ident.to_string();
                msg += "`";
            }
            msg += ".";
            return Err(msg);
        }

        // Do nothing if `skip_serializing_if` or `serialize_always` is already present
        if has_skip_serializing_if || has_always_attr {
            return Ok(());
        }

        // Add the `skip_serializing_if` attribute
        let attr = parse_quote!(
            #[serde(skip_serializing_if = "Option::is_none")]
        );
        field.attrs.push(attr);
    } else {
        // Warn on use of `serialize_always` on non-Option fields
        let has_attr = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("serialize_always"));
        if has_attr {
            return Err("`serialize_always` may only be used on fields of type `Option`.".into());
        }
    }
    Ok(())
}

/// Return `true`, if the type path refers to `std::option::Option`
///
/// Accepts
///
/// * `Option`
/// * `std::option::Option`, with or without leading `::`
/// * `core::option::Option`, with or without leading `::`
fn is_std_option(type_: &Type) -> bool {
    match type_ {
        Type::Array(_)
        | Type::BareFn(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Never(_)
        | Type::Ptr(_)
        | Type::Reference(_)
        | Type::Slice(_)
        | Type::TraitObject(_)
        | Type::Tuple(_)
        | Type::Verbatim(_) => false,

        Type::Group(syn::TypeGroup { elem, .. })
        | Type::Paren(syn::TypeParen { elem, .. })
        | Type::Path(syn::TypePath {
            qself: Some(syn::QSelf { ty: elem, .. }),
            ..
        }) => is_std_option(elem),

        Type::Path(syn::TypePath { qself: None, path }) => {
            (path.leading_colon.is_none()
                && path.segments.len() == 1
                && path.segments[0].ident == "Option")
                || (path.segments.len() == 3
                    && (path.segments[0].ident == "std" || path.segments[0].ident == "core")
                    && path.segments[1].ident == "option"
                    && path.segments[2].ident == "Option")
        }
        _ => false,
    }
}

/// Determine if the `field` has an attribute with given `namespace` and `name`
///
/// On the example of
/// `#[serde(skip_serializing_if = "Option::is_none")]`
///
/// * `serde` is the outermost path, here namespace
/// * it contains a `Meta::List`
/// * which contains in another Meta a `Meta::NameValue`
/// * with the name being `skip_serializing_if`
fn field_has_attribute(field: &Field, namespace: &str, name: &str) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident(namespace) {
            // Ignore non parsable attributes, as these are not important for us
            if let Meta::List(expr) = &attr.meta {
                let nested = match Punctuated::<Meta, Token![,]>::parse_terminated
                    .parse2(expr.tokens.clone())
                {
                    Ok(nested) => nested,
                    Err(_) => continue,
                };
                for expr in nested {
                    match expr {
                        Meta::NameValue(expr) => {
                            if let Some(ident) = expr.path.get_ident() {
                                if *ident == name {
                                    return true;
                                }
                            }
                        }
                        Meta::Path(expr) => {
                            if let Some(ident) = expr.get_ident() {
                                if *ident == name {
                                    return true;
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }
    false
}

/// Convenience macro to use the [`serde_as`] system.
///
/// The [`serde_as`] system is designed as a more flexible alternative to serde's `with` annotation.
/// The `#[serde_as]` attribute must be placed *before* the `#[derive]` attribute.
/// Each field of a struct or enum can be annotated with `#[serde_as(...)]` to specify which
/// transformations should be applied. `serde_as` is *not* supported on enum variants.
/// This is in contrast to `#[serde(with = "...")]`.
///
/// # Example
///
/// ```rust,ignore
/// use serde_with::{serde_as, DisplayFromStr, Map};
///
/// #[serde_as]
/// #[derive(Serialize, Deserialize)]
/// struct Data {
///     /// Serialize into number
///     #[serde_as(as = "_")]
///     a: u32,
///
///     /// Serialize into String
///     #[serde_as(as = "DisplayFromStr")]
///     b: u32,
///
///     /// Serialize into a map from String to String
///     #[serde_as(as = "Map<DisplayFromStr, _>")]
///     c: Vec<(u32, String)>,
/// }
/// ```
///
/// # Alternative path to `serde_with` crate
///
/// If `serde_with` is not available at the default path, its path should be specified with the
/// `crate` argument. See [re-exporting `serde_as`] for more use case information.
///
/// ```rust,ignore
/// #[serde_as(crate = "::some_other_lib::serde_with")]
/// #[derive(Deserialize)]
/// struct Data {
///     #[serde_as(as = "_")]
///     a: u32,
/// }
/// ```
///
/// # What this macro does
///
/// The `serde_as` macro only serves a convenience function.
/// All the steps it performs, can easily be done manually, in case the cost of an attribute macro
/// is deemed too high. The functionality can best be described with an example.
///
/// ```rust,ignore
/// #[serde_as]
/// #[derive(serde::Serialize)]
/// struct Foo {
///     #[serde_as(as = "Vec<_>")]
///     bar: Vec<u32>,
///
///     #[serde_as(as = "Option<DisplayFromStr>")]
///     baz: Option<u32>,
/// }
/// ```
///
/// 1. All the placeholder type `_` will be replaced with `::serde_with::Same`.
///    The placeholder type `_` marks all the places where the type's `Serialize` implementation
///    should be used. In the example, it means that the `u32` values will serialize with the
///    `Serialize` implementation of `u32`. The `Same` type implements `SerializeAs` whenever the
///    underlying type implements `Serialize` and is used to make the two traits compatible.
///
///    If you specify a custom path for `serde_with` via the `crate` attribute, the path to the
///    `Same` type will be altered accordingly.
///
/// 2. Wrap the type from the annotation inside a `::serde_with::As`.
///    In the above example we now have something like `::serde_with::As::<Vec<::serde_with::Same>>`.
///    The `As` type acts as the opposite of the `Same` type.
///    It allows using a `SerializeAs` type whenever a `Serialize` is required.
///
/// 3. Translate the `*as` attributes into the serde equivalent ones.
///    `#[serde_as(as = ...)]` will become `#[serde(with = ...)]`.
///    Similarly, `serialize_as` is translated to `serialize_with`.
///
///    The field attributes will be kept on the struct/enum such that other macros can use them
///    too.
///
/// 4. It searches `#[serde_as(as = ...)]` if there is a type named `BorrowCow` under any path.
///    If `BorrowCow` is found, the attribute `#[serde(borrow)]` is added to the field.
///    If `#[serde(borrow)]` or `#[serde(borrow = "...")]` is already present, this step will be
///    skipped.
///
/// 5. Restore the ability of accepting missing fields if both the field and the transformation are `Option`.
///
///    An `Option` is detected by an exact text match.
///    Renaming an import or type aliases can cause confusion here.
///    The following variants are supported.
///    * `Option`
///    * `std::option::Option`, with or without leading `::`
///    * `core::option::Option`, with or without leading `::`
///
///    If the field is of type `Option<T>` and the attribute `#[serde_as(as = "Option<S>")]` (also
///    `deserialize_as`; for any `T`/`S`) then `#[serde(default)]` is applied to the field.
///
///    This restores the ability of accepting missing fields, which otherwise often leads to confusing [serde_with#185](https://github.com/jonasbb/serde_with/issues/185).
///    `#[serde(default)]` is not applied, if it already exists.
///    It only triggers if both field and transformation are `Option`s.
///    For example, using `#[serde_as(as = "NoneAsEmptyString")]` on `Option<String>` will not see
///    any change.
///
///    If the automatically applied attribute is undesired, the behavior can be suppressed by adding
///    `#[serde_as(no_default)]`.
///
///     This can be combined like `#[serde_as(as = "Option<S>", no_default)]`.
///
/// After all these steps, the code snippet will have transformed into roughly this.
///
/// ```rust,ignore
/// #[derive(serde::Serialize)]
/// struct Foo {
///     #[serde_as(as = "Vec<_>")]
///     #[serde(with = "::serde_with::As::<Vec<::serde_with::Same>>")]
///     bar: Vec<u32>,
///
///     #[serde_as(as = "Option<DisplayFromStr>")]
///     #[serde(default)]
///     #[serde(with = "::serde_with::As::<Option<DisplayFromStr>>")]
///     baz: Option<u32>,
/// }
/// ```
///
/// # A note on `schemars` integration
/// When the `schemars_0_8`, `schemars_0_9`, or `schemars_1` features are enabled this macro
/// will scan for
/// `#[derive(JsonSchema)]` attributes and, if found, will add
/// `#[schemars(with = "Schema<T, ...>")]` annotations to any fields with a
/// `#[serde_as(as = ...)]` annotation. If you wish to override the default
/// behavior here you can add `#[serde_as(schemars = true)]` or
/// `#[serde_as(schemars = false)]`.
///
/// Note that this macro will check for any of the following derive paths:
/// * `JsonSchema`
/// * `schemars::JsonSchema`
/// * `::schemars::JsonSchema`
///
/// It will also work if the relevant derive is behind a `#[cfg_attr]` attribute
/// and propagate the `#[cfg_attr]` to the various `#[schemars]` field attributes.
///
/// [`serde_as`]: https://docs.rs/serde_with/3.14.0/serde_with/guide/index.html
/// [re-exporting `serde_as`]: https://docs.rs/serde_with/3.14.0/serde_with/guide/serde_as/index.html#re-exporting-serde_as
#[proc_macro_attribute]
pub fn serde_as(args: TokenStream, input: TokenStream) -> TokenStream {
    #[derive(FromMeta)]
    struct SerdeContainerOptions {
        #[darling(rename = "crate")]
        alt_crate_path: Option<Path>,
        #[darling(rename = "schemars")]
        enable_schemars_support: Option<bool>,
    }

    match NestedMeta::parse_meta_list(args.into()) {
        Ok(list) => {
            let container_options = match SerdeContainerOptions::from_list(&list) {
                Ok(v) => v,
                Err(e) => {
                    return TokenStream::from(e.write_errors());
                }
            };

            let serde_with_crate_path = container_options
                .alt_crate_path
                .unwrap_or_else(|| syn::parse_quote!(::serde_with));

            let schemars_config = match container_options.enable_schemars_support {
                _ if cfg!(not(any(
                    feature = "schemars_0_8",
                    feature = "schemars_0_9",
                    feature = "schemars_1"
                ))) =>
                {
                    SchemaFieldConfig::False
                }
                Some(condition) => condition.into(),
                None => utils::has_derive_jsonschema(input.clone()).unwrap_or_default(),
            };

            // Convert any error message into a nice compiler error
            let res = apply_function_to_struct_and_enum_fields_darling(
                input,
                &serde_with_crate_path,
                |field| serde_as_add_attr_to_field(field, &serde_with_crate_path, &schemars_config),
            )
            .unwrap_or_else(darling::Error::write_errors);
            TokenStream::from(res)
        }
        Err(e) => TokenStream::from(DarlingError::from(e).write_errors()),
    }
}

/// Inspect the field and convert the `serde_as` attribute into the classical `serde`
fn serde_as_add_attr_to_field(
    field: &mut Field,
    serde_with_crate_path: &Path,
    schemars_config: &SchemaFieldConfig,
) -> Result<(), DarlingError> {
    #[derive(FromField)]
    #[darling(attributes(serde_as))]
    struct SerdeAsOptions {
        /// The original type of the field
        ty: Type,

        r#as: Option<Type>,
        deserialize_as: Option<Type>,
        serialize_as: Option<Type>,
        no_default: Flag,
    }

    impl SerdeAsOptions {
        fn has_any_set(&self) -> bool {
            self.r#as.is_some() || self.deserialize_as.is_some() || self.serialize_as.is_some()
        }
    }

    #[derive(FromField)]
    #[darling(attributes(serde), allow_unknown_fields)]
    struct SerdeOptions {
        with: Option<String>,
        deserialize_with: Option<String>,
        serialize_with: Option<String>,

        borrow: Option<Override<String>>,
        default: Option<Override<String>>,
    }

    impl SerdeOptions {
        fn has_any_set(&self) -> bool {
            self.with.is_some() || self.deserialize_with.is_some() || self.serialize_with.is_some()
        }
    }

    /// Emit a `borrow` annotation, if the replacement type requires borrowing.
    fn emit_borrow_annotation(serde_options: &SerdeOptions, as_type: &Type, field: &mut Field) {
        let type_borrowcow = &syn::parse_quote!(BorrowCow);
        // If the field is not borrowed yet, check if we need to borrow it.
        if serde_options.borrow.is_none() && has_type_embedded(as_type, type_borrowcow) {
            let attr_borrow = parse_quote!(#[serde(borrow)]);
            field.attrs.push(attr_borrow);
        }
    }

    /// Emit a `default` annotation, if `as_type` and `field` are both `Option`.
    fn emit_default_annotation(
        serde_as_options: &SerdeAsOptions,
        serde_options: &SerdeOptions,
        as_type: &Type,
        field: &mut Field,
    ) {
        if !serde_as_options.no_default.is_present()
            && serde_options.default.is_none()
            && is_std_option(as_type)
            && is_std_option(&field.ty)
        {
            let attr_borrow = parse_quote!(#[serde(default)]);
            field.attrs.push(attr_borrow);
        }
    }

    // syn v2 no longer supports keywords in the path position of an attribute.
    // That breaks #[serde_as(as = "FooBar")], since `as` is a keyword.
    // For each attribute, that is named `serde_as`, we replace the `as` keyword with `r#as`.
    let mut has_serde_as = false;
    field.attrs.iter_mut().for_each(|attr| {
        if attr.path().is_ident("serde_as") {
            // We found a `serde_as` attribute.
            // Remember that such that we can quick exit otherwise
            has_serde_as = true;

            if let Meta::List(metalist) = &mut attr.meta {
                metalist.tokens = std::mem::take(&mut metalist.tokens)
                    .into_iter()
                    .map(|token| {
                        use proc_macro2::{Ident, TokenTree};

                        // Replace `as` with `r#as`.
                        match token {
                            TokenTree::Ident(ident) if ident == "as" => {
                                TokenTree::Ident(Ident::new_raw("as", ident.span()))
                            }
                            _ => token,
                        }
                    })
                    .collect();
            }
        }
    });
    // If there is no `serde_as` attribute, we can exit early.
    if !has_serde_as {
        return Ok(());
    }
    let serde_as_options = SerdeAsOptions::from_field(field)?;
    let serde_options = SerdeOptions::from_field(field)?;

    let mut errors = Vec::new();
    if !serde_as_options.has_any_set() {
        errors.push(DarlingError::custom("An empty `serde_as` attribute on a field has no effect. You are missing an `as`, `serialize_as`, or `deserialize_as` parameter."));
    }

    // Check if there are any conflicting attributes
    if serde_as_options.has_any_set() && serde_options.has_any_set() {
        errors.push(DarlingError::custom("Cannot combine `serde_as` with serde's `with`, `deserialize_with`, or `serialize_with`."));
    }

    if serde_as_options.r#as.is_some() && serde_as_options.deserialize_as.is_some() {
        errors.push(DarlingError::custom("Cannot combine `as` with `deserialize_as`. Use `serialize_as` to specify different serialization code."));
    } else if serde_as_options.r#as.is_some() && serde_as_options.serialize_as.is_some() {
        errors.push(DarlingError::custom("Cannot combine `as` with `serialize_as`. Use `deserialize_as` to specify different deserialization code."));
    }

    if !errors.is_empty() {
        return Err(DarlingError::multiple(errors));
    }

    let type_original = &serde_as_options.ty;
    let type_same = &syn::parse_quote!(#serde_with_crate_path::Same);
    if let Some(type_) = &serde_as_options.r#as {
        emit_borrow_annotation(&serde_options, type_, field);
        emit_default_annotation(&serde_as_options, &serde_options, type_, field);

        let replacement_type = replace_infer_type_with_type(type_.clone(), type_same);
        let attr_inner_tokens = quote!(#serde_with_crate_path::As::<#replacement_type>).to_string();
        let attr = parse_quote!(#[serde(with = #attr_inner_tokens)]);
        field.attrs.push(attr);

        match schemars_config {
            SchemaFieldConfig::False => {}
            lhs => {
                let rhs = utils::schemars_with_attr_if(
                    &field.attrs,
                    &["with", "serialize_with", "deserialize_with", "schema_with"],
                )?;

                match lhs & !rhs {
                    SchemaFieldConfig::False => {}
                    condition => {
                        let attr_inner_tokens = quote! {
                            #serde_with_crate_path::Schema::<#type_original, #replacement_type>
                        };
                        let attr_inner_tokens = attr_inner_tokens.to_string();
                        let attr = match condition {
                            SchemaFieldConfig::False => unreachable!(),
                            SchemaFieldConfig::True => {
                                parse_quote! { #[schemars(with = #attr_inner_tokens)] }
                            }
                            SchemaFieldConfig::Lazy(SchemaFieldCondition(condition)) => {
                                parse_quote! {
                                    #[cfg_attr(
                                        #condition,
                                        schemars(with = #attr_inner_tokens))
                                    ]
                                }
                            }
                        };

                        field.attrs.push(attr);
                    }
                }
            }
        }
    }
    if let Some(type_) = &serde_as_options.deserialize_as {
        emit_borrow_annotation(&serde_options, type_, field);
        emit_default_annotation(&serde_as_options, &serde_options, type_, field);

        let replacement_type = replace_infer_type_with_type(type_.clone(), type_same);
        let attr_inner_tokens =
            quote!(#serde_with_crate_path::As::<#replacement_type>::deserialize).to_string();
        let attr = parse_quote!(#[serde(deserialize_with = #attr_inner_tokens)]);
        field.attrs.push(attr);
    }
    if let Some(type_) = serde_as_options.serialize_as {
        let replacement_type = replace_infer_type_with_type(type_.clone(), type_same);
        let attr_inner_tokens =
            quote!(#serde_with_crate_path::As::<#replacement_type>::serialize).to_string();
        let attr = parse_quote!(#[serde(serialize_with = #attr_inner_tokens)]);
        field.attrs.push(attr);
    }

    Ok(())
}

/// Recursively replace all occurrences of `_` with `replacement` in a [Type][]
///
/// The [`serde_as`][macro@serde_as] macro allows to use the infer type, i.e., `_`, as shortcut for
/// `serde_with::As`. This function replaces all occurrences of the infer type with another type.
fn replace_infer_type_with_type(to_replace: Type, replacement: &Type) -> Type {
    match to_replace {
        // Base case
        // Replace the infer type with the actual replacement type
        Type::Infer(_) => replacement.clone(),

        // Recursive cases
        // Iterate through all positions where a type could occur and recursively call this function
        Type::Array(mut inner) => {
            *inner.elem = replace_infer_type_with_type(*inner.elem, replacement);
            Type::Array(inner)
        }
        Type::Group(mut inner) => {
            *inner.elem = replace_infer_type_with_type(*inner.elem, replacement);
            Type::Group(inner)
        }
        Type::Never(inner) => Type::Never(inner),
        Type::Paren(mut inner) => {
            *inner.elem = replace_infer_type_with_type(*inner.elem, replacement);
            Type::Paren(inner)
        }
        Type::Path(mut inner) => {
            if let Some(Pair::End(mut t)) | Some(Pair::Punctuated(mut t, _)) =
                inner.path.segments.pop()
            {
                t.arguments = match t.arguments {
                    PathArguments::None => PathArguments::None,
                    PathArguments::AngleBracketed(mut inner) => {
                        // Iterate over the args between the angle brackets
                        inner.args = inner
                            .args
                            .into_iter()
                            .map(|generic_argument| match generic_argument {
                                // replace types within the generics list, but leave other stuff
                                // like lifetimes untouched
                                GenericArgument::Type(type_) => GenericArgument::Type(
                                    replace_infer_type_with_type(type_, replacement),
                                ),
                                ga => ga,
                            })
                            .collect();
                        PathArguments::AngleBracketed(inner)
                    }
                    PathArguments::Parenthesized(mut inner) => {
                        inner.inputs = inner
                            .inputs
                            .into_iter()
                            .map(|type_| replace_infer_type_with_type(type_, replacement))
                            .collect();
                        inner.output = match inner.output {
                            ReturnType::Type(arrow, mut type_) => {
                                *type_ = replace_infer_type_with_type(*type_, replacement);
                                ReturnType::Type(arrow, type_)
                            }
                            default => default,
                        };
                        PathArguments::Parenthesized(inner)
                    }
                };
                inner.path.segments.push(t);
            }
            Type::Path(inner)
        }
        Type::Ptr(mut inner) => {
            *inner.elem = replace_infer_type_with_type(*inner.elem, replacement);
            Type::Ptr(inner)
        }
        Type::Reference(mut inner) => {
            *inner.elem = replace_infer_type_with_type(*inner.elem, replacement);
            Type::Reference(inner)
        }
        Type::Slice(mut inner) => {
            *inner.elem = replace_infer_type_with_type(*inner.elem, replacement);
            Type::Slice(inner)
        }
        Type::Tuple(mut inner) => {
            inner.elems = inner
                .elems
                .into_pairs()
                .map(|pair| match pair {
                    Pair::Punctuated(type_, p) => {
                        Pair::Punctuated(replace_infer_type_with_type(type_, replacement), p)
                    }
                    Pair::End(type_) => Pair::End(replace_infer_type_with_type(type_, replacement)),
                })
                .collect();
            Type::Tuple(inner)
        }

        // Pass unknown types or non-handleable types (e.g., bare Fn) without performing any
        // replacements
        type_ => type_,
    }
}

/// Check if a type ending in the `syn::Ident` `embedded_type` is contained in `type_`.
fn has_type_embedded(type_: &Type, embedded_type: &syn::Ident) -> bool {
    match type_ {
        // Base cases
        Type::Infer(_) => false,
        Type::Never(_inner) => false,

        // Recursive cases
        // Iterate through all positions where a type could occur and recursively call this function
        Type::Array(inner) => has_type_embedded(&inner.elem, embedded_type),
        Type::Group(inner) => has_type_embedded(&inner.elem, embedded_type),
        Type::Paren(inner) => has_type_embedded(&inner.elem, embedded_type),
        Type::Path(inner) => {
            match inner.path.segments.last() {
                Some(t) => {
                    if t.ident == *embedded_type {
                        return true;
                    }

                    match &t.arguments {
                        PathArguments::None => false,
                        PathArguments::AngleBracketed(inner) => {
                            // Iterate over the args between the angle brackets
                            inner
                                .args
                                .iter()
                                .any(|generic_argument| match generic_argument {
                                    // replace types within the generics list, but leave other stuff
                                    // like lifetimes untouched
                                    GenericArgument::Type(type_) => {
                                        has_type_embedded(type_, embedded_type)
                                    }
                                    _ga => false,
                                })
                        }
                        PathArguments::Parenthesized(inner) => {
                            inner
                                .inputs
                                .iter()
                                .any(|type_| has_type_embedded(type_, embedded_type))
                                || match &inner.output {
                                    ReturnType::Type(_arrow, type_) => {
                                        has_type_embedded(type_, embedded_type)
                                    }
                                    _default => false,
                                }
                        }
                    }
                }
                None => false,
            }
        }
        Type::Ptr(inner) => has_type_embedded(&inner.elem, embedded_type),
        Type::Reference(inner) => has_type_embedded(&inner.elem, embedded_type),
        Type::Slice(inner) => has_type_embedded(&inner.elem, embedded_type),
        Type::Tuple(inner) => inner.elems.pairs().any(|pair| match pair {
            Pair::Punctuated(type_, _) | Pair::End(type_) => {
                has_type_embedded(type_, embedded_type)
            }
        }),

        // Pass unknown types or non-handleable types (e.g., bare Fn) without performing any
        // replacements
        _type_ => false,
    }
}

/// Deserialize value by using its [`FromStr`] implementation
///
/// This is an alternative way to implement `Deserialize` for types, which also implement
/// [`FromStr`] by deserializing the type from string. Ensure that the struct/enum also implements
/// [`FromStr`]. If the implementation is missing, you will get an error message like
/// ```text
/// error[E0277]: the trait bound `Struct: std::str::FromStr` is not satisfied
/// ```
/// Additionally, `FromStr::Err` **must** implement [`Display`] as otherwise you will see a rather
/// unhelpful error message
///
/// Serialization with [`Display`] is available with the matching [`SerializeDisplay`] derive.
///
/// # Attributes
///
/// Attributes for the derive can be specified via the `#[serde_with(...)]` attribute on the struct
/// or enum. Currently, these arguments to the attribute are possible:
///
/// * **`#[serde_with(crate = "...")]`**: This allows using `DeserializeFromStr` when `serde_with`
///   is not available from the crate root. This happens while [renaming dependencies in
///   Cargo.toml][cargo-toml-rename] or when re-exporting the macro from a different crate.
///
///   This argument is analogue to [serde's crate argument][serde-crate] and the [crate argument
///   to `serde_as`][serde-as-crate].
///
/// # Example
///
/// ```rust,ignore
/// use std::str::FromStr;
///
/// #[derive(DeserializeFromStr)]
/// struct A {
///     a: u32,
///     b: bool,
/// }
///
/// impl FromStr for A {
///     type Err = String;
///
///     /// Parse a value like `123<>true`
///     fn from_str(s: &str) -> Result<Self, Self::Err> {
///         let mut parts = s.split("<>");
///         let number = parts
///             .next()
///             .ok_or_else(|| "Missing first value".to_string())?
///             .parse()
///             .map_err(|err: ParseIntError| err.to_string())?;
///         let bool = parts
///             .next()
///             .ok_or_else(|| "Missing second value".to_string())?
///             .parse()
///             .map_err(|err: ParseBoolError| err.to_string())?;
///         Ok(Self { a: number, b: bool })
///     }
/// }
///
/// let a: A = serde_json::from_str("\"159<>true\"").unwrap();
/// assert_eq!(A { a: 159, b: true }, a);
/// ```
///
/// [`Display`]: std::fmt::Display
/// [`FromStr`]: std::str::FromStr
/// [cargo-toml-rename]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml
/// [serde-as-crate]: https://docs.rs/serde_with/3.14.0/serde_with/guide/serde_as/index.html#re-exporting-serde_as
/// [serde-crate]: https://serde.rs/container-attrs.html#crate
#[proc_macro_derive(DeserializeFromStr, attributes(serde_with))]
pub fn derive_deserialize_fromstr(item: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(item);
    let derive_options = match DeriveOptions::from_derive_input(&input) {
        Ok(opt) => opt,
        Err(err) => {
            return err;
        }
    };
    TokenStream::from(deserialize_fromstr(
        input,
        derive_options.get_serde_with_path(),
    ))
}

fn deserialize_fromstr(mut input: DeriveInput, serde_with_crate_path: Path) -> TokenStream2 {
    let ident = input.ident;
    let where_clause = &mut input.generics.make_where_clause().predicates;
    where_clause.push(parse_quote!(Self: #serde_with_crate_path::__private__::FromStr));
    where_clause.push(parse_quote!(
        <Self as #serde_with_crate_path::__private__::FromStr>::Err: #serde_with_crate_path::__private__::Display
    ));
    let (de_impl_generics, ty_generics, where_clause) = split_with_de_lifetime(&input.generics);
    quote! {
        #[automatically_derived]
        impl #de_impl_generics #serde_with_crate_path::serde::Deserialize<'de> for #ident #ty_generics #where_clause {
            fn deserialize<__D>(deserializer: __D) -> #serde_with_crate_path::__private__::Result<Self, __D::Error>
            where
                __D: #serde_with_crate_path::serde::Deserializer<'de>,
            {
                struct Helper<__S>(#serde_with_crate_path::__private__::PhantomData<__S>);

                impl<'de, __S> #serde_with_crate_path::serde::de::Visitor<'de> for Helper<__S>
                where
                    __S: #serde_with_crate_path::__private__::FromStr,
                    <__S as #serde_with_crate_path::__private__::FromStr>::Err: #serde_with_crate_path::__private__::Display,
                {
                    type Value = __S;

                    fn expecting(&self, formatter: &mut #serde_with_crate_path::core::fmt::Formatter<'_>) -> #serde_with_crate_path::core::fmt::Result {
                        #serde_with_crate_path::__private__::Display::fmt("a string", formatter)
                    }

                    fn visit_str<__E>(
                        self,
                        value: &str
                    ) -> #serde_with_crate_path::__private__::Result<Self::Value, __E>
                    where
                        __E: #serde_with_crate_path::serde::de::Error,
                    {
                        value.parse::<Self::Value>().map_err(#serde_with_crate_path::serde::de::Error::custom)
                    }

                    fn visit_bytes<__E>(
                        self,
                        value: &[u8]
                    ) -> #serde_with_crate_path::__private__::Result<Self::Value, __E>
                    where
                        __E: #serde_with_crate_path::serde::de::Error,
                    {
                        let utf8 = #serde_with_crate_path::core::str::from_utf8(value).map_err(#serde_with_crate_path::serde::de::Error::custom)?;
                        self.visit_str(utf8)
                    }
                }

                deserializer.deserialize_str(Helper(#serde_with_crate_path::__private__::PhantomData))
            }
        }
    }
}

/// Serialize value by using it's [`Display`] implementation
///
/// This is an alternative way to implement `Serialize` for types, which also implement [`Display`]
/// by serializing the type as string. Ensure that the struct/enum also implements [`Display`].
/// If the implementation is missing, you will get an error message like
/// ```text
/// error[E0277]: `Struct` doesn't implement `std::fmt::Display`
/// ```
///
/// Deserialization with [`FromStr`] is available with the matching [`DeserializeFromStr`] derive.
///
/// # Attributes
///
/// Attributes for the derive can be specified via the `#[serde_with(...)]` attribute on the struct
/// or enum. Currently, these arguments to the attribute are possible:
///
/// * **`#[serde_with(crate = "...")]`**: This allows using `SerializeDisplay` when `serde_with` is
///   not available from the crate root. This happens while [renaming dependencies in
///   Cargo.toml][cargo-toml-rename] or when re-exporting the macro from a different crate.
///
///   This argument is analogue to [serde's crate argument][serde-crate] and the [crate argument
///   to `serde_as`][serde-as-crate].
///
/// # Example
///
/// ```rust,ignore
/// use std::fmt;
///
/// #[derive(SerializeDisplay)]
/// struct A {
///     a: u32,
///     b: bool,
/// }
///
/// impl fmt::Display for A {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "{}<>{}", self.a, self.b)
///     }
/// }
///
/// let a = A { a: 123, b: false };
/// assert_eq!(r#""123<>false""#, serde_json::to_string(&a).unwrap());
/// ```
///
/// [`Display`]: std::fmt::Display
/// [`FromStr`]: std::str::FromStr
/// [cargo-toml-rename]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml
/// [serde-as-crate]: https://docs.rs/serde_with/3.14.0/serde_with/guide/serde_as/index.html#re-exporting-serde_as
/// [serde-crate]: https://serde.rs/container-attrs.html#crate
#[proc_macro_derive(SerializeDisplay, attributes(serde_with))]
pub fn derive_serialize_display(item: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(item);
    let derive_options = match DeriveOptions::from_derive_input(&input) {
        Ok(opt) => opt,
        Err(err) => {
            return err;
        }
    };
    TokenStream::from(serialize_display(
        input,
        false,
        derive_options.get_serde_with_path(),
    ))
}

/// Serialize value by using its [`Display`] implementation with the “alternate” (`#`) format flag
///
/// This derive implements `serde::Serialize` for any type that already implements
/// [`std::fmt::Display`], emitting its string form using the alternate formatting specifier
/// (`{:#}`) instead of the normal `{}`.  In other words, rather than calling
/// `format!("{}", self)`, it calls `format!("{:#}", self)`.
///
/// Ensure that your type implements [`Display`], or you will get a compile‐error such as:
/// ```text
/// error[E0277]: `MyType` doesn't implement `std::fmt::Display`
/// ```
///
/// Deserialization from strings via [`std::str::FromStr`] is handled by the companion
/// [`DeserializeFromStr`] derive.
///
/// # Attributes
///
/// You may customize which `serde_with` crate is used (for renamed or re-exported crates)
/// via the same attribute namespace:
///
/// * `#[serde_with(crate = "...")]`
///   When your workspace renames or re-exports `serde_with`, use this to point at the correct path.
///   For example:
///   ```rust,ignore
///   #[derive(SerializeDisplayAlt)]
///   #[serde_with(crate = "my_forked_serde_with")]
///   pub struct Foo(/* … */);
///   ```
///
/// # Example
///
/// ```rust,ignore
/// use std::fmt;
/// use serde_with::{SerializeDisplayAlt, DeserializeFromStr};
///
/// #[derive(Debug, Clone, SerializeDisplayAlt, DeserializeFromStr)]
/// #[serde(transparent)]
/// pub struct MyType(u32);
///
/// impl fmt::Display for MyType {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         if f.alternate() {
///             // Alternate formatting: hex with 0x prefix
///             write!(f, "0x{:X}", self.0)
///         } else {
///             // Standard formatting: decimal
///             write!(f, "{}", self.0)
///         }
///     }
/// }
///
/// let v = MyType(15);
/// // SerializeDisplayAlt always uses `{:#}`, so this yields `"0xF"`
/// assert_eq!(r#""0xF""#, serde_json::to_string(&v).unwrap());
/// ```
///
/// [`Display`]: std::fmt::Display
/// [`FromStr`]: std::str::FromStr
/// [`DeserializeFromStr`]: crate::DeserializeFromStr
#[proc_macro_derive(SerializeDisplayAlt, attributes(serde_with))]
pub fn derive_serialize_display_alt(item: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(item);
    let derive_options = match DeriveOptions::from_derive_input(&input) {
        Ok(opt) => opt,
        Err(err) => {
            return err;
        }
    };
    TokenStream::from(serialize_display(
        input,
        true,
        derive_options.get_serde_with_path(),
    ))
}

fn serialize_display(
    mut input: DeriveInput,
    alternate: bool,
    serde_with_crate_path: Path,
) -> TokenStream2 {
    let ident = input.ident;
    input
        .generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: #serde_with_crate_path::__private__::Display));
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let collect_str_param = if alternate {
        quote! { &format_args!("{self:#}") }
    } else {
        quote! { &self }
    };

    quote! {
        #[automatically_derived]
        impl #impl_generics #serde_with_crate_path::serde::Serialize for #ident #ty_generics #where_clause {
            fn serialize<__S>(
                &self,
                serializer: __S
            ) -> #serde_with_crate_path::__private__::Result<__S::Ok, __S::Error>
            where
                __S: #serde_with_crate_path::serde::Serializer,
            {
                serializer.collect_str(#collect_str_param)
            }
        }
    }
}

#[doc(hidden)]
/// Private function. Not part of the public API
///
/// The only task of this derive macro is to consume any `serde_as` attributes and turn them into
/// inert attributes. This allows the serde_as macro to keep the field attributes without causing
/// compiler errors. The intend is that keeping the field attributes allows downstream crates to
/// consume and act on them without causing an ordering dependency to the serde_as macro.
///
/// Otherwise, downstream proc-macros would need to be placed *in front of* the main `#[serde_as]`
/// attribute, since otherwise the field attributes would already be stripped off.
///
/// More details about the use-cases in the GitHub discussion: <https://github.com/jonasbb/serde_with/discussions/260>.
#[proc_macro_derive(
    __private_consume_serde_as_attributes,
    attributes(serde_as, serde_with)
)]
pub fn __private_consume_serde_as_attributes(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// Apply attributes to all fields with matching types
///
/// Whenever you experience the need to apply the same attributes to multiple fields, you can use
/// this macro. It allows you to specify a list of types and a list of attributes.
/// Each field with a "matching" type will then get the attributes applied.
/// The `apply` attribute must be placed *before* any consuming attributes, such as `derive` or
/// `serde_as`, because Rust expands all attributes in order.
///
/// For example, if your struct or enum contains many `Option<T>` fields, but you do not want to
/// serialize `None` values, you can use this macro to apply the `#[serde(skip_serializing_if =
/// "Option::is_none")]` attribute to all fields of type `Option<T>`.
///
/// ```rust
/// # use serde_with_macros as serde_with;
/// #[serde_with::apply(
/// #   crate="serde_with",
///     Option => #[serde(skip_serializing_if = "Option::is_none")],
/// )]
/// #[derive(serde::Serialize)]
/// # #[derive(Default)]
/// struct Data {
///     a: Option<String>,
///     b: Option<u64>,
///     c: Option<String>,
///     d: Option<bool>,
/// }
/// #
/// # assert_eq!("{}", serde_json::to_string(&Data::default()).unwrap());
/// ```
///
/// Each rule starts with a type pattern, specifying which fields to match and a list of attributes
/// to apply. Multiple rules can be provided in a single `apply` attribute.
///
/// ```rust
/// # use serde_with_macros as serde_with;
/// #[serde_with::apply(
/// #   crate="serde_with",
///     Option => #[serde(default)] #[serde(skip_serializing_if = "Option::is_none")],
///     Option<bool> => #[serde(rename = "bool")],
/// )]
/// # #[derive(serde::Serialize)]
/// # #[derive(Default)]
/// # struct Data {
/// #     a: Option<String>,
/// #     b: Option<u64>,
/// #     c: Option<String>,
/// #     d: Option<bool>,
/// # }
/// #
/// # assert_eq!("{}", serde_json::to_string(&Data::default()).unwrap());
/// ```
///
/// ## Type Patterns
///
/// The type pattern left of the `=>` specifies which fields to match.
///
/// | Type Pattern            |                                       Matching Types | Notes                                                                           |
/// | :---------------------- | ---------------------------------------------------: | :------------------------------------------------------------------------------ |
/// | `_`                     | `Option<bool>`<br>`BTreeMap<&'static str, Vec<u32>>` | `_` matches all fields.                                                         |
/// | `Option`                |                   `Option<bool>`<br>`Option<String>` | A missing generic is compatible with any generic arguments.                     |
/// | `Option<bool>`          |                                       `Option<bool>` | A fully specified type only matches exactly.                                    |
/// | `BTreeMap<String, u32>` |                              `BTreeMap<String, u32>` | A fully specified type only matches exactly.                                    |
/// | `BTreeMap<String, _>`   |  `BTreeMap<String, u32>`<br>`BTreeMap<String, bool>` | Any `String` key `BTreeMap` matches, as the value is using the `_` placeholder. |
/// | `[u8; _]`               |                               `[u8; 1]`<br>`[u8; N]` | `_` also works as a placeholder for any array length.                           |
///
/// ## Opt-out for Individual Fields
///
/// The `apply` attribute will find all fields with a compatible type.
/// This can be overly eager and a different set of attributes might be required for a specific
/// field. You can opt-out of the `apply` attribute by adding the `#[serde_with(skip_apply)]`
/// attribute to the field. This will prevent any `apply` to apply to this field.
/// If two rules apply to the same field, it is impossible to opt-out of only a single one.
/// In this case the attributes must be applied to the field manually.
///
/// ```rust
/// # use serde_json::json;
/// # use serde_with_macros as serde_with;
/// #[serde_with::apply(
/// #   crate="serde_with",
///     Option => #[serde(skip_serializing_if = "Option::is_none")],
/// )]
/// #[derive(serde::Serialize)]
/// struct Data {
///     a: Option<String>,
///     #[serde_with(skip_apply)]
///     always_serialize_this_field: Option<u64>,
///     c: Option<String>,
///     d: Option<bool>,
/// }
///
/// let data = Data {
///     a: None,
///     always_serialize_this_field: None,
///     c: None,
///     d: None,
/// };
///
/// // serializes into this JSON:
/// # assert_eq!(json!(
/// {
///     "always_serialize_this_field": null
/// }
/// # ), serde_json::to_value(data).unwrap());
/// ```
///
/// # Alternative path to `serde_with` crate
///
/// If `serde_with` is not available at the default path, its path should be specified with the
/// `crate` argument. See [re-exporting `serde_as`] for more use case information.
///
/// ```rust,ignore
/// #[serde_with::apply(
///     crate = "::some_other_lib::serde_with"
///     Option => #[serde(skip_serializing_if = "Option::is_none")],
/// )]
/// #[derive(serde::Serialize)]
/// struct Data {
///     a: Option<String>,
///     b: Option<u64>,
///     c: Option<String>,
///     d: Option<bool>,
/// }
/// ```
#[proc_macro_attribute]
pub fn apply(args: TokenStream, input: TokenStream) -> TokenStream {
    apply::apply(args, input)
}

#![allow(
    clippy::blocks_in_conditions,
    clippy::needless_pass_by_value,
    clippy::if_not_else
)]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
use quote::{quote, quote_spanned};
use syn::parse::{Nothing, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::{
    parenthesized, parse_macro_input, token, Abi, Attribute, Data, DeriveInput, Error, Expr, Field,
    Generics, Path, Result, Token, Type, Visibility,
};

/// Derive the `RefCast` trait.
///
/// See the [crate-level documentation](./index.html) for usage examples!
///
/// # Attributes
///
/// Use the `#[trivial]` attribute to mark any zero-sized fields that are *not*
/// the one that references are going to be converted from.
///
/// ```
/// use ref_cast::RefCast;
/// use std::marker::PhantomData;
///
/// #[derive(RefCast)]
/// #[repr(transparent)]
/// pub struct Generic<T, U> {
///     raw: Vec<U>,
///     #[trivial]
///     aux: Variance<T, U>,
/// }
///
/// type Variance<T, U> = PhantomData<fn(T) -> U>;
/// ```
///
/// Fields with a type named `PhantomData` or `PhantomPinned` are automatically
/// recognized and do not need to be marked with this attribute.
///
/// ```
/// use ref_cast::RefCast;
/// use std::marker::{PhantomData, PhantomPinned};
///
/// #[derive(RefCast)]  // generates a conversion from &[u8] to &Bytes<'_>
/// #[repr(transparent)]
/// pub struct Bytes<'arena> {
///     lifetime: PhantomData<&'arena ()>,
///     pin: PhantomPinned,
///     bytes: [u8],
/// }
/// ```
#[proc_macro_derive(RefCast, attributes(trivial))]
pub fn derive_ref_cast(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_ref_cast(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derive that makes the `ref_cast_custom` attribute able to generate
/// freestanding reference casting functions for a type.
///
/// Please refer to the documentation of
/// [`#[ref_cast_custom]`][macro@ref_cast_custom] where these two macros are
/// documented together.
#[proc_macro_derive(RefCastCustom, attributes(trivial))]
pub fn derive_ref_cast_custom(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_ref_cast_custom(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Create a function for a RefCast-style reference cast. Call site gets control
/// of the visibility, function name, argument name, `const`ness, unsafety, and
/// documentation.
///
/// The `derive(RefCast)` macro produces a trait impl, which means the function
/// names are predefined, and public if your type is public, and not callable in
/// `const` (at least today on stable Rust). As an alternative to that,
/// `derive(RefCastCustom)` exposes greater flexibility so that instead of a
/// trait impl, the casting functions can be made associated functions or free
/// functions, can be named what you want, documented, `const` or `unsafe` if
/// you want, and have your exact choice of visibility.
///
/// ```rust
/// use ref_cast::{ref_cast_custom, RefCastCustom};
///
/// #[derive(RefCastCustom)]  // does not generate any public API by itself
/// #[repr(transparent)]
/// pub struct Frame([u8]);
///
/// impl Frame {
///     #[ref_cast_custom]  // requires derive(RefCastCustom) on the return type
///     pub(crate) const fn new(bytes: &[u8]) -> &Self;
///
///     #[ref_cast_custom]
///     pub(crate) fn new_mut(bytes: &mut [u8]) -> &mut Self;
/// }
///
/// // example use of the const fn
/// const FRAME: &Frame = Frame::new(b"...");
/// ```
///
/// The above shows associated functions, but you might alternatively want to
/// generate free functions:
///
/// ```rust
/// # use ref_cast::{ref_cast_custom, RefCastCustom};
/// #
/// # #[derive(RefCastCustom)]
/// # #[repr(transparent)]
/// # pub struct Frame([u8]);
/// #
/// impl Frame {
///     pub fn new<T: AsRef<[u8]>>(bytes: &T) -> &Self {
///         #[ref_cast_custom]
///         fn ref_cast(bytes: &[u8]) -> &Frame;
///
///         ref_cast(bytes.as_ref())
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn ref_cast_custom(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = TokenStream2::from(input);
    let expanded = match (|input: ParseStream| {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let constness: Option<Token![const]> = input.parse()?;
        let asyncness: Option<Token![async]> = input.parse()?;
        let unsafety: Option<Token![unsafe]> = input.parse()?;
        let abi: Option<Abi> = input.parse()?;
        let fn_token: Token![fn] = input.parse()?;
        let ident: Ident = input.parse()?;
        let mut generics: Generics = input.parse()?;

        let content;
        let paren_token = parenthesized!(content in input);
        let arg: Ident = content.parse()?;
        let colon_token: Token![:] = content.parse()?;
        let from_type: Type = content.parse()?;
        let _trailing_comma: Option<Token![,]> = content.parse()?;
        if !content.is_empty() {
            let rest: TokenStream2 = content.parse()?;
            return Err(Error::new_spanned(
                rest,
                "ref_cast_custom function is required to have a single argument",
            ));
        }

        let arrow_token: Token![->] = input.parse()?;
        let to_type: Type = input.parse()?;
        generics.where_clause = input.parse()?;
        let semi_token: Token![;] = input.parse()?;

        let _: Nothing = syn::parse::<Nothing>(args)?;

        Ok(Function {
            attrs,
            vis,
            constness,
            asyncness,
            unsafety,
            abi,
            fn_token,
            ident,
            generics,
            paren_token,
            arg,
            colon_token,
            from_type,
            arrow_token,
            to_type,
            semi_token,
        })
    })
    .parse2(input.clone())
    {
        Ok(function) => expand_function_body(function),
        Err(parse_error) => {
            let compile_error = parse_error.to_compile_error();
            quote!(#compile_error #input)
        }
    };
    TokenStream::from(expanded)
}

struct Function {
    attrs: Vec<Attribute>,
    vis: Visibility,
    constness: Option<Token![const]>,
    asyncness: Option<Token![async]>,
    unsafety: Option<Token![unsafe]>,
    abi: Option<Abi>,
    fn_token: Token![fn],
    ident: Ident,
    generics: Generics,
    paren_token: token::Paren,
    arg: Ident,
    colon_token: Token![:],
    from_type: Type,
    arrow_token: Token![->],
    to_type: Type,
    semi_token: Token![;],
}

fn expand_ref_cast(input: &DeriveInput) -> Result<TokenStream2> {
    check_repr(input)?;

    let name = &input.ident;
    let name_str = name.to_string();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = fields(input)?;
    let from = only_field_ty(fields)?;
    let trivial = trivial_fields(fields)?;

    let assert_trivial_fields = if !trivial.is_empty() {
        Some(quote! {
            if false {
                #(
                    ::ref_cast::__private::assert_trivial::<#trivial>();
                )*
            }
        })
    } else {
        None
    };

    Ok(quote! {
        impl #impl_generics ::ref_cast::RefCast for #name #ty_generics #where_clause {
            type From = #from;

            #[inline]
            fn ref_cast(_from: &Self::From) -> &Self {
                #assert_trivial_fields
                #[cfg(debug_assertions)]
                {
                    #[allow(unused_imports)]
                    use ::ref_cast::__private::LayoutUnsized;
                    ::ref_cast::__private::assert_layout::<Self, Self::From>(
                        #name_str,
                        ::ref_cast::__private::Layout::<Self>::SIZE,
                        ::ref_cast::__private::Layout::<Self::From>::SIZE,
                        ::ref_cast::__private::Layout::<Self>::ALIGN,
                        ::ref_cast::__private::Layout::<Self::From>::ALIGN,
                    );
                }
                unsafe {
                    &*(_from as *const Self::From as *const Self)
                }
            }

            #[inline]
            fn ref_cast_mut(_from: &mut Self::From) -> &mut Self {
                #[cfg(debug_assertions)]
                {
                    #[allow(unused_imports)]
                    use ::ref_cast::__private::LayoutUnsized;
                    ::ref_cast::__private::assert_layout::<Self, Self::From>(
                        #name_str,
                        ::ref_cast::__private::Layout::<Self>::SIZE,
                        ::ref_cast::__private::Layout::<Self::From>::SIZE,
                        ::ref_cast::__private::Layout::<Self>::ALIGN,
                        ::ref_cast::__private::Layout::<Self::From>::ALIGN,
                    );
                }
                unsafe {
                    &mut *(_from as *mut Self::From as *mut Self)
                }
            }
        }
    })
}

fn expand_ref_cast_custom(input: &DeriveInput) -> Result<TokenStream2> {
    check_repr(input)?;

    let vis = &input.vis;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = fields(input)?;
    let from = only_field_ty(fields)?;
    let trivial = trivial_fields(fields)?;

    let assert_trivial_fields = if !trivial.is_empty() {
        Some(quote! {
            fn __static_assert() {
                if false {
                    #(
                        ::ref_cast::__private::assert_trivial::<#trivial>();
                    )*
                }
            }
        })
    } else {
        None
    };

    Ok(quote! {
        const _: () = {
            #[non_exhaustive]
            #vis struct RefCastCurrentCrate {}

            unsafe impl #impl_generics ::ref_cast::__private::RefCastCustom<#from> for #name #ty_generics #where_clause {
                type CurrentCrate = RefCastCurrentCrate;
                #assert_trivial_fields
            }
        };
    })
}

fn expand_function_body(function: Function) -> TokenStream2 {
    let Function {
        attrs,
        vis,
        constness,
        asyncness,
        unsafety,
        abi,
        fn_token,
        ident,
        generics,
        paren_token,
        arg,
        colon_token,
        from_type,
        arrow_token,
        to_type,
        semi_token,
    } = function;

    let args = quote_spanned! {paren_token.span=>
        (#arg #colon_token #from_type)
    };

    let allow_unused_unsafe = if unsafety.is_some() {
        Some(quote!(#[allow(unused_unsafe)]))
    } else {
        None
    };

    let mut inline_attr = Some(quote!(#[inline]));
    for attr in &attrs {
        if attr.path().is_ident("inline") {
            inline_attr = None;
            break;
        }
    }

    // Apply a macro-generated span to the "unsafe" token for the unsafe block.
    // This is instead of reusing the caller's function signature's #unsafety
    // across both the generated function signature and generated unsafe block,
    // and instead of using `semi_token.span` like for the rest of the generated
    // code below, both of which would cause `forbid(unsafe_code)` located in
    // the caller to reject the expanded code.
    let macro_generated_unsafe = quote!(unsafe);

    quote_spanned! {semi_token.span=>
        #(#attrs)*
        #inline_attr
        #vis #constness #asyncness #unsafety #abi
        #fn_token #ident #generics #args #arrow_token #to_type {
            // check lifetime
            let _ = || {
                ::ref_cast::__private::ref_cast_custom::<#from_type, #to_type>(#arg);
            };

            // check same crate
            let _ = ::ref_cast::__private::CurrentCrate::<#from_type, #to_type> {};

            #allow_unused_unsafe // in case they are building with deny(unsafe_op_in_unsafe_fn)
            #[allow(clippy::transmute_ptr_to_ptr)]
            #macro_generated_unsafe {
                ::ref_cast::__private::transmute::<#from_type, #to_type>(#arg)
            }
        }
    }
}

fn check_repr(input: &DeriveInput) -> Result<()> {
    let mut has_repr = false;
    let mut errors = None;
    let mut push_error = |error| match &mut errors {
        Some(errors) => Error::combine(errors, error),
        None => errors = Some(error),
    };

    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            if let Err(error) = attr.parse_args_with(|input: ParseStream| {
                while !input.is_empty() {
                    let path = input.call(Path::parse_mod_style)?;
                    if path.is_ident("transparent") || path.is_ident("C") {
                        has_repr = true;
                    } else if path.is_ident("packed") {
                        // ignore
                    } else {
                        let meta_item_span = if input.peek(token::Paren) {
                            let group: TokenTree = input.parse()?;
                            quote!(#path #group)
                        } else if input.peek(Token![=]) {
                            let eq_token: Token![=] = input.parse()?;
                            let value: Expr = input.parse()?;
                            quote!(#path #eq_token #value)
                        } else {
                            quote!(#path)
                        };
                        let msg = if path.is_ident("align") {
                            "aligned repr on struct that implements RefCast is not supported"
                        } else {
                            "unrecognized repr on struct that implements RefCast"
                        };
                        push_error(Error::new_spanned(meta_item_span, msg));
                    }
                    if !input.is_empty() {
                        input.parse::<Token![,]>()?;
                    }
                }
                Ok(())
            }) {
                push_error(error);
            }
        }
    }

    if !has_repr {
        let mut requires_repr = Error::new(
            Span::call_site(),
            "RefCast trait requires #[repr(transparent)]",
        );
        if let Some(errors) = errors {
            requires_repr.combine(errors);
        }
        errors = Some(requires_repr);
    }

    match errors {
        None => Ok(()),
        Some(errors) => Err(errors),
    }
}

type Fields = Punctuated<Field, Token![,]>;

fn fields(input: &DeriveInput) -> Result<&Fields> {
    use syn::Fields;

    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            Fields::Unnamed(fields) => Ok(&fields.unnamed),
            Fields::Unit => Err(Error::new(
                Span::call_site(),
                "RefCast does not support unit structs",
            )),
        },
        Data::Enum(_) => Err(Error::new(
            Span::call_site(),
            "RefCast does not support enums",
        )),
        Data::Union(_) => Err(Error::new(
            Span::call_site(),
            "RefCast does not support unions",
        )),
    }
}

fn only_field_ty(fields: &Fields) -> Result<&Type> {
    let is_trivial = decide_trivial(fields)?;
    let mut only_field = None;

    for field in fields {
        if !is_trivial(field)? {
            if only_field.take().is_some() {
                break;
            }
            only_field = Some(&field.ty);
        }
    }

    only_field.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "RefCast requires a struct with a single field",
        )
    })
}

fn trivial_fields(fields: &Fields) -> Result<Vec<&Type>> {
    let is_trivial = decide_trivial(fields)?;
    let mut trivial = Vec::new();

    for field in fields {
        if is_trivial(field)? {
            trivial.push(&field.ty);
        }
    }

    Ok(trivial)
}

fn decide_trivial(fields: &Fields) -> Result<fn(&Field) -> Result<bool>> {
    for field in fields {
        if is_explicit_trivial(field)? {
            return Ok(is_explicit_trivial);
        }
    }
    Ok(is_implicit_trivial)
}

#[allow(clippy::unnecessary_wraps)] // match signature of is_explicit_trivial
fn is_implicit_trivial(field: &Field) -> Result<bool> {
    match &field.ty {
        Type::Tuple(ty) => Ok(ty.elems.is_empty()),
        Type::Path(ty) => {
            let ident = &ty.path.segments.last().unwrap().ident;
            Ok(ident == "PhantomData" || ident == "PhantomPinned")
        }
        _ => Ok(false),
    }
}

fn is_explicit_trivial(field: &Field) -> Result<bool> {
    for attr in &field.attrs {
        if attr.path().is_ident("trivial") {
            attr.meta.require_path_only()?;
            return Ok(true);
        }
    }
    Ok(false)
}

// SPDX-License-Identifier: Apache-2.0 OR MIT

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, format_ident, quote};
use syn::{
    Error, FnArg, GenericArgument, Ident, ImplItem, ItemImpl, Pat, PatIdent, PatType, Path,
    PathArguments, Result, ReturnType, Signature, Token, Type, TypePath, TypeReference,
    parse_quote, spanned::Spanned as _, token::Colon, visit_mut::VisitMut as _,
};

use crate::utils::{ReplaceReceiver, SliceExt as _};

pub(crate) fn attribute(args: &TokenStream, mut input: ItemImpl) -> TokenStream {
    let res = (|| -> Result<()> {
        if !args.is_empty() {
            bail!(args, "unexpected argument: `{}`", args)
        }
        validate_impl(&input)?;
        expand_impl(&mut input);
        Ok(())
    })();

    if let Err(e) = res {
        let mut tokens = e.to_compile_error();
        if let Type::Path(self_ty) = &*input.self_ty {
            let (impl_generics, _, where_clause) = input.generics.split_for_impl();

            // Generate a dummy impl of `PinnedDrop`.
            // In many cases, `#[pinned_drop] impl` is declared after `#[pin_project]`.
            // Therefore, if `pinned_drop` compile fails, you will also get an error
            // about `PinnedDrop` not being implemented.
            // This can be prevented to some extent by generating a dummy
            // `PinnedDrop` implementation.
            // We already know that we will get a compile error, so this won't
            // accidentally compile successfully.
            //
            // However, if `input.self_ty` is not Type::Path, there is a high possibility that
            // the type does not exist (since #[pin_project] can only be used on struct/enum
            // definitions), so do not generate a dummy impl.
            tokens.extend(quote! {
                impl #impl_generics ::pin_project::__private::PinnedDrop for #self_ty
                #where_clause
                {
                    unsafe fn drop(self: ::pin_project::__private::Pin<&mut Self>) {}
                }
            });
        }
        tokens
    } else {
        input.into_token_stream()
    }
}

/// Validates the signature of given `PinnedDrop` impl.
fn validate_impl(item: &ItemImpl) -> Result<()> {
    const INVALID_ITEM: &str =
        "#[pinned_drop] may only be used on implementation for the `PinnedDrop` trait";

    if let Some(attr) = item.attrs.find("pinned_drop") {
        bail!(attr, "duplicate #[pinned_drop] attribute");
    }

    if let Some((_, path, _)) = &item.trait_ {
        if !path.is_ident("PinnedDrop") {
            bail!(path, INVALID_ITEM);
        }
    } else {
        bail!(item.self_ty, INVALID_ITEM);
    }

    if item.unsafety.is_some() {
        bail!(item.unsafety, "implementing the trait `PinnedDrop` is not unsafe");
    }
    if item.items.is_empty() {
        bail!(item, "not all trait items implemented, missing: `drop`");
    }

    match &*item.self_ty {
        Type::Path(_) => {}
        ty => {
            bail!(ty, "implementing the trait `PinnedDrop` on this type is unsupported");
        }
    }

    item.items.iter().enumerate().try_for_each(|(i, item)| match item {
        ImplItem::Const(item) => {
            bail!(item, "const `{}` is not a member of trait `PinnedDrop`", item.ident)
        }
        ImplItem::Type(item) => {
            bail!(item, "type `{}` is not a member of trait `PinnedDrop`", item.ident)
        }
        ImplItem::Fn(method) => {
            validate_sig(&method.sig)?;
            if i == 0 { Ok(()) } else { bail!(method, "duplicate definitions with name `drop`") }
        }
        _ => unreachable!("unexpected ImplItem"),
    })
}

/// Validates the signature of given `PinnedDrop::drop` method.
///
/// The correct signature is: `(mut) self: (<path>::)Pin<&mut Self>`
fn validate_sig(sig: &Signature) -> Result<()> {
    fn get_ty_path(ty: &Type) -> Option<&Path> {
        if let Type::Path(TypePath { qself: None, path }) = ty { Some(path) } else { None }
    }

    const INVALID_ARGUMENT: &str = "method `drop` must take an argument `self: Pin<&mut Self>`";

    if sig.ident != "drop" {
        bail!(sig.ident, "method `{}` is not a member of trait `PinnedDrop`", sig.ident);
    }

    if let ReturnType::Type(_, ty) = &sig.output {
        match &**ty {
            Type::Tuple(ty) if ty.elems.is_empty() => {}
            _ => bail!(ty, "method `drop` must return the unit type"),
        }
    }

    match sig.inputs.len() {
        1 => {}
        0 => return Err(Error::new(sig.paren_token.span.join(), INVALID_ARGUMENT)),
        _ => bail!(sig.inputs, INVALID_ARGUMENT),
    }

    if let Some(arg) = sig.receiver() {
        // (mut) self: <path>
        if let Some(path) = get_ty_path(&arg.ty) {
            let ty =
                path.segments.last().expect("type paths should always have at least one segment");
            if let PathArguments::AngleBracketed(args) = &ty.arguments {
                // (mut) self: (<path>::)<ty><&mut <elem>..>
                if let Some(GenericArgument::Type(Type::Reference(TypeReference {
                    mutability: Some(_),
                    elem,
                    ..
                }))) = args.args.first()
                {
                    // (mut) self: (<path>::)Pin<&mut Self>
                    if args.args.len() == 1
                        && ty.ident == "Pin"
                        && get_ty_path(elem).map_or(false, |path| path.is_ident("Self"))
                    {
                        if sig.unsafety.is_some() {
                            bail!(sig.unsafety, "implementing the method `drop` is not unsafe");
                        }
                        return Ok(());
                    }
                }
            }
        }
    }

    bail!(sig.inputs[0], INVALID_ARGUMENT)
}

// from:
//
// fn drop(self: Pin<&mut Self>) {
//     // ...
// }
//
// into:
//
// unsafe fn drop(self: Pin<&mut Self>) {
//     fn __drop_inner<T>(__self: Pin<&mut Foo<'_, T>>) {
//         fn __drop_inner() {}
//         // ...
//     }
//     __drop_inner(self);
// }
//
fn expand_impl(item: &mut ItemImpl) {
    // `PinnedDrop` is a private trait and should not appear in docs.
    item.attrs.push(parse_quote!(#[doc(hidden)]));

    let path = &mut item.trait_.as_mut().expect("unexpected inherent impl").1;
    *path = parse_quote_spanned! { Span::call_site().located_at(path.span()) =>
        ::pin_project::__private::PinnedDrop
    };

    let method =
        if let ImplItem::Fn(method) = &mut item.items[0] { method } else { unreachable!() };

    // `fn drop(mut self: Pin<&mut Self>)` -> `fn __drop_inner<T>(mut __self: Pin<&mut Receiver>)`
    let drop_inner = {
        let mut drop_inner = method.clone();
        let ident = format_ident!("__drop_inner");
        // Add a dummy `__drop_inner` function to prevent users call outer `__drop_inner`.
        drop_inner.block.stmts.insert(0, parse_quote!(fn #ident() {}));
        drop_inner.sig.ident = ident;
        drop_inner.sig.generics = item.generics.clone();
        let receiver = drop_inner.sig.receiver().expect("drop() should have a receiver").clone();
        let pat = Box::new(Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: receiver.mutability,
            ident: Ident::new("__self", receiver.self_token.span()),
            subpat: None,
        }));
        drop_inner.sig.inputs[0] = FnArg::Typed(PatType {
            attrs: receiver.attrs,
            pat,
            colon_token: Colon::default(),
            ty: receiver.ty,
        });
        let self_ty = if let Type::Path(ty) = &*item.self_ty { ty } else { unreachable!() };
        let mut visitor = ReplaceReceiver(self_ty);
        visitor.visit_signature_mut(&mut drop_inner.sig);
        visitor.visit_block_mut(&mut drop_inner.block);
        drop_inner
    };

    // `fn drop(mut self: Pin<&mut Self>)` -> `unsafe fn drop(self: Pin<&mut Self>)`
    method.sig.unsafety = Some(<Token![unsafe]>::default());
    let self_token = if let FnArg::Receiver(ref mut rec) = method.sig.inputs[0] {
        rec.mutability = None;
        &rec.self_token
    } else {
        panic!("drop() should have a receiver")
    };

    method.block.stmts = parse_quote! {
        #[allow(
            clippy::missing_const_for_fn,
            clippy::needless_pass_by_value, // This lint does not warn the receiver.
            clippy::single_call_fn
        )]
        #drop_inner
        __drop_inner(#self_token);
    };
}

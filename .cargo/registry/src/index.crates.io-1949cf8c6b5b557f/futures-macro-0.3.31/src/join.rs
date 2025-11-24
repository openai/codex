//! The futures-rs `join!` macro implementation.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, Token};

#[derive(Default)]
struct Join {
    fut_exprs: Vec<Expr>,
}

impl Parse for Join {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut join = Self::default();

        while !input.is_empty() {
            join.fut_exprs.push(input.parse::<Expr>()?);

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(join)
    }
}

fn bind_futures(fut_exprs: Vec<Expr>, span: Span) -> (Vec<TokenStream2>, Vec<Ident>) {
    let mut future_let_bindings = Vec::with_capacity(fut_exprs.len());
    let future_names: Vec<_> = fut_exprs
        .into_iter()
        .enumerate()
        .map(|(i, expr)| {
            let name = format_ident!("_fut{}", i, span = span);
            future_let_bindings.push(quote! {
                // Move future into a local so that it is pinned in one place and
                // is no longer accessible by the end user.
                let mut #name = __futures_crate::future::maybe_done(#expr);
                let mut #name = unsafe { __futures_crate::Pin::new_unchecked(&mut #name) };
            });
            name
        })
        .collect();

    (future_let_bindings, future_names)
}

/// The `join!` macro.
pub(crate) fn join(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Join);

    // should be def_site, but that's unstable
    let span = Span::call_site();

    let (future_let_bindings, future_names) = bind_futures(parsed.fut_exprs, span);

    let poll_futures = future_names.iter().map(|fut| {
        quote! {
            __all_done &= __futures_crate::future::Future::poll(
                #fut.as_mut(), __cx).is_ready();
        }
    });
    let take_outputs = future_names.iter().map(|fut| {
        quote! {
            #fut.as_mut().take_output().unwrap(),
        }
    });

    TokenStream::from(quote! { {
        #( #future_let_bindings )*

        __futures_crate::future::poll_fn(move |__cx: &mut __futures_crate::task::Context<'_>| {
            let mut __all_done = true;
            #( #poll_futures )*
            if __all_done {
                __futures_crate::task::Poll::Ready((
                    #( #take_outputs )*
                ))
            } else {
                __futures_crate::task::Poll::Pending
            }
        }).await
    } })
}

/// The `try_join!` macro.
pub(crate) fn try_join(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Join);

    // should be def_site, but that's unstable
    let span = Span::call_site();

    let (future_let_bindings, future_names) = bind_futures(parsed.fut_exprs, span);

    let poll_futures = future_names.iter().map(|fut| {
        quote! {
            if __futures_crate::future::Future::poll(
                #fut.as_mut(), __cx).is_pending()
            {
                __all_done = false;
            } else if #fut.as_mut().output_mut().unwrap().is_err() {
                // `.err().unwrap()` rather than `.unwrap_err()` so that we don't introduce
                // a `T: Debug` bound.
                // Also, for an error type of ! any code after `err().unwrap()` is unreachable.
                #[allow(unreachable_code)]
                return __futures_crate::task::Poll::Ready(
                    __futures_crate::Err(
                        #fut.as_mut().take_output().unwrap().err().unwrap()
                    )
                );
            }
        }
    });
    let take_outputs = future_names.iter().map(|fut| {
        quote! {
            // `.ok().unwrap()` rather than `.unwrap()` so that we don't introduce
            // an `E: Debug` bound.
            // Also, for an ok type of ! any code after `ok().unwrap()` is unreachable.
            #[allow(unreachable_code)]
            #fut.as_mut().take_output().unwrap().ok().unwrap(),
        }
    });

    TokenStream::from(quote! { {
        #( #future_let_bindings )*

        #[allow(clippy::diverging_sub_expression)]
        __futures_crate::future::poll_fn(move |__cx: &mut __futures_crate::task::Context<'_>| {
            let mut __all_done = true;
            #( #poll_futures )*
            if __all_done {
                __futures_crate::task::Poll::Ready(
                    __futures_crate::Ok((
                        #( #take_outputs )*
                    ))
                )
            } else {
                __futures_crate::task::Poll::Pending
            }
        }).await
    } })
}

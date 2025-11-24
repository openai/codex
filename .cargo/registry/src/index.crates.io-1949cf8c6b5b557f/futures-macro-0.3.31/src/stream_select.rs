use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse::Parser, punctuated::Punctuated, Expr, Index, Token};

/// The `stream_select!` macro.
pub(crate) fn stream_select(input: TokenStream) -> Result<TokenStream, syn::Error> {
    let args = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(input)?;
    if args.len() < 2 {
        return Ok(quote! {
           compile_error!("stream select macro needs at least two arguments.")
        });
    }
    let generic_idents = (0..args.len()).map(|i| format_ident!("_{}", i)).collect::<Vec<_>>();
    let field_idents = (0..args.len()).map(|i| format_ident!("__{}", i)).collect::<Vec<_>>();
    let field_idents_2 = (0..args.len()).map(|i| format_ident!("___{}", i)).collect::<Vec<_>>();
    let field_indices = (0..args.len()).map(Index::from).collect::<Vec<_>>();
    let args = args.iter().map(|e| e.to_token_stream());

    Ok(quote! {
        {
            #[derive(Debug)]
            struct StreamSelect<#(#generic_idents),*> (#(Option<#generic_idents>),*);

            enum StreamEnum<#(#generic_idents),*> {
                #(
                    #generic_idents(#generic_idents)
                ),*,
                None,
            }

            impl<ITEM, #(#generic_idents),*> __futures_crate::stream::Stream for StreamEnum<#(#generic_idents),*>
            where #(#generic_idents: __futures_crate::stream::Stream<Item=ITEM> + ::std::marker::Unpin,)*
            {
                type Item = ITEM;

                fn poll_next(mut self: ::std::pin::Pin<&mut Self>, cx: &mut __futures_crate::task::Context<'_>) -> __futures_crate::task::Poll<Option<Self::Item>> {
                    match self.get_mut() {
                        #(
                            Self::#generic_idents(#generic_idents) => ::std::pin::Pin::new(#generic_idents).poll_next(cx)
                        ),*,
                        Self::None => panic!("StreamEnum::None should never be polled!"),
                    }
                }
            }

            impl<ITEM, #(#generic_idents),*> __futures_crate::stream::Stream for StreamSelect<#(#generic_idents),*>
            where #(#generic_idents: __futures_crate::stream::Stream<Item=ITEM> + ::std::marker::Unpin,)*
            {
                type Item = ITEM;

                fn poll_next(mut self: ::std::pin::Pin<&mut Self>, cx: &mut __futures_crate::task::Context<'_>) -> __futures_crate::task::Poll<Option<Self::Item>> {
                    let Self(#(ref mut #field_idents),*) = self.get_mut();
                    #(
                        let mut #field_idents_2 = false;
                    )*
                    let mut any_pending = false;
                    {
                        let mut stream_array = [#(#field_idents.as_mut().map(|f| StreamEnum::#generic_idents(f)).unwrap_or(StreamEnum::None)),*];
                        __futures_crate::async_await::shuffle(&mut stream_array);

                        for mut s in stream_array {
                            if let StreamEnum::None = s {
                                continue;
                            } else {
                                match __futures_crate::stream::Stream::poll_next(::std::pin::Pin::new(&mut s), cx) {
                                    r @ __futures_crate::task::Poll::Ready(Some(_)) => {
                                        return r;
                                    },
                                    __futures_crate::task::Poll::Pending => {
                                        any_pending = true;
                                    },
                                    __futures_crate::task::Poll::Ready(None) => {
                                        match s {
                                            #(
                                                StreamEnum::#generic_idents(_) => { #field_idents_2 = true; }
                                            ),*,
                                            StreamEnum::None => panic!("StreamEnum::None should never be polled!"),
                                        }
                                    },
                                }
                            }
                        }
                    }
                    #(
                        if #field_idents_2 {
                            *#field_idents = None;
                        }
                    )*
                    if any_pending {
                        __futures_crate::task::Poll::Pending
                    } else {
                        __futures_crate::task::Poll::Ready(None)
                    }
                }

                fn size_hint(&self) -> (usize, Option<usize>) {
                    let mut s = (0, Some(0));
                    #(
                        if let Some(new_hint) = self.#field_indices.as_ref().map(|s| s.size_hint()) {
                            s.0 += new_hint.0;
                            // We can change this out for `.zip` when the MSRV is 1.46.0 or higher.
                            s.1 = s.1.and_then(|a| new_hint.1.map(|b| a + b));
                        }
                    )*
                    s
                }
            }

            StreamSelect(#(Some(#args)),*)

        }
    })
}

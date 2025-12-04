use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Error, Expr, Ident, ItemFn, LitInt, Token,
};

#[proc_macro_attribute]
pub fn retry(args: TokenStream, input: TokenStream) -> TokenStream {
    let count = parse_macro_input!(args as LitInt);
    let input = parse_macro_input!(input as ItemFn);
    let attrs = &input.attrs;
    let name = &input.sig.ident;

    TokenStream::from(quote! {
        #(#attrs),*
        fn #name() {
            #input

            for i in 0..=#count {
                let result = std::panic::catch_unwind(|| {
                    #name();
                });

                if result.is_ok() {
                    return;
                }

                if i == #count {
                    std::panic::resume_unwind(result.unwrap_err());
                }
            }
        }
    })
}

#[proc_macro_attribute]
pub fn test_with_seed(args: TokenStream, input: TokenStream) -> TokenStream {
    struct Args {
        retry: LitInt,
        seed: Expr,
        seed_fn: Option<Ident>,
    }

    impl Parse for Args {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let mut retry = None;
            let mut seed = None;
            let mut seed_fn = None;

            while !input.is_empty() {
                let name = input.parse::<Ident>()?;
                match name.to_string().as_str() {
                    "retry" => {
                        input.parse::<Token![=]>()?;
                        retry.replace(input.parse()?);
                    }
                    "seed" => {
                        input.parse::<Token![=]>()?;
                        seed.replace(input.parse()?);
                    }
                    "seed_fn" => {
                        input.parse::<Token![=]>()?;
                        seed_fn.replace(input.parse()?);
                    }
                    x => {
                        return Err(Error::new(
                            name.span(),
                            format!("Unsupported parameter `{x}`"),
                        ))
                    }
                }

                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }

            if retry.is_none() {
                retry.replace(LitInt::new("0", Span::mixed_site()));
            }

            Ok(Self {
                retry: retry.expect("`retry` parameter is required"),
                seed: seed.expect("`seed` parameter is required"),
                seed_fn,
            })
        }
    }

    let Args {
        retry,
        seed,
        seed_fn,
    } = parse_macro_input!(args as Args);

    let seed_fn = seed_fn.iter();

    let func = parse_macro_input!(input as ItemFn);
    let attrs = &func.attrs;
    let name = &func.sig.ident;

    TokenStream::from(quote! {
        #[test]
        #(#attrs),*
        fn #name() {
            #func

            let mut seed = #seed;

            for i in 0..=#retry {
                let result = std::panic::catch_unwind(|| {
                    #name(seed);
                });

                if result.is_ok() {
                    return;
                }

                if i == #retry {
                    std::panic::resume_unwind(result.unwrap_err());
                }

                #(
                    seed = #seed_fn();
                )*

                if i < #retry {
                    println!("\nRetry {}/{} with a new seed {}", i + 1, #retry, seed);
                }
            }
        }
    })
}

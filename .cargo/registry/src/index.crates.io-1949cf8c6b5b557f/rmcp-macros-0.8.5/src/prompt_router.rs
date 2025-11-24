use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ImplItem, ItemImpl, Visibility, parse_quote};

#[derive(FromMeta, Debug, Default)]
#[darling(default)]
pub struct PromptRouterAttribute {
    pub router: Option<String>,
    pub vis: Option<Visibility>,
}

pub fn prompt_router(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attribute = if attr.is_empty() {
        Default::default()
    } else {
        let attr_args = darling::ast::NestedMeta::parse_meta_list(attr)?;
        PromptRouterAttribute::from_list(&attr_args)?
    };

    let mut impl_block = syn::parse2::<ItemImpl>(input)?;
    let self_ty = &impl_block.self_ty;

    let router_fn_ident = attribute
        .router
        .map(|s| format_ident!("{}", s))
        .unwrap_or_else(|| format_ident!("prompt_router"));
    let vis = attribute.vis.unwrap_or(Visibility::Inherited);

    let mut prompt_route_fn_calls = Vec::new();

    for item in &mut impl_block.items {
        if let ImplItem::Fn(fn_item) = item {
            let has_prompt_attr = fn_item.attrs.iter().any(|attr| {
                attr.path()
                    .segments
                    .last()
                    .map(|seg| seg.ident == "prompt")
                    .unwrap_or(false)
            });

            if has_prompt_attr {
                let fn_ident = &fn_item.sig.ident;
                let attr_fn_ident = format_ident!("{}_prompt_attr", fn_ident);

                // Check what parameters the function takes
                let mut param_names = Vec::new();
                let mut param_types = Vec::new();

                for input in &fn_item.sig.inputs {
                    if let syn::FnArg::Typed(pat_type) = input {
                        // Extract parameter pattern and type
                        param_types.push(&*pat_type.ty);
                        param_names.push(&*pat_type.pat);
                    }
                }

                // Use the exact same pattern as tool_router
                prompt_route_fn_calls.push(quote! {
                    .with_route((Self::#attr_fn_ident(), Self::#fn_ident))
                });
            }
        }
    }

    let router_fn: ImplItem = parse_quote! {
        #vis fn #router_fn_ident() -> rmcp::handler::server::router::prompt::PromptRouter<#self_ty> {
            rmcp::handler::server::router::prompt::PromptRouter::new()
                #(#prompt_route_fn_calls)*
        }
    };

    impl_block.items.push(router_fn);

    Ok(quote! {
        #impl_block
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_prompt_router_macro() -> syn::Result<()> {
        let input = quote! {
            impl MyPromptHandler {
                #[prompt]
                async fn greeting_prompt(&self) -> Result<Vec<PromptMessage>, Error> {
                    Ok(vec![])
                }

                #[prompt]
                async fn code_review_prompt(&self, Parameters(args): Parameters<CodeReviewArgs>) -> Result<Vec<PromptMessage>, Error> {
                    Ok(vec![])
                }
            }
        };

        let result = prompt_router(TokenStream::new(), input)?;
        let result_str = result.to_string();

        // Check that the prompt_router function was generated
        assert!(result_str.contains("fn prompt_router"));
        assert!(result_str.contains("PromptRouter :: new"));
        assert!(result_str.contains("greeting_prompt_prompt_attr"));
        assert!(result_str.contains("code_review_prompt_prompt_attr"));

        Ok(())
    }
}

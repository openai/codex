use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ImplItem, ItemImpl, parse_quote};

#[derive(FromMeta, Debug, Default)]
#[darling(default)]
pub struct PromptHandlerAttribute {
    pub router: Option<Expr>,
}

pub fn prompt_handler(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attribute = if attr.is_empty() {
        Default::default()
    } else {
        let attr_args = darling::ast::NestedMeta::parse_meta_list(attr)?;
        PromptHandlerAttribute::from_list(&attr_args)?
    };

    let mut impl_block = syn::parse2::<ItemImpl>(input)?;

    let router_expr = attribute
        .router
        .unwrap_or_else(|| syn::parse2(quote! { self.prompt_router }).unwrap());

    // Add get_prompt implementation
    let get_prompt_impl: ImplItem = parse_quote! {
        async fn get_prompt(
            &self,
            request: GetPromptRequestParam,
            context: RequestContext<RoleServer>,
        ) -> Result<GetPromptResult, rmcp::ErrorData> {
            let prompt_context = rmcp::handler::server::prompt::PromptContext::new(
                self,
                request.name,
                request.arguments,
                context,
            );
            #router_expr.get_prompt(prompt_context).await
        }
    };

    // Add list_prompts implementation
    let list_prompts_impl: ImplItem = parse_quote! {
        async fn list_prompts(
            &self,
            _request: Option<PaginatedRequestParam>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListPromptsResult, rmcp::ErrorData> {
            let prompts = #router_expr.list_all();
            Ok(ListPromptsResult {
                prompts,
                next_cursor: None,
            })
        }
    };

    // Check if methods already exist and replace them if they do
    let mut has_get_prompt = false;
    let mut has_list_prompts = false;

    for item in &mut impl_block.items {
        if let ImplItem::Fn(fn_item) = item {
            match fn_item.sig.ident.to_string().as_str() {
                "get_prompt" => {
                    *item = get_prompt_impl.clone();
                    has_get_prompt = true;
                }
                "list_prompts" => {
                    *item = list_prompts_impl.clone();
                    has_list_prompts = true;
                }
                _ => {}
            }
        }
    }

    // Add methods if they don't exist
    if !has_get_prompt {
        impl_block.items.push(get_prompt_impl);
    }
    if !has_list_prompts {
        impl_block.items.push(list_prompts_impl);
    }

    Ok(quote! {
        #impl_block
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_prompt_handler_macro() -> syn::Result<()> {
        let input = quote! {
            impl ServerHandler for MyPromptHandler {
                // Other handler methods...
            }
        };

        let result = prompt_handler(TokenStream::new(), input)?;
        let result_str = result.to_string();

        // Check that the required methods were generated
        assert!(result_str.contains("async fn get_prompt"));
        assert!(result_str.contains("PromptContext") && result_str.contains("new"));
        assert!(result_str.contains("async fn list_prompts"));
        assert!(result_str.contains("ListPromptsResult"));

        Ok(())
    }

    #[test]
    fn test_prompt_handler_with_custom_router() -> syn::Result<()> {
        let attr = quote! { router = self.get_prompt_router() };
        let input = quote! {
            impl ServerHandler for MyPromptHandler {
                // Other handler methods...
            }
        };

        let result = prompt_handler(attr, input)?;
        let result_str = result.to_string();

        // Check that the custom router expression is used
        assert!(
            result_str.contains("self")
                && result_str.contains("get_prompt_router")
                && result_str.contains("get_prompt")
        );
        assert!(
            result_str.contains("self")
                && result_str.contains("get_prompt_router")
                && result_str.contains("list_all")
        );

        Ok(())
    }
}

use darling::{FromMeta, ast::NestedMeta};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, ImplItem, ItemImpl};

#[derive(FromMeta)]
#[darling(default)]
pub struct ToolHandlerAttribute {
    pub router: Expr,
}

impl Default for ToolHandlerAttribute {
    fn default() -> Self {
        Self {
            router: syn::parse2(quote! {
                self.tool_router
            })
            .unwrap(),
        }
    }
}

pub fn tool_handler(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attr_args = NestedMeta::parse_meta_list(attr)?;
    let ToolHandlerAttribute { router } = ToolHandlerAttribute::from_list(&attr_args)?;
    let mut item_impl = syn::parse2::<ItemImpl>(input.clone())?;
    let tool_call_fn = quote! {
        async fn call_tool(
            &self,
            request: rmcp::model::CallToolRequestParam,
            context: rmcp::service::RequestContext<rmcp::RoleServer>,
        ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
            let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
            #router.call(tcc).await
        }
    };
    let tool_list_fn = quote! {
        async fn list_tools(
            &self,
            _request: Option<rmcp::model::PaginatedRequestParam>,
            _context: rmcp::service::RequestContext<rmcp::RoleServer>,
        ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
            Ok(rmcp::model::ListToolsResult::with_all_items(#router.list_all()))
        }
    };
    let tool_call_fn = syn::parse2::<ImplItem>(tool_call_fn)?;
    let tool_list_fn = syn::parse2::<ImplItem>(tool_list_fn)?;
    item_impl.items.push(tool_call_fn);
    item_impl.items.push(tool_list_fn);
    Ok(item_impl.into_token_stream())
}

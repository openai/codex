//! ```ignore
//! #[rmcp::tool_router(router)]
//! impl Handler {
//!
//! }
//! ```
//!

use darling::{FromMeta, ast::NestedMeta};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Ident, ImplItem, ItemImpl, Visibility};

#[derive(FromMeta)]
#[darling(default)]
pub struct ToolRouterAttribute {
    pub router: Ident,
    pub vis: Option<Visibility>,
}

impl Default for ToolRouterAttribute {
    fn default() -> Self {
        Self {
            router: format_ident!("tool_router"),
            vis: None,
        }
    }
}

pub fn tool_router(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attr_args = NestedMeta::parse_meta_list(attr)?;
    let ToolRouterAttribute { router, vis } = ToolRouterAttribute::from_list(&attr_args)?;
    let mut item_impl = syn::parse2::<ItemImpl>(input.clone())?;
    // find all function marked with `#[rmcp::tool]`
    let tool_attr_fns: Vec<_> = item_impl
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(fn_item) = item {
                fn_item
                    .attrs
                    .iter()
                    .any(|attr| {
                        attr.path()
                            .segments
                            .last()
                            .is_some_and(|seg| seg.ident == "tool")
                    })
                    .then_some(&fn_item.sig.ident)
            } else {
                None
            }
        })
        .collect();
    let mut routers = vec![];
    for handler in tool_attr_fns {
        let tool_attr_fn_ident = format_ident!("{handler}_tool_attr");
        routers.push(quote! {
            .with_route((Self::#tool_attr_fn_ident(), Self::#handler))
        })
    }
    let router_fn = syn::parse2::<ImplItem>(quote! {
        #vis fn #router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
            rmcp::handler::server::router::tool::ToolRouter::<Self>::new()
                #(#routers)*
        }
    })?;
    item_impl.items.push(router_fn);
    Ok(item_impl.into_token_stream())
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_router_attr() -> Result<(), Box<dyn std::error::Error>> {
        let attr = quote! {
            router = test_router,
            vis = "pub(crate)"
        };
        let attr_args = NestedMeta::parse_meta_list(attr)?;
        let ToolRouterAttribute { router, vis } = ToolRouterAttribute::from_list(&attr_args)?;
        println!("router: {}", router);
        if let Some(vis) = vis {
            println!("visibility: {}", vis.to_token_stream());
        } else {
            println!("visibility: None");
        }
        Ok(())
    }
}

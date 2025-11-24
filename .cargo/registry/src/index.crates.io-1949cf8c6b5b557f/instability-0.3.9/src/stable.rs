use darling::{ast::NestedMeta, Error, FromMeta};
use indoc::formatdoc;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_quote, Item};

use crate::item_like::{ItemLike, Stability};

pub fn stable_macro(args: TokenStream, input: TokenStream) -> TokenStream {
    let attributes = match NestedMeta::parse_meta_list(args) {
        Ok(attributes) => attributes,
        Err(err) => return Error::from(err).write_errors(),
    };
    let unstable_attribute = match StableAttribute::from_list(&attributes) {
        Ok(attributes) => attributes,
        Err(err) => return err.write_errors(),
    };
    match syn::parse2::<Item>(input) {
        Ok(item) => match item {
            Item::Type(item_type) => unstable_attribute.expand(item_type),
            Item::Enum(item_enum) => unstable_attribute.expand(item_enum),
            Item::Struct(item_struct) => unstable_attribute.expand(item_struct),
            Item::Fn(item_fn) => unstable_attribute.expand(item_fn),
            Item::Mod(item_mod) => unstable_attribute.expand(item_mod),
            Item::Trait(item_trait) => unstable_attribute.expand(item_trait),
            Item::Const(item_const) => unstable_attribute.expand(item_const),
            Item::Static(item_static) => unstable_attribute.expand(item_static),
            Item::Use(item_use) => unstable_attribute.expand(item_use),
            Item::Impl(item_impl) => unstable_attribute.expand_impl(item_impl),
            _ => panic!("unsupported item type"),
        },
        Err(err) => Error::from(err).write_errors(),
    }
}

#[derive(Debug, Default, FromMeta)]
pub struct StableAttribute {
    /// The version at which the item was stabilized.
    since: Option<String>,

    /// A link or reference to a tracking issue for the feature.
    issue: Option<String>,
}

impl StableAttribute {
    pub fn expand(&self, item: impl ItemLike + ToTokens + Clone) -> TokenStream {
        if !item.is_public() {
            // We only care about public items.
            return item.into_token_stream();
        }
        self.expand_impl(item)
    }

    pub fn expand_impl(&self, mut item: impl Stability + ToTokens) -> TokenStream {
        let doc = if let Some(ref version) = self.since {
            formatdoc! {"
                # Stability

                This API was stabilized in version {}.",
                version.trim_start_matches('v')
            }
        } else {
            formatdoc! {"
                # Stability

                This API is stable."}
        };
        item.push_attr(parse_quote! { #[doc = #doc] });

        if let Some(issue) = &self.issue {
            let doc = format!("The tracking issue is: `{}`.", issue);
            item.push_attr(parse_quote! { #[doc = #doc] });
        }
        item.into_token_stream()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::parse_quote;

    use super::*;

    #[test]
    fn expand_non_public_item() {
        let item: syn::ItemStruct = parse_quote! {
            struct MyStruct;
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item.clone());
        assert_eq!(tokens.to_string(), quote! { struct MyStruct; }.to_string());
    }

    const STABLE_DOC: &str = "# Stability\n\nThis API is stable.";
    const SINCE_DOC: &str = "# Stability\n\nThis API was stabilized in version 1.0.0.";
    const ISSUE_DOC: &str = "The tracking issue is: `#123`.";

    #[test]
    fn expand_with_since() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let stable = StableAttribute {
            since: Some("v1.0.0".to_string()),
            issue: None,
        };
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #SINCE_DOC]
            pub type Foo = Bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_with_issue() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let stable = StableAttribute {
            since: None,
            issue: Some("#123".to_string()),
        };
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            #[doc = #ISSUE_DOC]
            pub type Foo = Bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_with_since_and_issue() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let stable = StableAttribute {
            since: Some("v1.0.0".to_string()),
            issue: Some("#123".to_string()),
        };
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #SINCE_DOC]
            #[doc = #ISSUE_DOC]
            pub type Foo = Bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_type() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub type Foo = Bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_struct() {
        let item: syn::ItemStruct = parse_quote! {
            pub struct Foo {
                pub field: i32,
            }
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub struct Foo {
                pub field: i32,
            }
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_enum() {
        let item: syn::ItemEnum = parse_quote! {
            pub enum Foo {
                A,
                B,
            }
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub enum Foo {
                A,
                B,
            }
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_fn() {
        let item: syn::ItemFn = parse_quote! {
            pub fn foo() {}
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub fn foo() {}
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_trait() {
        let item: syn::ItemTrait = parse_quote! {
            pub trait Foo {
                fn bar(&self);
            }
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub trait Foo {
                fn bar(&self);
            }
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_const() {
        let item: syn::ItemConst = parse_quote! {
            pub const FOO: i32 = 42;
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub const FOO: i32 = 42;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_static() {
        let item: syn::ItemStatic = parse_quote! {
            pub static FOO: i32 = 42;
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub static FOO: i32 = 42;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_mod() {
        let item: syn::ItemMod = parse_quote! {
            pub mod foo {
                pub fn bar() {}
            }
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub mod foo {
                pub fn bar() {}
            }
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_use() {
        let item: syn::ItemUse = parse_quote! {
            pub use crate::foo::bar;
        };
        let stable = StableAttribute::default();
        let tokens = stable.expand(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            pub use crate::foo::bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_impl_block() {
        let item: syn::ItemImpl = parse_quote! {
            impl Default for crate::foo::Foo {}
        };
        let tokens = StableAttribute::default().expand_impl(item);
        let expected = quote! {
            #[doc = #STABLE_DOC]
            impl Default for crate::foo::Foo {}
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }
}

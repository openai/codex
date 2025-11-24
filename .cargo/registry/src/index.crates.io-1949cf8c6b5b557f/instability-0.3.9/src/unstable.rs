use darling::{ast::NestedMeta, Error, FromMeta};
use indoc::formatdoc;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_quote, Item};

use crate::item_like::{ItemLike, Stability};

pub fn unstable_macro(args: TokenStream, input: TokenStream) -> TokenStream {
    let attributes = match NestedMeta::parse_meta_list(args) {
        Ok(attributes) => attributes,
        Err(err) => return Error::from(err).write_errors(),
    };
    let unstable_attribute = match UnstableAttribute::from_list(&attributes) {
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
pub struct UnstableAttribute {
    /// The name of the feature that enables the unstable API.
    ///
    /// If not specified, the item will instead be guarded by a catch-all `unstable` feature.
    feature: Option<String>,

    /// A link or reference to a tracking issue for the unstable feature.
    ///
    /// This will be included in the item's documentation.
    issue: Option<String>,
}

impl UnstableAttribute {
    pub fn expand(&self, mut item: impl ItemLike + ToTokens + Clone) -> TokenStream {
        if !item.is_public() {
            // We only care about public items.
            return item.into_token_stream();
        }

        let feature_flag = self.feature_flag();
        self.add_doc(&mut item);

        let mut hidden_item = item.clone();
        hidden_item.set_visibility(parse_quote! { pub(crate) });

        let allows = item
            .allowed_lints()
            .into_iter()
            .map(|ident| quote! { #[allow(#ident)] });

        #[cfg(not(instability_disable_unstable_docs))]
        let (cfg_gate_unstable, cfg_gate_stable) = (
            quote! { #[cfg(any(doc, feature = #feature_flag))] },
            quote! { #[cfg(not(any(doc, feature = #feature_flag)))] },
        );

        #[cfg(instability_disable_unstable_docs)]
        let (cfg_gate_unstable, cfg_gate_stable) = (
            quote! { #[cfg(feature = #feature_flag)] },
            quote! { #[cfg(not(feature = #feature_flag))] },
        );

        quote! {
            #cfg_gate_unstable
            #[cfg_attr(docsrs, doc(cfg(feature = #feature_flag)))]
            #item

            #cfg_gate_stable
            #(#allows)*
            #hidden_item
        }
    }

    pub fn expand_impl(&self, mut item: impl Stability + ToTokens) -> TokenStream {
        let feature_flag = self.feature_flag();
        self.add_doc(&mut item);

        #[cfg(not(instability_disable_unstable_docs))]
        let cfg_gate_unstable = quote! { #[cfg(any(doc, feature = #feature_flag))] };

        #[cfg(instability_disable_unstable_docs)]
        let cfg_gate_unstable = quote! { #[cfg(feature = #feature_flag)] };

        quote! {
            #cfg_gate_unstable
            #[cfg_attr(docsrs, doc(cfg(feature = #feature_flag)))]
            #item
        }
    }

    fn add_doc(&self, item: &mut impl Stability) {
        let feature_flag = self.feature_flag();
        let doc = formatdoc! {"
            # Stability

            **This API is marked as unstable** and is only available when the `{feature_flag}`
            crate feature is enabled. This comes with no stability guarantees, and could be changed
            or removed at any time."};
        item.push_attr(parse_quote! { #[doc = #doc] });

        if let Some(issue) = &self.issue {
            let doc = format!("The tracking issue is: `{}`.", issue);
            item.push_attr(parse_quote! { #[doc = #doc] });
        }
    }

    fn feature_flag(&self) -> String {
        self.feature
            .as_deref()
            .map_or(String::from("unstable"), |name| format!("unstable-{name}"))
    }
}

#[cfg(all(test, not(instability_disable_unstable_docs)))]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::parse_quote;

    use super::*;

    #[test]
    fn unstable_feature_flag_default() {
        let unstable = UnstableAttribute::default();
        assert_eq!(unstable.feature_flag(), "unstable");
    }

    #[test]
    fn unstable_feature_flag_with_feature() {
        let unstable = UnstableAttribute {
            feature: Some("experimental".to_string()),
            issue: None,
        };
        assert_eq!(unstable.feature_flag(), "unstable-experimental");
    }

    #[test]
    fn expand_non_public_item() {
        let item: syn::ItemStruct = parse_quote! {
            struct MyStruct;
        };
        let unstable = UnstableAttribute::default();
        let tokens = unstable.expand(item.clone());
        assert_eq!(tokens.to_string(), quote! { struct MyStruct; }.to_string());
    }

    const DEFAULT_DOC: &str = "# Stability\n\n**This API is marked as unstable** and is only available when the `unstable`\ncrate feature is enabled. This comes with no stability guarantees, and could be changed\nor removed at any time.";
    const WITH_FEATURES_DOC: &str = "# Stability\n\n**This API is marked as unstable** and is only available when the `unstable-experimental`\ncrate feature is enabled. This comes with no stability guarantees, and could be changed\nor removed at any time.";
    const ISSUE_DOC: &str = "The tracking issue is: `#123`.";

    #[test]
    fn expand_with_feature() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let unstable = UnstableAttribute {
            feature: Some("experimental".to_string()),
            issue: None,
        };
        let tokens = unstable.expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable-experimental"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable-experimental")))]
            #[doc = #WITH_FEATURES_DOC]
            pub type Foo = Bar;

            #[cfg(not(any(doc, feature = "unstable-experimental")))]
            #[allow(dead_code)]
            #[doc = #WITH_FEATURES_DOC]
            pub(crate) type Foo = Bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_with_issue() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let unstable = UnstableAttribute {
            feature: None,
            issue: Some("#123".to_string()),
        };
        let tokens = unstable.expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            #[doc = #ISSUE_DOC]
            pub type Foo = Bar;

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            #[doc = #ISSUE_DOC]
            pub(crate) type Foo = Bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_type() {
        let item: syn::ItemType = parse_quote! { pub type Foo = Bar; };
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub type Foo = Bar;

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) type Foo = Bar;
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub struct Foo {
                pub field: i32,
            }

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) struct Foo {
                pub (crate) field: i32,
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub enum Foo {
                A,
                B,
            }

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) enum Foo {
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub fn foo() {}

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) fn foo() {}
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub trait Foo {
                fn bar(&self);
            }

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) trait Foo {
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub const FOO: i32 = 42;

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) const FOO: i32 = 42;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_static() {
        let item: syn::ItemStatic = parse_quote! {
            pub static FOO: i32 = 42;
        };
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub static FOO: i32 = 42;

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) static FOO: i32 = 42;
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub mod foo {
                pub fn bar() {}
            }

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) mod foo {
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
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub use crate::foo::bar;

            #[cfg(not(any(doc, feature = "unstable")))]
            #[allow(unused_imports)]
            #[doc = #DEFAULT_DOC]
            pub(crate) use crate::foo::bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_impl_block() {
        let item: syn::ItemImpl = parse_quote! {
            impl Default for crate::foo::Foo {}
        };
        let tokens = UnstableAttribute::default().expand_impl(item);
        let expected = quote! {
            #[cfg(any(doc, feature = "unstable"))]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            impl Default for crate::foo::Foo {}
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }
}

#[cfg(all(test, instability_disable_unstable_docs))]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::parse_quote;

    use super::*;

    const DEFAULT_DOC: &str = "# Stability\n\n**This API is marked as unstable** and is only available when the `unstable`\ncrate feature is enabled. This comes with no stability guarantees, and could be changed\nor removed at any time.";

    #[test]
    fn expand_public_fn() {
        let item: syn::ItemFn = parse_quote! {
            pub fn foo() {}
        };
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(feature = "unstable")]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub fn foo() {}

            #[cfg(not(feature = "unstable"))]
            #[allow(dead_code)]
            #[doc = #DEFAULT_DOC]
            pub(crate) fn foo() {}
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn expand_public_use() {
        let item: syn::ItemUse = parse_quote! {
            pub use crate::foo::bar;
        };
        let tokens = UnstableAttribute::default().expand(item);
        let expected = quote! {
            #[cfg(feature = "unstable")]
            #[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
            #[doc = #DEFAULT_DOC]
            pub use crate::foo::bar;

            #[cfg(not(feature = "unstable"))]
            #[allow(unused_imports)]
            #[doc = #DEFAULT_DOC]
            pub(crate) use crate::foo::bar;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }
}

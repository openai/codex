use syn::Visibility;

pub trait Stability {
    #[allow(unused)]
    fn attrs(&self) -> &[syn::Attribute];

    fn push_attr(&mut self, attr: syn::Attribute);
}

pub trait ItemLike: Stability {
    fn visibility(&self) -> &Visibility;

    fn set_visibility(&mut self, visibility: Visibility);

    fn is_public(&self) -> bool {
        matches!(self.visibility(), Visibility::Public(_))
    }

    fn allowed_lints(&self) -> Vec<syn::Ident>;
}

/// Implement `ItemLike` for the given type.
///
/// This makes each of the syn::Item* types implement our `ItemLike` trait to make it possible to
/// work with them in a more uniform way.
///
/// A single type can be passed to this macro, or multiple types can be passed at once.
/// Each type can be passed with a list of lints that are allowed for that type (defaulting to
/// `dead_code` if not specified).
macro_rules! impl_item_like {
    // run impl_item_like for each item in a list of items
    ($($(#[allow($($lint:ident),*)])? $ty:ty ),+ ,) => {
        $(
            impl_item_like!($(#[allow($($lint),*)])? $ty );
        )*
    };

    // run impl_item_like for a single item without any lints
    ($ty:ty) => {
        impl_item_like!(#[allow(dead_code)] $ty );
    };

    // Implement `ItemLike` for the given type.
    (#[allow($($lint:ident),*)] $ty:ty) => {
        impl Stability for $ty {
            fn attrs(&self) -> &[syn::Attribute] {
                &self.attrs
            }

            fn push_attr(&mut self, attr: syn::Attribute) {
                self.attrs.push(attr);
            }
        }

        impl ItemLike for $ty {
            fn visibility(&self) -> &Visibility {
                &self.vis
            }

            fn set_visibility(&mut self, visibility: Visibility) {
                self.vis = visibility;
            }

            fn allowed_lints(&self) -> Vec<syn::Ident> {
                vec![
                    $(syn::Ident::new(stringify!($lint), proc_macro2::Span::call_site()),)*
                ]
            }
        }
    };

}

impl_item_like!(
    syn::ItemType,
    syn::ItemEnum,
    syn::ItemFn,
    syn::ItemMod,
    syn::ItemTrait,
    syn::ItemConst,
    syn::ItemStatic,
    #[allow(unused_imports)]
    syn::ItemUse,
);

impl Stability for syn::ItemStruct {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.attrs
    }

    fn push_attr(&mut self, attr: syn::Attribute) {
        self.attrs.push(attr);
    }
}

impl ItemLike for syn::ItemStruct {
    fn visibility(&self) -> &Visibility {
        &self.vis
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        // Also constrain visibility of all fields to be at most the given
        // item visibility.
        self.fields
            .iter_mut()
            .filter(|field| matches!(&field.vis, Visibility::Public(_)))
            .for_each(|field| field.vis = visibility.clone());

        self.vis = visibility;
    }

    fn allowed_lints(&self) -> Vec<syn::Ident> {
        vec![syn::Ident::new("dead_code", proc_macro2::Span::call_site())]
    }
}

impl Stability for syn::ItemImpl {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.attrs
    }

    fn push_attr(&mut self, attr: syn::Attribute) {
        self.attrs.push(attr);
    }
}

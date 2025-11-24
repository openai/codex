use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use zvariant_utils::def_attrs;

pub fn zvariant_path() -> TokenStream {
    if let Ok(FoundCrate::Name(name)) = crate_name("zvariant") {
        let ident = format_ident!("{}", name);
        quote! { ::#ident }
    } else if let Ok(FoundCrate::Name(name)) = crate_name("zbus") {
        let ident = format_ident!("{}", name);
        quote! { ::#ident::zvariant }
    } else {
        quote! { ::zvariant }
    }
}

def_attrs! {
    crate zvariant;

    /// Attributes defined on structures.
    pub StructAttributes("struct") { signature str, rename_all str, deny_unknown_fields none };
    /// Attributes defined on fields.
    pub FieldAttributes("field") { rename str };
}

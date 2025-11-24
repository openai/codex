use proc_macro2::{Delimiter, Group};
use quote::quote;

#[test]
fn main() {
    // Okay. Rustc allows top-level `static` with no value syntactically, but
    // not semantically. Syn parses as Item::Verbatim.
    let tokens = quote! {
        pub static FOO: usize;
        pub static BAR: usize;
    };
    let file = syn::parse2::<syn::File>(tokens).unwrap();
    println!("{:#?}", file);

    // Okay.
    let inner = Group::new(
        Delimiter::None,
        quote!(static FOO: usize = 0; pub static BAR: usize = 0),
    );
    let tokens = quote!(pub #inner;);
    let file = syn::parse2::<syn::File>(tokens).unwrap();
    println!("{:#?}", file);

    // Formerly parser crash.
    let inner = Group::new(
        Delimiter::None,
        quote!(static FOO: usize; pub static BAR: usize),
    );
    let tokens = quote!(pub #inner;);
    let file = syn::parse2::<syn::File>(tokens).unwrap();
    println!("{:#?}", file);
}

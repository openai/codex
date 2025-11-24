use super::*;

ast_struct! {
    /// A complete file of Rust source code.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    ///
    /// # Example
    ///
    /// Parse a Rust source file into a `syn::File` and print out a debug
    /// representation of the syntax tree.
    ///
    /// ```
    /// use std::env;
    /// use std::fs::File;
    /// use std::io::Read;
    /// use std::process;
    ///
    /// fn main() {
    /// # }
    /// #
    /// # fn fake_main() {
    ///     let mut args = env::args();
    ///     let _ = args.next(); // executable name
    ///
    ///     let filename = match (args.next(), args.next()) {
    ///         (Some(filename), None) => filename,
    ///         _ => {
    ///             eprintln!("Usage: dump-syntax path/to/filename.rs");
    ///             process::exit(1);
    ///         }
    ///     };
    ///
    ///     let mut file = File::open(&filename).expect("Unable to open file");
    ///
    ///     let mut src = String::new();
    ///     file.read_to_string(&mut src).expect("Unable to read file");
    ///
    ///     let syntax = syn::parse_file(&src).expect("Unable to parse file");
    ///
    ///     // Debug impl is available if Syn is built with "extra-traits" feature.
    ///     println!("{:#?}", syntax);
    /// }
    /// ```
    ///
    /// Running with its own source code as input, this program prints output
    /// that begins with:
    ///
    /// ```text
    /// File {
    ///     shebang: None,
    ///     attrs: [],
    ///     items: [
    ///         Use(
    ///             ItemUse {
    ///                 attrs: [],
    ///                 vis: Inherited,
    ///                 use_token: Use,
    ///                 leading_colon: None,
    ///                 tree: Path(
    ///                     UsePath {
    ///                         ident: Ident(
    ///                             std,
    ///                         ),
    ///                         colon2_token: Colon2,
    ///                         tree: Name(
    ///                             UseName {
    ///                                 ident: Ident(
    ///                                     env,
    ///                                 ),
    ///                             },
    ///                         ),
    ///                     },
    ///                 ),
    ///                 semi_token: Semi,
    ///             },
    ///         ),
    /// ...
    /// ```
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct File {
        pub shebang: Option<String>,
        pub attrs: Vec<Attribute>,
        pub items: Vec<Item>,
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::parse::{Parse, ParseStream, Result};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for File {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(File {
                shebang: None,
                attrs: input.call(Attribute::parse_inner)?,
                items: {
                    let mut items = Vec::new();
                    while !input.is_empty() {
                        items.push(input.parse()?);
                    }
                    items
                },
            })
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use crate::attr::FilterAttrs;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, TokenStreamExt};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for File {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.inner());
            tokens.append_all(&self.items);
        }
    }
}

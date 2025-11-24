#![allow(unused_macros, unused_macro_rules)]

#[path = "../debug/mod.rs"]
pub mod debug;

use std::str::FromStr;
use syn::parse::Result;

macro_rules! errorf {
    ($($tt:tt)*) => {{
        use ::std::io::Write;
        let stderr = ::std::io::stderr();
        write!(stderr.lock(), $($tt)*).unwrap();
    }};
}

macro_rules! punctuated {
    ($($e:expr,)+) => {{
        let mut seq = ::syn::punctuated::Punctuated::new();
        $(
            seq.push($e);
        )+
        seq
    }};

    ($($e:expr),+) => {
        punctuated!($($e,)+)
    };
}

macro_rules! snapshot {
    ($($args:tt)*) => {
        snapshot_impl!(() $($args)*)
    };
}

macro_rules! snapshot_impl {
    (($expr:ident) as $t:ty, @$snapshot:literal) => {
        let tokens = crate::macros::TryIntoTokens::try_into_tokens($expr).unwrap();
        let $expr: $t = syn::parse_quote!(#tokens);
        let debug = crate::macros::debug::Lite(&$expr);
        if !cfg!(miri) {
            #[allow(clippy::needless_raw_string_hashes)] // https://github.com/mitsuhiko/insta/issues/389
            {
                insta::assert_debug_snapshot!(debug, @$snapshot);
            }
        }
    };
    (($($expr:tt)*) as $t:ty, @$snapshot:literal) => {{
        let tokens = crate::macros::TryIntoTokens::try_into_tokens($($expr)*).unwrap();
        let syntax_tree: $t = syn::parse_quote!(#tokens);
        let debug = crate::macros::debug::Lite(&syntax_tree);
        if !cfg!(miri) {
            #[allow(clippy::needless_raw_string_hashes)]
            {
                insta::assert_debug_snapshot!(debug, @$snapshot);
            }
        }
        syntax_tree
    }};
    (($($expr:tt)*) , @$snapshot:literal) => {{
        let syntax_tree = $($expr)*;
        let debug = crate::macros::debug::Lite(&syntax_tree);
        if !cfg!(miri) {
            #[allow(clippy::needless_raw_string_hashes)]
            {
                insta::assert_debug_snapshot!(debug, @$snapshot);
            }
        }
        syntax_tree
    }};
    (($($expr:tt)*) $next:tt $($rest:tt)*) => {
        snapshot_impl!(($($expr)* $next) $($rest)*)
    };
}

pub trait TryIntoTokens {
    #[allow(dead_code)]
    fn try_into_tokens(self) -> Result<proc_macro2::TokenStream>;
}

impl TryIntoTokens for &str {
    fn try_into_tokens(self) -> Result<proc_macro2::TokenStream> {
        let tokens = proc_macro2::TokenStream::from_str(self)?;
        Ok(tokens)
    }
}

impl TryIntoTokens for proc_macro2::TokenStream {
    fn try_into_tokens(self) -> Result<proc_macro2::TokenStream> {
        Ok(self)
    }
}

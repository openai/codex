#![allow(unused_macros, unused_macro_rules)]

#[path = "../debug/mod.rs"]
pub mod debug;

use syn::parse::{Parse, Result};

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
        let $expr = crate::macros::Tokens::parse::<$t>($expr).unwrap();
        let debug = crate::macros::debug::Lite(&$expr);
        if !cfg!(miri) {
            insta::assert_debug_snapshot!(debug, @$snapshot);
        }
    };
    (($($expr:tt)*) as $t:ty, @$snapshot:literal) => {{
        let syntax_tree = crate::macros::Tokens::parse::<$t>($($expr)*).unwrap();
        let debug = crate::macros::debug::Lite(&syntax_tree);
        if !cfg!(miri) {
            insta::assert_debug_snapshot!(debug, @$snapshot);
        }
        syntax_tree
    }};
    (($($expr:tt)*) , @$snapshot:literal) => {{
        let syntax_tree = $($expr)*;
        let debug = crate::macros::debug::Lite(&syntax_tree);
        if !cfg!(miri) {
            insta::assert_debug_snapshot!(debug, @$snapshot);
        }
        syntax_tree
    }};
    (($($expr:tt)*) $next:tt $($rest:tt)*) => {
        snapshot_impl!(($($expr)* $next) $($rest)*)
    };
}

pub trait Tokens {
    fn parse<T: Parse>(self) -> Result<T>;
}

impl<'a> Tokens for &'a str {
    fn parse<T: Parse>(self) -> Result<T> {
        syn::parse_str(self)
    }
}

impl Tokens for proc_macro2::TokenStream {
    fn parse<T: Parse>(self) -> Result<T> {
        syn::parse2(self)
    }
}

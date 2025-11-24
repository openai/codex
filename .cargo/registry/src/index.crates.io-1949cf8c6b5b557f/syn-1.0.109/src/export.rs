pub use std::clone::Clone;
pub use std::cmp::{Eq, PartialEq};
pub use std::default::Default;
pub use std::fmt::{self, Debug, Formatter};
pub use std::hash::{Hash, Hasher};
pub use std::marker::Copy;
pub use std::option::Option::{None, Some};
pub use std::result::Result::{Err, Ok};

#[cfg(feature = "printing")]
pub extern crate quote;

pub use proc_macro2::{Span, TokenStream as TokenStream2};

#[cfg(feature = "parsing")]
pub use crate::group::{parse_braces, parse_brackets, parse_parens};

pub use crate::span::IntoSpans;

#[cfg(all(
    not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
    feature = "proc-macro"
))]
pub use proc_macro::TokenStream;

#[cfg(feature = "printing")]
pub use quote::{ToTokens, TokenStreamExt};

#[allow(non_camel_case_types)]
pub type bool = help::Bool;
#[allow(non_camel_case_types)]
pub type str = help::Str;

mod help {
    pub type Bool = bool;
    pub type Str = str;
}

pub struct private(pub(crate) ());

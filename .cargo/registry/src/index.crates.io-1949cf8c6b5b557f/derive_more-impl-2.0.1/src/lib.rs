#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![recursion_limit = "128"]
#![cfg_attr(any(not(docsrs), ci), deny(rustdoc::all))]
#![forbid(non_ascii_idents, unsafe_code)]
#![warn(clippy::nonstandard_macro_braces)]

use proc_macro::TokenStream;
use syn::parse::Error as ParseError;

mod utils;

#[cfg(any(feature = "add_assign", feature = "mul_assign"))]
mod add_assign_like;
#[cfg(any(
    feature = "add",
    feature = "add_assign",
    feature = "mul",
    feature = "mul_assign",
))]
mod add_helpers;
#[cfg(any(feature = "add", feature = "mul"))]
mod add_like;
#[cfg(feature = "as_ref")]
mod r#as;
#[cfg(feature = "constructor")]
mod constructor;
#[cfg(feature = "deref")]
mod deref;
#[cfg(feature = "deref_mut")]
mod deref_mut;
#[cfg(feature = "error")]
mod error;
#[cfg(any(feature = "debug", feature = "display"))]
mod fmt;
#[cfg(feature = "from")]
mod from;
#[cfg(feature = "from_str")]
mod from_str;
#[cfg(feature = "index")]
mod index;
#[cfg(feature = "index_mut")]
mod index_mut;
#[cfg(feature = "into")]
mod into;
#[cfg(feature = "into_iterator")]
mod into_iterator;
#[cfg(feature = "is_variant")]
mod is_variant;
#[cfg(feature = "mul_assign")]
mod mul_assign_like;
#[cfg(any(feature = "mul", feature = "mul_assign"))]
mod mul_helpers;
#[cfg(feature = "mul")]
mod mul_like;
#[cfg(feature = "not")]
mod not_like;
#[cfg(any(feature = "debug", feature = "display"))]
pub(crate) mod parsing;
#[cfg(feature = "sum")]
mod sum_like;
#[cfg(feature = "try_from")]
mod try_from;
#[cfg(feature = "try_into")]
mod try_into;
#[cfg(feature = "try_unwrap")]
mod try_unwrap;
#[cfg(feature = "unwrap")]
mod unwrap;

// This trait describes the possible return types of
// the derives. A derive can generally be infallible and
// return a TokenStream, or it can be fallible and return
// a Result<TokenStream, syn::parse::Error>.
//
// This trait can be unused if no feature is enabled. We already error in that case but this
// warning distracts from the actual error.
#[allow(dead_code)]
trait Output {
    fn process(self) -> TokenStream;
}

impl Output for proc_macro2::TokenStream {
    fn process(self) -> TokenStream {
        self.into()
    }
}

impl Output for Result<proc_macro2::TokenStream, ParseError> {
    fn process(self) -> TokenStream {
        match self {
            Ok(ts) => ts.into(),
            Err(e) => e.to_compile_error().into(),
        }
    }
}

macro_rules! create_derive(
    ($feature:literal, $mod_:ident $(:: $mod_rest:ident)*, $trait_:ident, $fn_name: ident $(,$attribute:ident)* $(,)?) => {
        #[cfg(feature = $feature)]
        #[proc_macro_derive($trait_, attributes($($attribute),*))]
        #[doc = include_str!(concat!("../doc/", $feature, ".md"))]
        pub fn $fn_name(input: TokenStream) -> TokenStream {
            let ast = syn::parse(input).unwrap();
            Output::process($mod_$(:: $mod_rest)*::expand(&ast, stringify!($trait_)))
        }
    }
);

create_derive!("add", add_like, Add, add_derive);
create_derive!("add", add_like, Sub, sub_derive);
create_derive!("add", add_like, BitAnd, bit_and_derive);
create_derive!("add", add_like, BitOr, bit_or_derive);
create_derive!("add", add_like, BitXor, bit_xor_derive);

create_derive!("add_assign", add_assign_like, AddAssign, add_assign_derive,);
create_derive!("add_assign", add_assign_like, SubAssign, sub_assign_derive,);
create_derive!(
    "add_assign",
    add_assign_like,
    BitAndAssign,
    bit_and_assign_derive,
);
create_derive!(
    "add_assign",
    add_assign_like,
    BitOrAssign,
    bit_or_assign_derive,
);
create_derive!(
    "add_assign",
    add_assign_like,
    BitXorAssign,
    bit_xor_assign_derive,
);

create_derive!("as_ref", r#as::r#mut, AsMut, as_mut_derive, as_mut);
create_derive!("as_ref", r#as::r#ref, AsRef, as_ref_derive, as_ref);

create_derive!("constructor", constructor, Constructor, constructor_derive);

create_derive!("debug", fmt::debug, Debug, debug_derive, debug);

create_derive!("deref", deref, Deref, deref_derive, deref);

create_derive!(
    "deref_mut",
    deref_mut,
    DerefMut,
    deref_mut_derive,
    deref_mut,
);

create_derive!("display", fmt::display, Display, display_derive, display);
create_derive!("display", fmt::display, Binary, binary_derive, binary);
create_derive!("display", fmt::display, Octal, octal_derive, octal);
create_derive!(
    "display",
    fmt::display,
    LowerHex,
    lower_hex_derive,
    lower_hex,
);
create_derive!(
    "display",
    fmt::display,
    UpperHex,
    upper_hex_derive,
    upper_hex,
);
create_derive!(
    "display",
    fmt::display,
    LowerExp,
    lower_exp_derive,
    lower_exp,
);
create_derive!(
    "display",
    fmt::display,
    UpperExp,
    upper_exp_derive,
    upper_exp,
);
create_derive!("display", fmt::display, Pointer, pointer_derive, pointer);

create_derive!("error", error, Error, error_derive, error);

create_derive!("from", from, From, from_derive, from);

create_derive!("from_str", from_str, FromStr, from_str_derive);

create_derive!("index", index, Index, index_derive, index);

create_derive!(
    "index_mut",
    index_mut,
    IndexMut,
    index_mut_derive,
    index_mut,
);

create_derive!("into", into, Into, into_derive, into);

create_derive!(
    "into_iterator",
    into_iterator,
    IntoIterator,
    into_iterator_derive,
    into_iterator,
);

create_derive!(
    "is_variant",
    is_variant,
    IsVariant,
    is_variant_derive,
    is_variant,
);

create_derive!("mul", mul_like, Mul, mul_derive, mul);
create_derive!("mul", mul_like, Div, div_derive, div);
create_derive!("mul", mul_like, Rem, rem_derive, rem);
create_derive!("mul", mul_like, Shr, shr_derive, shr);
create_derive!("mul", mul_like, Shl, shl_derive, shl);

create_derive!(
    "mul_assign",
    mul_assign_like,
    MulAssign,
    mul_assign_derive,
    mul_assign,
);
create_derive!(
    "mul_assign",
    mul_assign_like,
    DivAssign,
    div_assign_derive,
    div_assign,
);
create_derive!(
    "mul_assign",
    mul_assign_like,
    RemAssign,
    rem_assign_derive,
    rem_assign,
);
create_derive!(
    "mul_assign",
    mul_assign_like,
    ShrAssign,
    shr_assign_derive,
    shr_assign,
);
create_derive!(
    "mul_assign",
    mul_assign_like,
    ShlAssign,
    shl_assign_derive,
    shl_assign,
);

create_derive!("not", not_like, Not, not_derive);
create_derive!("not", not_like, Neg, neg_derive);

create_derive!("sum", sum_like, Sum, sum_derive);
create_derive!("sum", sum_like, Product, product_derive);

create_derive!("try_from", try_from, TryFrom, try_from_derive, try_from);

create_derive!("try_into", try_into, TryInto, try_into_derive, try_into);

create_derive!(
    "try_unwrap",
    try_unwrap,
    TryUnwrap,
    try_unwrap_derive,
    try_unwrap,
);

create_derive!("unwrap", unwrap, Unwrap, unwrap_derive, unwrap);

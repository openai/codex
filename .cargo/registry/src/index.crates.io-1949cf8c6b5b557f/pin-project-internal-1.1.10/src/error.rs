// SPDX-License-Identifier: Apache-2.0 OR MIT

macro_rules! format_err {
    ($span:expr, $msg:expr $(,)?) => {
        syn::Error::new_spanned(&$span as &dyn quote::ToTokens, &$msg as &dyn std::fmt::Display)
    };
    ($span:expr, $($tt:tt)*) => {
        format_err!($span, format!($($tt)*))
    };
}

macro_rules! bail {
    ($($tt:tt)*) => {
        return Err(format_err!($($tt)*))
    };
}

#![allow(
    clippy::no_effect_underscore_binding,
    clippy::too_many_lines,
    clippy::used_underscore_binding
)]

#[rustfmt::skip]
mod gen;

use proc_macro2::{Ident, Literal, TokenStream};
use ref_cast::RefCast;
use std::fmt::{self, Debug};
use std::ops::Deref;
use syn::punctuated::Punctuated;

#[derive(RefCast)]
#[repr(transparent)]
pub struct Lite<T: ?Sized> {
    value: T,
}

#[allow(non_snake_case)]
pub fn Lite<T: ?Sized>(value: &T) -> &Lite<T> {
    Lite::ref_cast(value)
}

impl<T: ?Sized> Deref for Lite<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl Debug for Lite<bool> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.value)
    }
}

impl Debug for Lite<u32> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.value)
    }
}

impl Debug for Lite<usize> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.value)
    }
}

impl Debug for Lite<String> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:?}", self.value)
    }
}

impl Debug for Lite<Ident> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:?}", self.value.to_string())
    }
}

impl Debug for Lite<Literal> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.value)
    }
}

impl Debug for Lite<TokenStream> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let string = self.value.to_string();
        if string.len() <= 80 {
            write!(formatter, "TokenStream(`{}`)", self.value)
        } else {
            formatter
                .debug_tuple("TokenStream")
                .field(&format_args!("`{}`", string))
                .finish()
        }
    }
}

impl<T> Debug for Lite<&T>
where
    Lite<T>: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(Lite(self.value), formatter)
    }
}

impl<T> Debug for Lite<Box<T>>
where
    Lite<T>: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(Lite(&*self.value), formatter)
    }
}

impl<T> Debug for Lite<Vec<T>>
where
    Lite<T>: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_list()
            .entries(self.value.iter().map(Lite))
            .finish()
    }
}

impl<T, P> Debug for Lite<Punctuated<T, P>>
where
    Lite<T>: Debug,
    Lite<P>: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut list = formatter.debug_list();
        for pair in self.pairs() {
            let (node, punct) = pair.into_tuple();
            list.entry(Lite(node));
            list.entries(punct.map(Lite));
        }
        list.finish()
    }
}

struct Present;

impl Debug for Present {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Some")
    }
}

struct Option {
    present: bool,
}

impl Debug for Option {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(if self.present { "Some" } else { "None" })
    }
}

use std::num::NonZero;

use proc_macro::{Group, Ident, Literal, Punct, Span, TokenStream, TokenTree};

/// Turn a type into a [`TokenStream`].
pub(crate) trait ToTokenStream: Sized {
    fn append_to(self, ts: &mut TokenStream);
    fn into_token_stream(self) -> TokenStream {
        let mut ts = TokenStream::new();
        self.append_to(&mut ts);
        ts
    }
}

pub(crate) trait ToTokenTree: Sized {
    fn into_token_tree(self) -> TokenTree;
}

impl<T: ToTokenTree> ToTokenStream for T {
    fn append_to(self, ts: &mut TokenStream) {
        ts.extend([self.into_token_tree()])
    }
}

impl ToTokenTree for bool {
    fn into_token_tree(self) -> TokenTree {
        let lit = if self { "true" } else { "false" };
        TokenTree::Ident(Ident::new(lit, Span::mixed_site()))
    }
}

impl ToTokenStream for TokenStream {
    fn append_to(self, ts: &mut TokenStream) {
        ts.extend(self)
    }
}

impl ToTokenTree for TokenTree {
    fn into_token_tree(self) -> TokenTree {
        self
    }
}

impl ToTokenTree for &str {
    fn into_token_tree(self) -> TokenTree {
        TokenTree::Literal(Literal::string(self))
    }
}

impl ToTokenTree for NonZero<u16> {
    fn into_token_tree(self) -> TokenTree {
        quote_group! {{
            unsafe { ::core::num::NonZero::<u16>::new_unchecked(#(self.get())) }
        }}
    }
}

macro_rules! impl_for_tree_types {
    ($($type:ty)*) => {$(
        impl ToTokenTree for $type {
            fn into_token_tree(self) -> TokenTree {
                TokenTree::from(self)
            }
        }
    )*};
}
impl_for_tree_types![Ident Literal Group Punct];

macro_rules! impl_for_int {
    ($($type:ty => $method:ident)*) => {$(
        impl ToTokenTree for $type {
            fn into_token_tree(self) -> TokenTree {
                TokenTree::from(Literal::$method(self))
            }
        }
    )*};
}
impl_for_int! {
    i8 => i8_unsuffixed
    u8 => u8_unsuffixed
    u16 => u16_unsuffixed
    i32 => i32_unsuffixed
    u32 => u32_unsuffixed
}

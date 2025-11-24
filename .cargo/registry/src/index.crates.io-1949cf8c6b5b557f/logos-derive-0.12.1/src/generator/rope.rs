use proc_macro2::TokenStream;
use quote::quote;

use crate::generator::{Context, Generator};
use crate::graph::Rope;

impl<'a> Generator<'a> {
    pub fn generate_rope(&mut self, rope: &Rope, mut ctx: Context) -> TokenStream {
        let miss = ctx.miss(rope.miss.first(), self);
        let read = ctx.read(rope.pattern.len());
        let then = self.goto(rope.then, ctx.advance(rope.pattern.len()));

        let pat = match rope.pattern.to_bytes() {
            Some(bytes) => byte_slice_literal(&bytes),
            None => {
                let ranges = rope.pattern.iter();

                quote!([#(#ranges),*])
            }
        };

        return quote! {
            match #read {
                Some(#pat) => #then,
                _ => #miss,
            }
        };
    }
}

fn byte_slice_literal(bytes: &[u8]) -> TokenStream {
    if bytes.iter().any(|&b| b < 0x20 || b >= 0x7F) {
        return quote!(&[#(#bytes),*]);
    }

    let slice = std::str::from_utf8(bytes).unwrap();

    syn::parse_str(&format!("b{:?}", slice)).unwrap()
}

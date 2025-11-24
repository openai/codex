use crate::util::ToIdent;
use proc_macro2::{Literal, TokenStream};
use quote::{quote, ToTokens};
use syn::Ident;

pub struct TableStack {
    tables: Vec<(Ident, [u8; 256])>,
    shift: u8,
}

pub struct TableView<'a> {
    ident: &'a Ident,
    table: &'a mut [u8; 256],
    mask: u8,
}

impl TableStack {
    pub fn new() -> Self {
        TableStack {
            tables: vec![("COMPACT_TABLE_0".to_ident(), [0; 256])],
            shift: 0,
        }
    }

    pub fn view(&mut self) -> TableView {
        let mask = if self.shift < 8 {
            // Reusing existing table with a shifted mask
            let mask = 1u8 << self.shift;

            self.shift += 1;

            mask
        } else {
            // Need to create a new table
            let ident = format!("COMPACT_TABLE_{}", self.tables.len()).to_ident();

            self.tables.push((ident, [0; 256]));
            self.shift = 1;

            1
        };

        let (ref ident, ref mut table) = self.tables.last_mut().unwrap();

        TableView { ident, table, mask }
    }
}

impl<'a> TableView<'a> {
    pub fn ident(&self) -> &'a Ident {
        self.ident
    }

    pub fn flag(&mut self, byte: u8) {
        self.table[byte as usize] |= self.mask;
    }

    pub fn mask(&self) -> Literal {
        Literal::u8_unsuffixed(self.mask)
    }
}

impl ToTokens for TableStack {
    fn to_tokens(&self, out: &mut TokenStream) {
        if self.shift == 0 {
            return;
        }

        for (ident, table) in self.tables.iter() {
            let bytes = table.iter().copied().map(Literal::u8_unsuffixed);

            out.extend(quote! {
                static #ident: [u8; 256] = [#(#bytes),*];
            });
        }
    }
}

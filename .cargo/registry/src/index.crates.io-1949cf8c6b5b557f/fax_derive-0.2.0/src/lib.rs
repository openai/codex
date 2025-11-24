use proc_macro2::{TokenStream, Span, Literal};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Expr, Error, parse::{ParseStream, Parse}, Result, Token, Ident,
    punctuated::Punctuated, braced, Type,
};
use std::fmt::{self, Debug};
use std::ops::Range;

#[proc_macro]
pub fn bitmaps(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as BitMaps);

    // Build the output, possibly using quasi-quotation
    let maps = input.maps.iter().map(|map| {
        let (code, root) = map.build();
        let name = &map.name;
        let typ = match map.typ {
            Some(ref t) => quote!{ #t },
            None => quote! { u16 }
        };
        let list = map.entries.iter().map(|entry| {
            let bits = entry.bits;
            let val = &entry.value;
            quote!{ (#val, #bits) }
        });
        let arms = map.entries.iter().map(|entry| {
            let bits = entry.bits;
            let val = &entry.value;
            quote!{ #val => #bits }
        });
        let n = map.entries.len();

        quote! {
            pub mod #name {
                pub use super::*;
                use crate::{BitReader, Bits};

                #code

                pub fn decode(reader: &mut impl BitReader) -> Option<#typ> {
                    let root = #root;
                    root.find(reader)
                }

                pub fn encode(val: #typ) -> Option<Bits> {
                    let bits = match val {
                        #(#arms,)*
                        _ => return None
                    };
                    Some(bits)
                }

                pub static ENTRIES: [(#typ, Bits); #n] = [ #(#list,)* ];
            }
        }
    });
    let expanded = quote! {
        #(#maps)*
    };

    // Hand the output tokens back to the compiler
    proc_macro::TokenStream::from(expanded)
}

struct BitMaps {
    maps: Vec<BitMap>
}
impl Parse for BitMaps {
    fn parse(input: ParseStream) -> Result<Self> {
        let entries = Punctuated::<_, Token![,]>::parse_terminated(input)?;

        Ok(BitMaps { maps: entries.into_pairs().map(|p| p.into_value()).collect() })
    }
}

struct BitMap {
    name: Ident,
    entries: Vec<BitMapEntry>,
    typ: Option<Type>
}
impl Parse for BitMap {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let typ;
        if input.peek(Token![<]) {
            let _ = input.parse::<Token![<]>()?;
            typ = Some(input.parse::<Type>()?);
            let _ = input.parse::<Token![>]>()?;
        } else {
            typ = None;
        }
        
        let content;

        braced!(content in input);
        let entries = Punctuated::<_, Token![,]>::parse_terminated(&content)?;

        Ok(BitMap {
            name, typ,
            entries: entries.into_pairs().map(|p| p.into_value()).collect()
        })
    }
}
impl BitMap {
    fn build(&self) -> (TokenStream, TokenStream) {
        let patterns: Vec<(usize, Bits)> = self.entries.iter().enumerate().map(|(i, e)| (i, e.bits)).collect();
        let node = Node::build(&patterns).unwrap();
        
        let mut defs = vec![];
        let out = self.walk(&mut defs, &node, Bits::empty());

        (quote! {
            #(#defs)*
        }, out)
    }
    fn walk(&self, defs: &mut Vec<TokenStream>, node: &Node<usize>, bits: Bits) -> TokenStream {
        match *node {
            Node::Value(idx, len) => {
                let val = &self.entries[idx].value;
                quote!{ Entry::Value(#val, #len) }
            }
            Node::LeafLut(ref lut) => {
                let name = Ident::new(&format!("LEAF_LUT_{}", bits), Span::call_site());
                let size = lut.data.len();
                let width = lut.width;
                let entries = lut.data.iter().map(|e| match *e {
                    Some((idx, len)) => {
                        let val = &self.entries[idx].value;
                        quote! { Some((#val, #len)) }
                    },
                    None => quote!{ None }
                });
                let typ = match self.typ {
                    Some(ref t) => quote!{ #t },
                    None => quote! { u16 }
                };
                defs.push(quote!{
                    static #name: [Option<(#typ, u8)>; #size] = [
                        #(#entries,)*
                    ];
                });
                quote! { Entry::Leaf(#width, &#name) }
            }
            Node::PrefixLut(ref lut) => {
                let name = Ident::new(&format!("PREFIX_LUT_{}", bits), Span::call_site());
                let size = 1usize << lut.width;
                let width = lut.width;
                let entries: Vec<_> = lut.data.iter().enumerate().map(|(i, node)| match node {
                    None => quote!{ Entry::Empty }.into(),
                    Some(node) => {
                        let entry_bits = bits.concat(Bits::new(i as u16, lut.width));
                        self.walk(defs, node, entry_bits)
                    }
                }).collect();

                let typ = match self.typ {
                    Some(ref t) => quote!{ Entry<#t> },
                    None => quote! { Entry<u16> }
                };
                defs.push(quote!{
                    static #name: [#typ; #size] = [
                        #(#entries,)*
                    ];
                });

                quote! { Entry::Prefix(#width, &#name) }
            }
        }
    }
}
struct BitMapEntry {
    bits: Bits,
    value: Expr,
}
impl Parse for BitMapEntry {
    fn parse(input: ParseStream) -> Result<Self> {
        let bits = input.parse()?;
        let _ = input.parse::<Token![=>]>()?;
        let value = input.parse()?;
        Ok(BitMapEntry { bits, value })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Bits {
    data: u16,
    len: u8
}
impl Bits {
    fn new(data: u16, len: u8) -> Bits {
        Bits { data, len }
    }
    fn concat(self, rhs: Bits) -> Bits {
        Bits {
            data: self.data << rhs.len | rhs.data,
            len: self.len + rhs.len
        }
    }
    fn common_prefix_len(self, other: Bits) -> u8 {
        (self.align_left() ^ other.align_left()).leading_zeros() as u8
    }
    fn align_left(self) -> u16 {
        self.data << (16 - self.len)
    }
    fn prefix(self, len: u8) -> u16 {
        assert!(len <= self.len);
        self.data >> (self.len - len)
    }
    fn prefix_range(self, len: u8) -> Range<u16> {
        assert!(len >= self.len);
        let s = len - self.len;
        let n = 1 << (len - self.len);
        let m = self.data << s;
        m .. m + n
    }
    fn strip_prefix(self, len: u8) -> Bits {
        assert!(len <= self.len);
        let len =  self.len - len;
        Bits {
            data: self.data & ((1<<len)-1),
            len
        }
    }
    fn empty() -> Bits {
        Bits {
            data: 0,
            len: 0
        }
    }
}
impl fmt::Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:0w$b}", self.data, w=self.len as usize)
    }
}
impl fmt::Debug for Bits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "d={:0b} w={}", self.data, self.len)
    }
}
impl ToTokens for Bits {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Bits { data, len } = *self;
        tokens.extend(quote! {
            Bits { data: #data, len: #len }
        })
    }
}
impl Parse for Bits {
    fn parse(input: ParseStream) -> Result<Self> {
        let lit: Literal = input.parse()?;
        
        let mut data = 0;
        let mut len = 0;
        let s = lit.to_string();
        for c in s.chars() {
            let b = match c {
                '0' => 0,
                '1' => 1,
                _ => return Err(Error::new(lit.span(), "only 0 and 1 are allowed"))
            };
            data = data << 1 | b;
            len += 1;
        }
        Ok(Bits { data, len })
    }
}

struct LeafLut<T> {
    width: u8,
    data: Vec<Option<(T, u8)>>
}
impl<T: Clone> LeafLut<T> {
    fn build(patterns: &[(T, Bits)], prefix: u8, width: u8) -> Self {
        assert!(patterns.len() > 1);

        let mut data = vec![None; 1usize << width];
        for (val, pat) in patterns {
            let pat = pat.strip_prefix(prefix);
            
            for idx in pat.prefix_range(width) {
                data[idx as usize] = Some((val.clone(), pat.len));
            }
        }
    
        assert!(data.len() > 1);
        LeafLut {
            data,
            width
        }
    }
}
enum Node<T> {
    Value(T, u8),
    LeafLut(LeafLut<T>),
    PrefixLut(PrefixLut<T>)
}
impl<T: Copy + Default + Debug> Node<T> {
    fn size(&self) -> usize {
        match self {
            Node::Value(_, _) => 1,
            Node::LeafLut(ref lut) => 1 << lut.width,
            Node::PrefixLut(ref lut) => lut.size(),
        }
    }
    fn cost(&self) -> f64 {
        match self {
            Node::Value(_, _) => 0.0,
            Node::LeafLut(_) => 1.0,
            Node::PrefixLut(ref lut) => lut.cost(),
        }
    }
    fn build(patterns: &[(T, Bits)]) -> Option<Node<T>> {
        Self::build_prefix(patterns, 0)
    }
    fn build_prefix(patterns: &[(T, Bits)], prefix: u8) -> Option<Node<T>> {
        //debug!("{:?}", patterns);
        match patterns.len() {
            0 => None,
            1 => {
                let (val, bits) = patterns[0];
                Some(Node::Value(val, bits.len))
            }
            _ => {
                let width = patterns.iter().map(|(_, b)| b.len).max().unwrap() - prefix;
                if width > 8 {
                    Some(Node::PrefixLut(PrefixLut::build(&patterns, prefix)))
                } else {
                    Some(Node::LeafLut(LeafLut::build(patterns, prefix, width) ))
                }
            }
        }
    }
}

struct PrefixLut<T> {
    width: u8,
    data: Vec<Option<Node<T>>>
}
impl<T: Copy + Default + Debug> PrefixLut<T> {
    fn size(&self) -> usize {
        self.data.iter().filter_map(|o| o.as_ref().map(|n| n.size())).sum::<usize>() + self.data.len()
    }
    // cost per 1 bit
    fn cost(&self) -> f64 {
        self.data.iter().filter_map(|o| o.as_ref().map(|n| n.cost())).sum::<f64>() * 0.5f64.powi(-(self.width as i32))
    }
    fn build(patterns: &[(T, Bits)], prefix: u8) -> Self {
        // determine LUT size
        let max_width = patterns.iter().map(|(_, b)| b.len).max().unwrap() - prefix;

        let mut best = None;
        let mut best_cost = f64::INFINITY;
        for w in max_width.min(4) .. max_width {
            let lut = Self::build_width(patterns, w, prefix);
            let cost = lut.cost();
            if cost < best_cost {
                best_cost = cost;
                best = Some(lut);
            }
        }
        best.expect("empty results")
    }
    fn build_width(patterns: &[(T, Bits)], width: u8, prefix: u8) -> Self {
        let mut slots = vec![vec![]; 1 << width];
        //dbg!(patterns);
        for &(val, bits) in patterns {
            let bits = bits.strip_prefix(prefix);
            //debug!("{} - {}bits ({:?})", bits, width, val);

            if bits.len >= width {
                //debug!(" = {}",  bits.strip_prefix(width));
                slots[bits.prefix(width) as usize].push((val, bits));
            } else {
                for k in bits.prefix_range(width) {
                    //debug!("  -> {}", k);
                    slots[k as usize].push((val, bits));
                }
            }
        }

        let data: Vec<_> = slots.iter().map(|patterns| Node::build_prefix(&patterns, width)).collect();

        PrefixLut {
            data,
            width
        }
    }
}
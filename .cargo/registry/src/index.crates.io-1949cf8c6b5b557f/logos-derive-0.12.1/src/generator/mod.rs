use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::Ident;

use crate::graph::{Graph, Meta, Node, NodeId, Range};
use crate::leaf::Leaf;
use crate::util::ToIdent;

mod context;
mod fork;
mod leaf;
mod rope;
mod tables;

use self::context::Context;
use self::tables::TableStack;

pub struct Generator<'a> {
    /// Name of the type we are implementing the `Logos` trait for
    name: &'a Ident,
    /// Name of the type with any generics it might need
    this: &'a TokenStream,
    /// Id to the root node
    root: NodeId,
    /// Reference to the graph with all of the nodes
    graph: &'a Graph<Leaf<'a>>,
    /// Meta data collected for the nodes
    meta: Meta,
    /// Buffer with functions growing during generation
    rendered: TokenStream,
    /// Set of functions that have already been rendered
    fns: Set<(NodeId, Context)>,
    /// Function name identifiers
    idents: Map<(NodeId, Context), Ident>,
    /// Local function calls. Note: a call might change its context,
    /// so we can't use `idents` for this purpose.
    gotos: Map<(NodeId, Context), TokenStream>,
    /// Identifiers for helper functions matching a byte to a given
    /// set of ranges
    tests: Map<Vec<Range>, Ident>,
    /// Related to above, table stack manages tables that need to be
    tables: TableStack,
}

impl<'a> Generator<'a> {
    pub fn new(
        name: &'a Ident,
        this: &'a TokenStream,
        root: NodeId,
        graph: &'a Graph<Leaf>,
    ) -> Self {
        let rendered = Self::fast_loop_macro();
        let meta = Meta::analyze(root, graph);

        Generator {
            name,
            this,
            root,
            graph,
            meta,
            rendered,
            fns: Set::default(),
            idents: Map::default(),
            gotos: Map::default(),
            tests: Map::default(),
            tables: TableStack::new(),
        }
    }

    pub fn generate(mut self) -> TokenStream {
        let root = self.goto(self.root, Context::default()).clone();
        let rendered = &self.rendered;
        let tables = &self.tables;

        quote! {
            #tables
            #rendered
            #root
        }
    }

    fn generate_fn(&mut self, id: NodeId, ctx: Context) {
        if self.fns.contains(&(id, ctx)) {
            return;
        }
        self.fns.insert((id, ctx));

        let body = match &self.graph[id] {
            Node::Fork(fork) => self.generate_fork(id, fork, ctx),
            Node::Rope(rope) => self.generate_rope(rope, ctx),
            Node::Leaf(leaf) => self.generate_leaf(leaf, ctx),
        };
        let ident = self.generate_ident(id, ctx);
        let out = quote! {
            #[inline]
            fn #ident<'s>(lex: &mut Lexer<'s>) {
                #body
            }
        };

        self.rendered.append_all(out);
    }

    fn goto(&mut self, id: NodeId, mut ctx: Context) -> &TokenStream {
        let key = (id, ctx);

        if !self.gotos.contains_key(&key) {
            let meta = &self.meta[id];
            let enters_loop = !meta.loop_entry_from.is_empty();

            let bump = if enters_loop || !ctx.can_backtrack() {
                ctx.switch(self.graph[id].miss())
            } else {
                None
            };

            let bump = match (bump, enters_loop, meta.min_read) {
                (Some(t), _, _) => Some(t),
                (None, true, _) => ctx.bump(),
                (None, false, 0) => ctx.bump(),
                (None, false, _) => None,
            };

            if meta.min_read == 0 || ctx.remainder() < meta.min_read {
                ctx.wipe();
            }

            let ident = self.generate_ident(id, ctx);
            let mut call_site = quote!(#ident(lex));

            if let Some(bump) = bump {
                call_site = quote!({
                    #bump
                    #call_site
                });
            }
            self.gotos.insert(key, call_site);
            self.generate_fn(id, ctx);
        }
        &self.gotos[&key]
    }

    fn generate_ident(&mut self, id: NodeId, ctx: Context) -> &Ident {
        self.idents.entry((id, ctx)).or_insert_with(|| {
            let mut ident = format!("goto{}", id);

            ctx.write_suffix(&mut ident);

            ident.to_ident()
        })
    }

    /// Returns an identifier to a function that matches a byte to any
    /// of the provided ranges. This will generate either a simple
    /// match expression, or use a lookup table internally.
    fn generate_test(&mut self, ranges: Vec<Range>) -> &Ident {
        if !self.tests.contains_key(&ranges) {
            let idx = self.tests.len();
            let ident = format!("pattern{}", idx).to_ident();

            let lo = ranges.first().unwrap().start;
            let hi = ranges.last().unwrap().end;

            let body = match ranges.len() {
                0..=2 => {
                    quote! {
                        match byte {
                            #(#ranges)|* => true,
                            _ => false,
                        }
                    }
                }
                _ if hi - lo < 64 => {
                    let mut offset = hi.saturating_sub(63);

                    while offset.count_ones() > 1 && lo - offset > 0 {
                        offset += 1;
                    }

                    let mut table = 0u64;

                    for byte in ranges.iter().flat_map(|range| *range) {
                        if byte - offset >= 64 {
                            panic!("{:#?} {} {} {}", ranges, hi, lo, offset);
                        }
                        table |= 1 << (byte - offset);
                    }

                    let search = match offset {
                        0 => quote!(byte),
                        _ => quote!(byte.wrapping_sub(#offset)),
                    };

                    quote! {
                        const LUT: u64 = #table;

                        match 1u64.checked_shl(#search as u32) {
                            Some(shift) => LUT & shift != 0,
                            None => false,
                        }
                    }
                }
                _ => {
                    let mut view = self.tables.view();

                    for byte in ranges.iter().flat_map(|range| *range) {
                        view.flag(byte);
                    }

                    let mask = view.mask();
                    let lut = view.ident();

                    quote! {
                        #lut[byte as usize] & #mask > 0
                    }
                }
            };
            self.rendered.append_all(quote! {
                #[inline]
                fn #ident(byte: u8) -> bool {
                    #body
                }
            });
            self.tests.insert(ranges.clone(), ident);
        }
        &self.tests[&ranges]
    }
}

macro_rules! match_quote {
    ($source:expr; $($byte:tt,)* ) => {match $source {
        $( $byte => quote!($byte), )*
        byte => quote!(#byte),
    }}
}

fn byte_to_tokens(byte: u8) -> TokenStream {
    match_quote! {
        byte;
        b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9',
        b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
        b'u', b'v', b'w', b'x', b'y', b'z',
        b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J',
        b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T',
        b'U', b'V', b'W', b'X', b'Y', b'Z',
        b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')',
        b'{', b'}', b'[', b']', b'<', b'>', b'-', b'=', b'_', b'+',
        b':', b';', b',', b'.', b'/', b'?', b'|', b'"', b'\'', b'\\',
    }
}

impl ToTokens for Range {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Range { start, end } = self;

        tokens.append_all(byte_to_tokens(*start));

        if start != end {
            tokens.append_all(quote!(..=));
            tokens.append_all(byte_to_tokens(*end));
        }
    }
}

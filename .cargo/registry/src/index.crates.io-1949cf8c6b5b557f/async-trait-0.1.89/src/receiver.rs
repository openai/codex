use proc_macro2::{Group, TokenStream, TokenTree};
use syn::visit_mut::{self, VisitMut};
use syn::{
    Block, ExprPath, Ident, Item, Macro, Pat, PatIdent, Path, Receiver, Signature, Token, TypePath,
};

pub fn has_self_in_sig(sig: &mut Signature) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_signature_mut(sig);
    visitor.0
}

pub fn has_self_in_block(block: &mut Block) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_block_mut(block);
    visitor.0
}

fn has_self_in_token_stream(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "Self",
        TokenTree::Group(group) => has_self_in_token_stream(group.stream()),
        _ => false,
    })
}

pub fn mut_pat(pat: &mut Pat) -> Option<Token![mut]> {
    let mut visitor = HasMutPat(None);
    visitor.visit_pat_mut(pat);
    visitor.0
}

fn contains_fn(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "fn",
        TokenTree::Group(group) => contains_fn(group.stream()),
        _ => false,
    })
}

struct HasMutPat(Option<Token![mut]>);

impl VisitMut for HasMutPat {
    fn visit_pat_ident_mut(&mut self, i: &mut PatIdent) {
        if let Some(m) = &i.mutability {
            self.0 = Some(Token![mut](m.span));
        } else {
            visit_mut::visit_pat_ident_mut(self, i);
        }
    }
}

struct HasSelf(bool);

impl VisitMut for HasSelf {
    fn visit_expr_path_mut(&mut self, expr: &mut ExprPath) {
        self.0 |= expr.path.segments[0].ident == "Self";
        visit_mut::visit_expr_path_mut(self, expr);
    }

    fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
        self.0 |= ty.path.segments[0].ident == "Self";
        visit_mut::visit_type_path_mut(self, ty);
    }

    fn visit_receiver_mut(&mut self, _arg: &mut Receiver) {
        self.0 = true;
    }

    fn visit_item_mut(&mut self, _: &mut Item) {
        // Do not recurse into nested items.
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        if !contains_fn(mac.tokens.clone()) {
            self.0 |= has_self_in_token_stream(mac.tokens.clone());
        }
    }
}

pub struct ReplaceSelf;

fn prepend_underscore_to_self(ident: &mut Ident) -> bool {
    let modified = ident == "self";
    if modified {
        *ident = Ident::new("__self", ident.span());
    }
    modified
}

impl ReplaceSelf {
    fn visit_token_stream(&mut self, tokens: &mut TokenStream) -> bool {
        let mut out = Vec::new();
        let mut modified = false;
        visit_token_stream_impl(self, tokens.clone(), &mut modified, &mut out);
        if modified {
            *tokens = TokenStream::from_iter(out);
        }
        return modified;

        fn visit_token_stream_impl(
            visitor: &mut ReplaceSelf,
            tokens: TokenStream,
            modified: &mut bool,
            out: &mut Vec<TokenTree>,
        ) {
            for tt in tokens {
                match tt {
                    TokenTree::Ident(mut ident) => {
                        *modified |= prepend_underscore_to_self(&mut ident);
                        out.push(TokenTree::Ident(ident));
                    }
                    TokenTree::Group(group) => {
                        let mut content = group.stream();
                        *modified |= visitor.visit_token_stream(&mut content);
                        let mut new = Group::new(group.delimiter(), content);
                        new.set_span(group.span());
                        out.push(TokenTree::Group(new));
                    }
                    other => out.push(other),
                }
            }
        }
    }
}

impl VisitMut for ReplaceSelf {
    fn visit_ident_mut(&mut self, i: &mut Ident) {
        prepend_underscore_to_self(i);
    }

    fn visit_path_mut(&mut self, p: &mut Path) {
        if p.segments.len() == 1 {
            // Replace `self`, but not `self::function`.
            self.visit_ident_mut(&mut p.segments[0].ident);
        }
        for segment in &mut p.segments {
            self.visit_path_arguments_mut(&mut segment.arguments);
        }
    }

    fn visit_item_mut(&mut self, i: &mut Item) {
        // Visit `macro_rules!` because locally defined macros can refer to
        // `self`.
        //
        // Visit `futures::select` and similar select macros, which commonly
        // appear syntactically like an item despite expanding to an expression.
        //
        // Otherwise, do not recurse into nested items.
        if let Item::Macro(i) = i {
            if i.mac.path.is_ident("macro_rules")
                || i.mac.path.segments.last().unwrap().ident == "select"
            {
                self.visit_macro_mut(&mut i.mac);
            }
        }
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        // We can't tell in general whether `self` inside a macro invocation
        // refers to the self in the argument list or a different self
        // introduced within the macro. Heuristic: if the macro input contains
        // `fn`, then `self` is more likely to refer to something other than the
        // outer function's self argument.
        if !contains_fn(mac.tokens.clone()) {
            self.visit_token_stream(&mut mac.tokens);
        }
    }
}

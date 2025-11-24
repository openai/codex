use proc_macro::{token_stream, Delimiter, TokenStream, TokenTree};

pub type Iter<'a> = &'a mut IterImpl;

pub struct IterImpl {
    stack: Vec<token_stream::IntoIter>,
    peeked: Option<TokenTree>,
}

pub fn new(tokens: TokenStream) -> IterImpl {
    IterImpl {
        stack: vec![tokens.into_iter()],
        peeked: None,
    }
}

impl IterImpl {
    pub fn peek(&mut self) -> Option<&TokenTree> {
        self.peeked = self.next();
        self.peeked.as_ref()
    }
}

impl Iterator for IterImpl {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tt) = self.peeked.take() {
            return Some(tt);
        }
        loop {
            let top = self.stack.last_mut()?;
            match top.next() {
                None => drop(self.stack.pop()),
                Some(TokenTree::Group(ref group)) if group.delimiter() == Delimiter::None => {
                    self.stack.push(group.stream().into_iter());
                }
                Some(tt) => return Some(tt),
            }
        }
    }
}

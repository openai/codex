use syn::ext::IdentExt;
use syn::parse::ParseStream;
use syn::{Ident, Token};

#[test]
fn test_peek() {
    _ = |input: ParseStream| {
        _ = input.peek(Ident);
        _ = input.peek(Ident::peek_any);
        _ = input.peek(Token![::]);
    };
}

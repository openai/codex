use proc_macro::TokenStream;
use proc_macro2::{Group, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser, Result};
use syn::visit_mut::VisitMut;

struct Scrub<'a> {
    /// Whether the stream is a try stream.
    is_try: bool,
    /// The unit expression, `()`.
    unit: Box<syn::Expr>,
    has_yielded: bool,
    crate_path: &'a TokenStream2,
}

fn parse_input(input: TokenStream) -> syn::Result<(TokenStream2, Vec<syn::Stmt>)> {
    let mut input = TokenStream2::from(input).into_iter();
    let crate_path = match input.next().unwrap() {
        TokenTree::Group(group) => group.stream(),
        _ => panic!(),
    };
    let stmts = syn::Block::parse_within.parse2(replace_for_await(input))?;
    Ok((crate_path, stmts))
}

impl<'a> Scrub<'a> {
    fn new(is_try: bool, crate_path: &'a TokenStream2) -> Self {
        Self {
            is_try,
            unit: syn::parse_quote!(()),
            has_yielded: false,
            crate_path,
        }
    }
}

struct Partial<T>(T, TokenStream2);

impl<T: Parse> Parse for Partial<T> {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Partial(input.parse()?, input.parse()?))
    }
}

fn visit_token_stream_impl(
    visitor: &mut Scrub<'_>,
    tokens: TokenStream2,
    modified: &mut bool,
    out: &mut TokenStream2,
) {
    use quote::ToTokens;
    use quote::TokenStreamExt;

    let mut tokens = tokens.into_iter().peekable();
    while let Some(tt) = tokens.next() {
        match tt {
            TokenTree::Ident(i) if i == "yield" => {
                let stream = std::iter::once(TokenTree::Ident(i)).chain(tokens).collect();
                match syn::parse2(stream) {
                    Ok(Partial(yield_expr, rest)) => {
                        let mut expr = syn::Expr::Yield(yield_expr);
                        visitor.visit_expr_mut(&mut expr);
                        expr.to_tokens(out);
                        *modified = true;
                        tokens = rest.into_iter().peekable();
                    }
                    Err(e) => {
                        out.append_all(e.to_compile_error().into_iter());
                        *modified = true;
                        return;
                    }
                }
            }
            TokenTree::Ident(i) if i == "stream" || i == "try_stream" => {
                out.append(TokenTree::Ident(i));
                match tokens.peek() {
                    Some(TokenTree::Punct(p)) if p.as_char() == '!' => {
                        out.extend(tokens.next()); // !
                        if let Some(TokenTree::Group(_)) = tokens.peek() {
                            out.extend(tokens.next()); // { .. } or [ .. ] or ( .. )
                        }
                    }
                    _ => {}
                }
            }
            TokenTree::Group(group) => {
                let mut content = group.stream();
                *modified |= visitor.visit_token_stream(&mut content);
                let mut new = Group::new(group.delimiter(), content);
                new.set_span(group.span());
                out.append(new);
            }
            other => out.append(other),
        }
    }
}

impl Scrub<'_> {
    fn visit_token_stream(&mut self, tokens: &mut TokenStream2) -> bool {
        let (mut out, mut modified) = (TokenStream2::new(), false);
        visit_token_stream_impl(self, tokens.clone(), &mut modified, &mut out);

        if modified {
            *tokens = out;
        }

        modified
    }
}

impl VisitMut for Scrub<'_> {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        match i {
            syn::Expr::Yield(yield_expr) => {
                self.has_yielded = true;

                syn::visit_mut::visit_expr_yield_mut(self, yield_expr);

                let value_expr = yield_expr.expr.as_ref().unwrap_or(&self.unit);

                // let ident = &self.yielder;

                let yield_expr = if self.is_try {
                    quote! { __yield_tx.send(::core::result::Result::Ok(#value_expr)).await }
                } else {
                    quote! { __yield_tx.send(#value_expr).await }
                };
                *i = syn::parse_quote! {
                    {
                        #[allow(unreachable_code)]
                        if false {
                            break '__async_stream_private_check_scope (loop {});
                        }
                        #yield_expr
                    }
                };
            }
            syn::Expr::Try(try_expr) => {
                syn::visit_mut::visit_expr_try_mut(self, try_expr);
                // let ident = &self.yielder;
                let e = &try_expr.expr;

                *i = syn::parse_quote! {
                    match #e {
                        ::core::result::Result::Ok(v) => v,
                        ::core::result::Result::Err(e) => {
                            __yield_tx.send(::core::result::Result::Err(e.into())).await;
                            return;
                        }
                    }
                };
            }
            syn::Expr::Closure(_) | syn::Expr::Async(_) => {
                // Don't transform inner closures or async blocks.
            }
            syn::Expr::ForLoop(expr) => {
                syn::visit_mut::visit_expr_for_loop_mut(self, expr);
                // TODO: Should we allow other attributes?
                if expr.attrs.len() != 1 || !expr.attrs[0].meta.path().is_ident(AWAIT_ATTR_NAME) {
                    return;
                }
                let syn::ExprForLoop {
                    attrs,
                    label,
                    pat,
                    expr,
                    body,
                    ..
                } = expr;

                attrs.pop().unwrap();

                let crate_path = self.crate_path;
                *i = syn::parse_quote! {{
                    let mut __pinned = #expr;
                    let mut __pinned = unsafe {
                        ::core::pin::Pin::new_unchecked(&mut __pinned)
                    };
                    #label
                    loop {
                        let #pat = match #crate_path::__private::next(&mut __pinned).await {
                            ::core::option::Option::Some(e) => e,
                            ::core::option::Option::None => break,
                        };
                        #body
                    }
                }}
            }
            _ => syn::visit_mut::visit_expr_mut(self, i),
        }
    }

    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let mac_ident = mac.path.segments.last().map(|p| &p.ident);
        if mac_ident.map_or(false, |i| i == "stream" || i == "try_stream") {
            return;
        }

        self.visit_token_stream(&mut mac.tokens);
    }

    fn visit_item_mut(&mut self, i: &mut syn::Item) {
        // Recurse into macros but otherwise don't transform inner items.
        if let syn::Item::Macro(i) = i {
            self.visit_macro_mut(&mut i.mac);
        }
    }
}

/// The first token tree in the stream must be a group containing the path to the `async-stream`
/// crate.
#[proc_macro]
#[doc(hidden)]
pub fn stream_inner(input: TokenStream) -> TokenStream {
    let (crate_path, mut stmts) = match parse_input(input) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut scrub = Scrub::new(false, &crate_path);

    for stmt in &mut stmts {
        scrub.visit_stmt_mut(stmt);
    }

    let dummy_yield = if scrub.has_yielded {
        None
    } else {
        Some(quote!(if false {
            __yield_tx.send(()).await;
        }))
    };

    quote!({
        let (mut __yield_tx, __yield_rx) = unsafe { #crate_path::__private::yielder::pair() };
        #crate_path::__private::AsyncStream::new(__yield_rx, async move {
            '__async_stream_private_check_scope: {
                #dummy_yield
                #(#stmts)*
            }
        })
    })
    .into()
}

/// The first token tree in the stream must be a group containing the path to the `async-stream`
/// crate.
#[proc_macro]
#[doc(hidden)]
pub fn try_stream_inner(input: TokenStream) -> TokenStream {
    let (crate_path, mut stmts) = match parse_input(input) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut scrub = Scrub::new(true, &crate_path);

    for stmt in &mut stmts {
        scrub.visit_stmt_mut(stmt);
    }

    let dummy_yield = if scrub.has_yielded {
        None
    } else {
        Some(quote!(if false {
            __yield_tx.send(()).await;
        }))
    };

    quote!({
        let (mut __yield_tx, __yield_rx) = unsafe { #crate_path::__private::yielder::pair() };
        #crate_path::__private::AsyncStream::new(__yield_rx, async move {
            '__async_stream_private_check_scope: {
                #dummy_yield
                #(#stmts)*
            }
        })
    })
    .into()
}

// syn 2.0 wont parse `#[await] for x in xs {}`
// because `await` is a keyword, use `await_` instead
const AWAIT_ATTR_NAME: &str = "await_";

/// Replace `for await` with `#[await] for`, which will be later transformed into a `next` loop.
fn replace_for_await(input: impl IntoIterator<Item = TokenTree>) -> TokenStream2 {
    let mut input = input.into_iter().peekable();
    let mut tokens = Vec::new();

    while let Some(token) = input.next() {
        match token {
            TokenTree::Ident(ident) => {
                match input.peek() {
                    Some(TokenTree::Ident(next)) if ident == "for" && next == "await" => {
                        let next_span = next.span();
                        let next = syn::Ident::new(AWAIT_ATTR_NAME, next_span);
                        tokens.extend(quote!(#[#next]));
                        let _ = input.next();
                    }
                    _ => {}
                }
                tokens.push(ident.into());
            }
            TokenTree::Group(group) => {
                let stream = replace_for_await(group.stream());
                let mut new_group = Group::new(group.delimiter(), stream);
                new_group.set_span(group.span());
                tokens.push(new_group.into());
            }
            _ => tokens.push(token),
        }
    }

    tokens.into_iter().collect()
}

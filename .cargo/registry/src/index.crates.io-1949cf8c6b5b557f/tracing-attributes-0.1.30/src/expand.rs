use std::iter;

use proc_macro2::TokenStream;
use quote::TokenStreamExt;
use quote::{quote, quote_spanned, ToTokens};
use syn::visit_mut::VisitMut;
use syn::{
    punctuated::Punctuated, spanned::Spanned, Block, Expr, ExprAsync, ExprCall, FieldPat, FnArg,
    Ident, Item, ItemFn, Pat, PatIdent, PatReference, PatStruct, PatTuple, PatTupleStruct, PatType,
    Path, ReturnType, Signature, Stmt, Token, Type, TypePath,
};

use crate::{
    attr::{Field, Fields, FormatMode, InstrumentArgs, Level},
    MaybeItemFn, MaybeItemFnRef,
};

/// Given an existing function, generate an instrumented version of that function
pub(crate) fn gen_function<'a, B: ToTokens + 'a>(
    input: MaybeItemFnRef<'a, B>,
    args: InstrumentArgs,
    instrumented_function_name: &str,
    self_type: Option<&TypePath>,
) -> proc_macro2::TokenStream {
    // these are needed ahead of time, as ItemFn contains the function body _and_
    // isn't representable inside a quote!/quote_spanned! macro
    // (Syn's ToTokens isn't implemented for ItemFn)
    let MaybeItemFnRef {
        outer_attrs,
        inner_attrs,
        vis,
        sig,
        brace_token,
        block,
    } = input;

    let Signature {
        output,
        inputs: params,
        unsafety,
        asyncness,
        constness,
        abi,
        ident,
        generics:
            syn::Generics {
                params: gen_params,
                where_clause,
                lt_token,
                gt_token,
            },
        fn_token,
        paren_token,
        variadic,
    } = sig;

    let warnings = args.warnings();

    let (return_type, return_span) = if let ReturnType::Type(_, return_type) = &output {
        (erase_impl_trait(return_type), return_type.span())
    } else {
        // Point at function name if we don't have an explicit return type
        (syn::parse_quote! { () }, ident.span())
    };
    // Install a fake return statement as the first thing in the function
    // body, so that we eagerly infer that the return type is what we
    // declared in the async fn signature.
    // The `#[allow(..)]` is given because the return statement is
    // unreachable, but does affect inference, so it needs to be written
    // exactly that way for it to do its magic.
    let fake_return_edge = quote_spanned! {return_span=>
        #[allow(
            unknown_lints,
            unreachable_code,
            clippy::diverging_sub_expression,
            clippy::empty_loop,
            clippy::let_unit_value,
            clippy::let_with_type_underscore,
            clippy::needless_return,
            clippy::unreachable
        )]
        if false {
            let __tracing_attr_fake_return: #return_type = loop {};
            return __tracing_attr_fake_return;
        }
    };
    let block = quote! {
        {
            #fake_return_edge
            { #block }
        }
    };

    let body = gen_block(
        &block,
        params,
        asyncness.is_some(),
        args,
        instrumented_function_name,
        self_type,
    );

    let mut result = quote!(
        #(#outer_attrs) *
        #vis #constness #asyncness #unsafety #abi #fn_token #ident
        #lt_token #gen_params #gt_token
    );

    paren_token.surround(&mut result, |tokens| {
        params.to_tokens(tokens);
        variadic.to_tokens(tokens);
    });

    output.to_tokens(&mut result);
    where_clause.to_tokens(&mut result);

    brace_token.surround(&mut result, |tokens| {
        tokens.append_all(inner_attrs);
        warnings.to_tokens(tokens);
        body.to_tokens(tokens);
    });

    result
}

/// Instrument a block
fn gen_block<B: ToTokens>(
    block: &B,
    params: &Punctuated<FnArg, Token![,]>,
    async_context: bool,
    mut args: InstrumentArgs,
    instrumented_function_name: &str,
    self_type: Option<&TypePath>,
) -> proc_macro2::TokenStream {
    // generate the span's name
    let span_name = args
        // did the user override the span's name?
        .name
        .as_ref()
        .map(|name| quote!(#name))
        .unwrap_or_else(|| quote!(#instrumented_function_name));

    let args_level = args.level();
    let level = args_level.clone();

    let follows_from = args.follows_from.iter();
    let follows_from = quote! {
        #(for cause in #follows_from {
            __tracing_attr_span.follows_from(cause);
        })*
    };

    // generate this inside a closure, so we can return early on errors.
    let span = (|| {
        // Pull out the arguments-to-be-skipped first, so we can filter results
        // below.
        let param_names: Vec<(Ident, (Ident, RecordType))> = params
            .clone()
            .into_iter()
            .flat_map(|param| match param {
                FnArg::Typed(PatType { pat, ty, .. }) => {
                    param_names(*pat, RecordType::parse_from_ty(&ty))
                }
                FnArg::Receiver(_) => Box::new(iter::once((
                    Ident::new("self", param.span()),
                    RecordType::Debug,
                ))),
            })
            // Little dance with new (user-exposed) names and old (internal)
            // names of identifiers. That way, we could do the following
            // even though async_trait (<=0.1.43) rewrites "self" as "_self":
            // ```
            // #[async_trait]
            // impl Foo for FooImpl {
            //     #[instrument(skip(self))]
            //     async fn foo(&self, v: usize) {}
            // }
            // ```
            .map(|(x, record_type)| {
                // if we are inside a function generated by async-trait <=0.1.43, we need to
                // take care to rewrite "_self" as "self" for 'user convenience'
                if self_type.is_some() && x == "_self" {
                    (Ident::new("self", x.span()), (x, record_type))
                } else {
                    (x.clone(), (x, record_type))
                }
            })
            .collect();

        for skip in &args.skips {
            if !param_names.iter().map(|(user, _)| user).any(|y| y == skip) {
                return quote_spanned! {skip.span()=>
                    compile_error!("attempting to skip non-existent parameter")
                };
            }
        }

        let target = args.target();

        let parent = args.parent.iter();

        // filter out skipped fields
        let quoted_fields: Vec<_> = param_names
            .iter()
            .filter(|(param, _)| {
                if args.skip_all || args.skips.contains(param) {
                    return false;
                }

                // If any parameters have the same name as a custom field, skip
                // and allow them to be formatted by the custom field.
                if let Some(ref fields) = args.fields {
                    fields.0.iter().all(|Field { ref name, .. }| {
                        let first = name.first();
                        first != name.last() || !first.iter().any(|name| name == &param)
                    })
                } else {
                    true
                }
            })
            .map(|(user_name, (real_name, record_type))| match record_type {
                RecordType::Value => quote!(#user_name = #real_name),
                RecordType::Debug => quote!(#user_name = ::tracing::field::debug(&#real_name)),
            })
            .collect();

        // replace every use of a variable with its original name
        if let Some(Fields(ref mut fields)) = args.fields {
            let mut replacer = IdentAndTypesRenamer {
                idents: param_names.into_iter().map(|(a, (b, _))| (a, b)).collect(),
                types: Vec::new(),
            };

            // when async-trait <=0.1.43 is in use, replace instances
            // of the "Self" type inside the fields values
            if let Some(self_type) = self_type {
                replacer.types.push(("Self", self_type.clone()));
            }

            for e in fields.iter_mut().filter_map(|f| f.value.as_mut()) {
                syn::visit_mut::visit_expr_mut(&mut replacer, e);
            }
        }

        let custom_fields = &args.fields;

        quote!(::tracing::span!(
            target: #target,
            #(parent: #parent,)*
            #level,
            #span_name,
            #(#quoted_fields,)*
            #custom_fields

        ))
    })();

    let target = args.target();

    let err_event = match args.err_args {
        Some(event_args) => {
            let level_tokens = event_args.level(Level::Error);
            match event_args.mode {
                FormatMode::Default | FormatMode::Display => Some(quote!(
                    ::tracing::event!(target: #target, #level_tokens, error = %e)
                )),
                FormatMode::Debug => Some(quote!(
                    ::tracing::event!(target: #target, #level_tokens, error = ?e)
                )),
            }
        }
        _ => None,
    };

    let ret_event = match args.ret_args {
        Some(event_args) => {
            let level_tokens = event_args.level(args_level);
            match event_args.mode {
                FormatMode::Display => Some(quote!(
                    ::tracing::event!(target: #target, #level_tokens, return = %x)
                )),
                FormatMode::Default | FormatMode::Debug => Some(quote!(
                    ::tracing::event!(target: #target, #level_tokens, return = ?x)
                )),
            }
        }
        _ => None,
    };

    // Generate the instrumented function body.
    // If the function is an `async fn`, this will wrap it in an async block,
    // which is `instrument`ed using `tracing-futures`. Otherwise, this will
    // enter the span and then perform the rest of the body.
    // If `err` is in args, instrument any resulting `Err`s.
    // If `ret` is in args, instrument any resulting `Ok`s when the function
    // returns `Result`s, otherwise instrument any resulting values.
    if async_context {
        let mk_fut = match (err_event, ret_event) {
            (Some(err_event), Some(ret_event)) => quote_spanned!(block.span()=>
                async move {
                    let __match_scrutinee = async move #block.await;
                    match  __match_scrutinee {
                        #[allow(clippy::unit_arg)]
                        Ok(x) => {
                            #ret_event;
                            Ok(x)
                        },
                        Err(e) => {
                            #err_event;
                            Err(e)
                        }
                    }
                }
            ),
            (Some(err_event), None) => quote_spanned!(block.span()=>
                async move {
                    match async move #block.await {
                        #[allow(clippy::unit_arg)]
                        Ok(x) => Ok(x),
                        Err(e) => {
                            #err_event;
                            Err(e)
                        }
                    }
                }
            ),
            (None, Some(ret_event)) => quote_spanned!(block.span()=>
                async move {
                    let x = async move #block.await;
                    #ret_event;
                    x
                }
            ),
            (None, None) => quote_spanned!(block.span()=>
                async move #block
            ),
        };

        return quote!(
            let __tracing_attr_span = #span;
            let __tracing_instrument_future = #mk_fut;
            if !__tracing_attr_span.is_disabled() {
                #follows_from
                ::tracing::Instrument::instrument(
                    __tracing_instrument_future,
                    __tracing_attr_span
                )
                .await
            } else {
                __tracing_instrument_future.await
            }
        );
    }

    let span = quote!(
        // These variables are left uninitialized and initialized only
        // if the tracing level is statically enabled at this point.
        // While the tracing level is also checked at span creation
        // time, that will still create a dummy span, and a dummy guard
        // and drop the dummy guard later. By lazily initializing these
        // variables, Rust will generate a drop flag for them and thus
        // only drop the guard if it was created. This creates code that
        // is very straightforward for LLVM to optimize out if the tracing
        // level is statically disabled, while not causing any performance
        // regression in case the level is enabled.
        let __tracing_attr_span;
        let __tracing_attr_guard;
        if ::tracing::level_enabled!(#level) || ::tracing::if_log_enabled!(#level, {true} else {false}) {
            __tracing_attr_span = #span;
            #follows_from
            __tracing_attr_guard = __tracing_attr_span.enter();
        }
    );

    match (err_event, ret_event) {
        (Some(err_event), Some(ret_event)) => quote_spanned! {block.span()=>
            #span
            #[allow(clippy::redundant_closure_call)]
            match (move || #block)() {
                #[allow(clippy::unit_arg)]
                Ok(x) => {
                    #ret_event;
                    Ok(x)
                },
                Err(e) => {
                    #err_event;
                    Err(e)
                }
            }
        },
        (Some(err_event), None) => quote_spanned!(block.span()=>
            #span
            #[allow(clippy::redundant_closure_call)]
            match (move || #block)() {
                #[allow(clippy::unit_arg)]
                Ok(x) => Ok(x),
                Err(e) => {
                    #err_event;
                    Err(e)
                }
            }
        ),
        (None, Some(ret_event)) => quote_spanned!(block.span()=>
            #span
            #[allow(clippy::redundant_closure_call)]
            let x = (move || #block)();
            #ret_event;
            x
        ),
        (None, None) => quote_spanned!(block.span() =>
            // Because `quote` produces a stream of tokens _without_ whitespace, the
            // `if` and the block will appear directly next to each other. This
            // generates a clippy lint about suspicious `if/else` formatting.
            // Therefore, suppress the lint inside the generated code...
            #[allow(clippy::suspicious_else_formatting)]
            {
                #span
                // ...but turn the lint back on inside the function body.
                #[warn(clippy::suspicious_else_formatting)]
                #block
            }
        ),
    }
}

/// Indicates whether a field should be recorded as `Value` or `Debug`.
enum RecordType {
    /// The field should be recorded using its `Value` implementation.
    Value,
    /// The field should be recorded using `tracing::field::debug()`.
    Debug,
}

impl RecordType {
    /// Array of primitive types which should be recorded as [RecordType::Value].
    const TYPES_FOR_VALUE: &'static [&'static str] = &[
        "bool",
        "str",
        "u8",
        "i8",
        "u16",
        "i16",
        "u32",
        "i32",
        "u64",
        "i64",
        "u128",
        "i128",
        "f32",
        "f64",
        "usize",
        "isize",
        "String",
        "NonZeroU8",
        "NonZeroI8",
        "NonZeroU16",
        "NonZeroI16",
        "NonZeroU32",
        "NonZeroI32",
        "NonZeroU64",
        "NonZeroI64",
        "NonZeroU128",
        "NonZeroI128",
        "NonZeroUsize",
        "NonZeroIsize",
        "Wrapping",
    ];

    /// Parse `RecordType` from [Type] by looking up
    /// the [RecordType::TYPES_FOR_VALUE] array.
    fn parse_from_ty(ty: &Type) -> Self {
        match ty {
            Type::Path(TypePath { path, .. })
                if path
                    .segments
                    .iter()
                    .next_back()
                    .map(|path_segment| {
                        let ident = path_segment.ident.to_string();
                        Self::TYPES_FOR_VALUE.iter().any(|&t| t == ident)
                    })
                    .unwrap_or(false) =>
            {
                RecordType::Value
            }
            Type::Reference(syn::TypeReference { elem, .. }) => RecordType::parse_from_ty(elem),
            _ => RecordType::Debug,
        }
    }
}

fn param_names(pat: Pat, record_type: RecordType) -> Box<dyn Iterator<Item = (Ident, RecordType)>> {
    match pat {
        Pat::Ident(PatIdent { ident, .. }) => Box::new(iter::once((ident, record_type))),
        Pat::Reference(PatReference { pat, .. }) => param_names(*pat, record_type),
        // We can't get the concrete type of fields in the struct/tuple
        // patterns by using `syn`. e.g. `fn foo(Foo { x, y }: Foo) {}`.
        // Therefore, the struct/tuple patterns in the arguments will just
        // always be recorded as `RecordType::Debug`.
        Pat::Struct(PatStruct { fields, .. }) => Box::new(
            fields
                .into_iter()
                .flat_map(|FieldPat { pat, .. }| param_names(*pat, RecordType::Debug)),
        ),
        Pat::Tuple(PatTuple { elems, .. }) => Box::new(
            elems
                .into_iter()
                .flat_map(|p| param_names(p, RecordType::Debug)),
        ),
        Pat::TupleStruct(PatTupleStruct { elems, .. }) => Box::new(
            elems
                .into_iter()
                .flat_map(|p| param_names(p, RecordType::Debug)),
        ),

        // The above *should* cover all cases of irrefutable patterns,
        // but we purposefully don't do any funny business here
        // (such as panicking) because that would obscure rustc's
        // much more informative error message.
        _ => Box::new(iter::empty()),
    }
}

/// The specific async code pattern that was detected
enum AsyncKind<'a> {
    /// Immediately-invoked async fn, as generated by `async-trait <= 0.1.43`:
    /// `async fn foo<...>(...) {...}; Box::pin(foo<...>(...))`
    Function(&'a ItemFn),
    /// A function returning an async (move) block, optionally `Box::pin`-ed,
    /// as generated by `async-trait >= 0.1.44`:
    /// `Box::pin(async move { ... })`
    Async {
        async_expr: &'a ExprAsync,
        pinned_box: bool,
    },
}

pub(crate) struct AsyncInfo<'block> {
    // statement that must be patched
    source_stmt: &'block Stmt,
    kind: AsyncKind<'block>,
    self_type: Option<TypePath>,
    input: &'block ItemFn,
}

impl<'block> AsyncInfo<'block> {
    /// Get the AST of the inner function we need to hook, if it looks like a
    /// manual future implementation.
    ///
    /// When we are given a function that returns a (pinned) future containing the
    /// user logic, it is that (pinned) future that needs to be instrumented.
    /// Were we to instrument its parent, we would only collect information
    /// regarding the allocation of that future, and not its own span of execution.
    ///
    /// We inspect the block of the function to find if it matches any of the
    /// following patterns:
    ///
    /// - Immediately-invoked async fn, as generated by `async-trait <= 0.1.43`:
    ///   `async fn foo<...>(...) {...}; Box::pin(foo<...>(...))`
    ///
    /// - A function returning an async (move) block, optionally `Box::pin`-ed,
    ///   as generated by `async-trait >= 0.1.44`:
    ///   `Box::pin(async move { ... })`
    ///
    /// We the return the statement that must be instrumented, along with some
    /// other information.
    /// 'gen_body' will then be able to use that information to instrument the
    /// proper function/future.
    ///
    /// (this follows the approach suggested in
    /// https://github.com/dtolnay/async-trait/issues/45#issuecomment-571245673)
    pub(crate) fn from_fn(input: &'block ItemFn) -> Option<Self> {
        // are we in an async context? If yes, this isn't a manual async-like pattern
        if input.sig.asyncness.is_some() {
            return None;
        }

        let block = &input.block;

        // list of async functions declared inside the block
        let inside_funs = block.stmts.iter().filter_map(|stmt| {
            if let Stmt::Item(Item::Fn(fun)) = &stmt {
                // If the function is async, this is a candidate
                if fun.sig.asyncness.is_some() {
                    return Some((stmt, fun));
                }
            }
            None
        });

        // last expression of the block: it determines the return value of the
        // block, this is quite likely a `Box::pin` statement or an async block
        let (last_expr_stmt, last_expr) = block.stmts.iter().rev().find_map(|stmt| {
            if let Stmt::Expr(expr, _semi) = stmt {
                Some((stmt, expr))
            } else {
                None
            }
        })?;

        // is the last expression an async block?
        if let Expr::Async(async_expr) = last_expr {
            return Some(AsyncInfo {
                source_stmt: last_expr_stmt,
                kind: AsyncKind::Async {
                    async_expr,
                    pinned_box: false,
                },
                self_type: None,
                input,
            });
        }

        // is the last expression a function call?
        let (outside_func, outside_args) = match last_expr {
            Expr::Call(ExprCall { func, args, .. }) => (func, args),
            _ => return None,
        };

        // is it a call to `Box::pin()`?
        let path = match outside_func.as_ref() {
            Expr::Path(path) => &path.path,
            _ => return None,
        };
        if !path_to_string(path).ends_with("Box::pin") {
            return None;
        }

        // Does the call take an argument? If it doesn't,
        // it's not gonna compile anyway, but that's no reason
        // to (try to) perform an out of bounds access
        if outside_args.is_empty() {
            return None;
        }

        // Is the argument to Box::pin an async block that
        // captures its arguments?
        if let Expr::Async(async_expr) = &outside_args[0] {
            return Some(AsyncInfo {
                source_stmt: last_expr_stmt,
                kind: AsyncKind::Async {
                    async_expr,
                    pinned_box: true,
                },
                self_type: None,
                input,
            });
        }

        // Is the argument to Box::pin a function call itself?
        let func = match &outside_args[0] {
            Expr::Call(ExprCall { func, .. }) => func,
            _ => return None,
        };

        // "stringify" the path of the function called
        let func_name = match **func {
            Expr::Path(ref func_path) => path_to_string(&func_path.path),
            _ => return None,
        };

        // Was that function defined inside of the current block?
        // If so, retrieve the statement where it was declared and the function itself
        let (stmt_func_declaration, func) = inside_funs
            .into_iter()
            .find(|(_, fun)| fun.sig.ident == func_name)?;

        // If "_self" is present as an argument, we store its type to be able to rewrite "Self" (the
        // parameter type) with the type of "_self"
        let mut self_type = None;
        for arg in &func.sig.inputs {
            if let FnArg::Typed(ty) = arg {
                if let Pat::Ident(PatIdent { ref ident, .. }) = *ty.pat {
                    if ident == "_self" {
                        let mut ty = *ty.ty.clone();
                        // extract the inner type if the argument is "&self" or "&mut self"
                        if let Type::Reference(syn::TypeReference { elem, .. }) = ty {
                            ty = *elem;
                        }

                        if let Type::Path(tp) = ty {
                            self_type = Some(tp);
                            break;
                        }
                    }
                }
            }
        }

        Some(AsyncInfo {
            source_stmt: stmt_func_declaration,
            kind: AsyncKind::Function(func),
            self_type,
            input,
        })
    }

    pub(crate) fn gen_async(
        self,
        args: InstrumentArgs,
        instrumented_function_name: &str,
    ) -> Result<proc_macro::TokenStream, syn::Error> {
        // let's rewrite some statements!
        let mut out_stmts: Vec<TokenStream> = self
            .input
            .block
            .stmts
            .iter()
            .map(|stmt| stmt.to_token_stream())
            .collect();

        if let Some((iter, _stmt)) = self
            .input
            .block
            .stmts
            .iter()
            .enumerate()
            .find(|(_iter, stmt)| *stmt == self.source_stmt)
        {
            // instrument the future by rewriting the corresponding statement
            out_stmts[iter] = match self.kind {
                // `Box::pin(immediately_invoked_async_fn())`
                AsyncKind::Function(fun) => {
                    let fun = MaybeItemFn::from(fun.clone());
                    gen_function(
                        fun.as_ref(),
                        args,
                        instrumented_function_name,
                        self.self_type.as_ref(),
                    )
                }
                // `async move { ... }`, optionally pinned
                AsyncKind::Async {
                    async_expr,
                    pinned_box,
                } => {
                    let instrumented_block = gen_block(
                        &async_expr.block,
                        &self.input.sig.inputs,
                        true,
                        args,
                        instrumented_function_name,
                        None,
                    );
                    let async_attrs = &async_expr.attrs;
                    if pinned_box {
                        quote! {
                            ::std::boxed::Box::pin(#(#async_attrs) * async move { #instrumented_block })
                        }
                    } else {
                        quote! {
                            #(#async_attrs) * async move { #instrumented_block }
                        }
                    }
                }
            };
        }

        let vis = &self.input.vis;
        let sig = &self.input.sig;
        let attrs = &self.input.attrs;
        Ok(quote!(
            #(#attrs) *
            #vis #sig {
                #(#out_stmts) *
            }
        )
        .into())
    }
}

// Return a path as a String
fn path_to_string(path: &Path) -> String {
    use std::fmt::Write;
    // some heuristic to prevent too many allocations
    let mut res = String::with_capacity(path.segments.len() * 5);
    for i in 0..path.segments.len() {
        write!(&mut res, "{}", path.segments[i].ident)
            .expect("writing to a String should never fail");
        if i < path.segments.len() - 1 {
            res.push_str("::");
        }
    }
    res
}

/// A visitor struct to replace idents and types in some piece
/// of code (e.g. the "self" and "Self" tokens in user-supplied
/// fields expressions when the function is generated by an old
/// version of async-trait).
struct IdentAndTypesRenamer<'a> {
    types: Vec<(&'a str, TypePath)>,
    idents: Vec<(Ident, Ident)>,
}

impl VisitMut for IdentAndTypesRenamer<'_> {
    // we deliberately compare strings because we want to ignore the spans
    // If we apply clippy's lint, the behavior changes
    #[allow(clippy::cmp_owned)]
    fn visit_ident_mut(&mut self, id: &mut Ident) {
        for (old_ident, new_ident) in &self.idents {
            if id.to_string() == old_ident.to_string() {
                *id = new_ident.clone();
            }
        }
    }

    fn visit_type_mut(&mut self, ty: &mut Type) {
        for (type_name, new_type) in &self.types {
            if let Type::Path(TypePath { path, .. }) = ty {
                if path_to_string(path) == *type_name {
                    *ty = Type::Path(new_type.clone());
                }
            }
        }
    }
}

// A visitor struct that replace an async block by its patched version
struct AsyncTraitBlockReplacer<'a> {
    block: &'a Block,
    patched_block: Block,
}

impl VisitMut for AsyncTraitBlockReplacer<'_> {
    fn visit_block_mut(&mut self, i: &mut Block) {
        if i == self.block {
            *i = self.patched_block.clone();
        }
    }
}

// Replaces any `impl Trait` with `_` so it can be used as the type in
// a `let` statement's LHS.
struct ImplTraitEraser;

impl VisitMut for ImplTraitEraser {
    fn visit_type_mut(&mut self, t: &mut Type) {
        if let Type::ImplTrait(..) = t {
            *t = syn::TypeInfer {
                underscore_token: Token![_](t.span()),
            }
            .into();
        } else {
            syn::visit_mut::visit_type_mut(self, t);
        }
    }
}

fn erase_impl_trait(ty: &Type) -> Type {
    let mut ty = ty.clone();
    ImplTraitEraser.visit_type_mut(&mut ty);
    ty
}

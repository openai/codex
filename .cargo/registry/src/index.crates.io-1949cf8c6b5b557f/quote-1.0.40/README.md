Rust Quasi-Quoting
==================

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/quote-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/quote)
[<img alt="crates.io" src="https://img.shields.io/crates/v/quote.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/quote)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-quote-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/quote)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/quote/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/quote/actions?query=branch%3Amaster)

This crate provides the [`quote!`] macro for turning Rust syntax tree data
structures into tokens of source code.

[`quote!`]: https://docs.rs/quote/1.0/quote/macro.quote.html

Procedural macros in Rust receive a stream of tokens as input, execute arbitrary
Rust code to determine how to manipulate those tokens, and produce a stream of
tokens to hand back to the compiler to compile into the caller's crate.
Quasi-quoting is a solution to one piece of that &mdash; producing tokens to
return to the compiler.

The idea of quasi-quoting is that we write *code* that we treat as *data*.
Within the `quote!` macro, we can write what looks like code to our text editor
or IDE. We get all the benefits of the editor's brace matching, syntax
highlighting, indentation, and maybe autocompletion. But rather than compiling
that as code into the current crate, we can treat it as data, pass it around,
mutate it, and eventually hand it back to the compiler as tokens to compile into
the macro caller's crate.

This crate is motivated by the procedural macro use case, but is a
general-purpose Rust quasi-quoting library and is not specific to procedural
macros.

```toml
[dependencies]
quote = "1.0"
```

*Version requirement: Quote supports rustc 1.56 and up.*<br>
[*Release notes*](https://github.com/dtolnay/quote/releases)

<br>

## Syntax

The quote crate provides a [`quote!`] macro within which you can write Rust code
that gets packaged into a [`TokenStream`] and can be treated as data. You should
think of `TokenStream` as representing a fragment of Rust source code.

[`TokenStream`]: https://docs.rs/proc-macro2/1.0/proc_macro2/struct.TokenStream.html

Within the `quote!` macro, interpolation is done with `#var`. Any type
implementing the [`quote::ToTokens`] trait can be interpolated. This includes
most Rust primitive types as well as most of the syntax tree types from [`syn`].

[`quote::ToTokens`]: https://docs.rs/quote/1.0/quote/trait.ToTokens.html
[`syn`]: https://github.com/dtolnay/syn

```rust
let tokens = quote! {
    struct SerializeWith #generics #where_clause {
        value: &'a #field_ty,
        phantom: core::marker::PhantomData<#item_ty>,
    }

    impl #generics serde::Serialize for SerializeWith #generics #where_clause {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            #path(self.value, serializer)
        }
    }

    SerializeWith {
        value: #value,
        phantom: core::marker::PhantomData::<#item_ty>,
    }
};
```

<br>

## Repetition

Repetition is done using `#(...)*` or `#(...),*` similar to `macro_rules!`. This
iterates through the elements of any variable interpolated within the repetition
and inserts a copy of the repetition body for each one. The variables in an
interpolation may be a `Vec`, slice, `BTreeSet`, or any `Iterator`.

- `#(#var)*` — no separators
- `#(#var),*` — the character before the asterisk is used as a separator
- `#( struct #var; )*` — the repetition can contain other things
- `#( #k => println!("{}", #v), )*` — even multiple interpolations

Note that there is a difference between `#(#var ,)*` and `#(#var),*`—the latter
does not produce a trailing comma. This matches the behavior of delimiters in
`macro_rules!`.

<br>

## Returning tokens to the compiler

The `quote!` macro evaluates to an expression of type
`proc_macro2::TokenStream`. Meanwhile Rust procedural macros are expected to
return the type `proc_macro::TokenStream`.

The difference between the two types is that `proc_macro` types are entirely
specific to procedural macros and cannot ever exist in code outside of a
procedural macro, while `proc_macro2` types may exist anywhere including tests
and non-macro code like main.rs and build.rs. This is why even the procedural
macro ecosystem is largely built around `proc_macro2`, because that ensures the
libraries are unit testable and accessible in non-macro contexts.

There is a [`From`]-conversion in both directions so returning the output of
`quote!` from a procedural macro usually looks like `tokens.into()` or
`proc_macro::TokenStream::from(tokens)`.

[`From`]: https://doc.rust-lang.org/std/convert/trait.From.html

<br>

## Examples

### Combining quoted fragments

Usually you don't end up constructing an entire final `TokenStream` in one
piece. Different parts may come from different helper functions. The tokens
produced by `quote!` themselves implement `ToTokens` and so can be interpolated
into later `quote!` invocations to build up a final result.

```rust
let type_definition = quote! {...};
let methods = quote! {...};

let tokens = quote! {
    #type_definition
    #methods
};
```

### Constructing identifiers

Suppose we have an identifier `ident` which came from somewhere in a macro
input and we need to modify it in some way for the macro output. Let's consider
prepending the identifier with an underscore.

Simply interpolating the identifier next to an underscore will not have the
behavior of concatenating them. The underscore and the identifier will continue
to be two separate tokens as if you had written `_ x`.

```rust
// incorrect
quote! {
    let mut _#ident = 0;
}
```

The solution is to build a new identifier token with the correct value. As this
is such a common case, the `format_ident!` macro provides a convenient utility
for doing so correctly.

```rust
let varname = format_ident!("_{}", ident);
quote! {
    let mut #varname = 0;
}
```

Alternatively, the APIs provided by Syn and proc-macro2 can be used to directly
build the identifier. This is roughly equivalent to the above, but will not
handle `ident` being a raw identifier.

```rust
let concatenated = format!("_{}", ident);
let varname = syn::Ident::new(&concatenated, ident.span());
quote! {
    let mut #varname = 0;
}
```

### Making method calls

Let's say our macro requires some type specified in the macro input to have a
constructor called `new`. We have the type in a variable called `field_type` of
type `syn::Type` and want to invoke the constructor.

```rust
// incorrect
quote! {
    let value = #field_type::new();
}
```

This works only sometimes. If `field_type` is `String`, the expanded code
contains `String::new()` which is fine. But if `field_type` is something like
`Vec<i32>` then the expanded code is `Vec<i32>::new()` which is invalid syntax.
Ordinarily in handwritten Rust we would write `Vec::<i32>::new()` but for macros
often the following is more convenient.

```rust
quote! {
    let value = <#field_type>::new();
}
```

This expands to `<Vec<i32>>::new()` which behaves correctly.

A similar pattern is appropriate for trait methods.

```rust
quote! {
    let value = <#field_type as core::default::Default>::default();
}
```

<br>

## Hygiene

Any interpolated tokens preserve the `Span` information provided by their
`ToTokens` implementation. Tokens that originate within a `quote!` invocation
are spanned with [`Span::call_site()`].

[`Span::call_site()`]: https://docs.rs/proc-macro2/1.0/proc_macro2/struct.Span.html#method.call_site

A different span can be provided explicitly through the [`quote_spanned!`]
macro.

[`quote_spanned!`]: https://docs.rs/quote/1.0/quote/macro.quote_spanned.html

<br>

## Non-macro code generators

When using `quote` in a build.rs or main.rs and writing the output out to a
file, consider having the code generator pass the tokens through [prettyplease]
before writing. This way if an error occurs in the generated code it is
convenient for a human to read and debug.

Be aware that no kind of hygiene or span information is retained when tokens are
written to a file; the conversion from tokens to source code is lossy.

Example usage in build.rs:

```rust
let output = quote! { ... };
let syntax_tree = syn::parse2(output).unwrap();
let formatted = prettyplease::unparse(&syntax_tree);

let out_dir = env::var_os("OUT_DIR").unwrap();
let dest_path = Path::new(&out_dir).join("out.rs");
fs::write(dest_path, formatted).unwrap();
```

[prettyplease]: https://github.com/dtolnay/prettyplease

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>

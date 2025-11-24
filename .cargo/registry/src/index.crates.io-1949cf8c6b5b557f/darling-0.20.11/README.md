# Darling

[![Build Status](https://github.com/TedDriggs/darling/workflows/CI/badge.svg)](https://github.com/TedDriggs/darling/actions)
[![Latest Version](https://img.shields.io/crates/v/darling.svg)](https://crates.io/crates/darling)
![Rustc Version 1.56+](https://img.shields.io/badge/rustc-1.56+-lightgray.svg)

`darling` is a crate for proc macro authors, which enables parsing attributes into structs. It is heavily inspired by `serde` both in its internals and in its API.

# Benefits

-   Easy and declarative parsing of macro input - make your proc-macros highly controllable with minimal time investment.
-   Great validation and errors, no work required. When users of your proc-macro make a mistake, `darling` makes sure they get error markers at the right place in their source, and provides "did you mean" suggestions for misspelled fields.

# Usage

`darling` provides a set of traits which can be derived or manually implemented.

1. `FromMeta` is used to extract values from a meta-item in an attribute. Implementations are likely reusable for many libraries, much like `FromStr` or `serde::Deserialize`. Trait implementations are provided for primitives, some std types, and some `syn` types.
2. `FromDeriveInput` is implemented or derived by each proc-macro crate which depends on `darling`. This is the root for input parsing; it gets access to the identity, generics, and visibility of the target type, and can specify which attribute names should be parsed or forwarded from the input AST.
3. `FromField` is implemented or derived by each proc-macro crate which depends on `darling`. Structs deriving this trait will get access to the identity (if it exists), type, and visibility of the field.
4. `FromVariant` is implemented or derived by each proc-macro crate which depends on `darling`. Structs deriving this trait will get access to the identity and contents of the variant, which can be transformed the same as any other `darling` input.
5. `FromAttributes` is a lower-level version of the more-specific `FromDeriveInput`, `FromField`, and `FromVariant` traits. Structs deriving this trait get a meta-item extractor and error collection which works for any syntax element, including traits, trait items, and functions. This is useful for non-derive proc macros.

## Additional Modules

-   `darling::ast` provides generic types for representing the AST.
-   `darling::usage` provides traits and functions for determining where type parameters and lifetimes are used in a struct or enum.
-   `darling::util` provides helper types with special `FromMeta` implementations, such as `PathList`.

# Example

```rust,ignore
use darling::{FromDeriveInput, FromMeta};

#[derive(Default, FromMeta)]
#[darling(default)]
pub struct Lorem {
    #[darling(rename = "sit")]
    ipsum: bool,
    dolor: Option<String>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(my_crate), forward_attrs(allow, doc, cfg))]
pub struct MyTraitOpts {
    ident: syn::Ident,
    attrs: Vec<syn::Attribute>,
    lorem: Lorem,
}
```

The above code will then be able to parse this input:

```rust,ignore
/// A doc comment which will be available in `MyTraitOpts::attrs`.
#[derive(MyTrait)]
#[my_crate(lorem(dolor = "Hello", sit))]
pub struct ConsumingType;
```

# Attribute Macros

Non-derive attribute macros are supported.
To parse arguments for attribute macros, derive `FromMeta` on the argument receiver type, then use `darling::ast::NestedMeta::parse_meta_list` to convert the arguments `TokenStream` to a `Vec<NestedMeta>`, then pass that to the derived `from_list` method on your argument receiver type.
This will produce a normal `darling::Result<T>` that can be used the same as a result from parsing a `DeriveInput`.

## Macro Code

```rust,ignore
use darling::{Error, FromMeta};
use darling::ast::NestedMeta;
use syn::ItemFn;
use proc_macro::TokenStream;

#[derive(Debug, FromMeta)]
struct MacroArgs {
    #[darling(default)]
    timeout_ms: Option<u16>,
    path: String,
}

#[proc_macro_attribute]
pub fn your_attr(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => { return TokenStream::from(Error::from(e).write_errors()); }
    };
    let _input = syn::parse_macro_input!(input as ItemFn);

    let _args = match MacroArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => { return TokenStream::from(e.write_errors()); }
    };

    // do things with `args`
    unimplemented!()
}
```

## Consuming Code

```rust,ignore
use your_crate::your_attr;

#[your_attr(path = "hello", timeout_ms = 15)]
fn do_stuff() {
    println!("Hello");
}
```

# Features

Darling's features are built to work well for real-world projects.

-   **Defaults**: Supports struct- and field-level defaults, using the same path syntax as `serde`.
    Additionally, `Option<T>` and `darling::util::Flag` fields are innately optional; you don't need to declare `#[darling(default)]` for those.
-   **Field Renaming**: Fields can have different names in usage vs. the backing code.
-   **Auto-populated fields**: Structs deriving `FromDeriveInput` and `FromField` can declare properties named `ident`, `vis`, `ty`, `attrs`, and `generics` to automatically get copies of the matching values from the input AST. `FromDeriveInput` additionally exposes `data` to get access to the body of the deriving type, and `FromVariant` exposes `fields`.
    -   **Transformation of forwarded attributes**: You can add `#[darling(with=path)]` to the `attrs` field to use a custom function to transform the forwarded attributes before they're provided to your struct. The function signature is `fn(Vec<Attribute>) -> darling::Result<T>`, where `T` is the type you declared for the `attrs` field. Returning an error from this function will propagate with all other parsing errors.
-   **Mapping function**: Use `#[darling(map="path")]` or `#[darling(and_then="path")]` to specify a function that runs on the result of parsing a meta-item field. This can change the return type, which enables you to parse to an intermediate form and convert that to the type you need in your struct.
-   **Skip fields**: Use `#[darling(skip)]` to mark a field that shouldn't be read from attribute meta-items.
-   **Multiple-occurrence fields**: Use `#[darling(multiple)]` on a `Vec` field to allow that field to appear multiple times in the meta-item. Each occurrence will be pushed into the `Vec`.
-   **Span access**: Use `darling::util::SpannedValue` in a struct to get access to that meta item's source code span. This can be used to emit warnings that point at a specific field from your proc macro. In addition, you can use `darling::Error::write_errors` to automatically get precise error location details in most cases.
-   **"Did you mean" suggestions**: Compile errors from derived darling trait impls include suggestions for misspelled fields.
-   **Struct flattening**: Use `#[darling(flatten)]` to remove one level of structure when presenting your meta item to users. Fields that are not known to the parent struct will be forwarded to the `flatten` field.
-   **Custom shorthand**: Use `#[darling(from_word = ...)]` on a struct or enum to override how a simple word is interpreted. By default, it is an error for your macro's user to fail to specify the fields of your struct, but with this you can choose to instead produce a set of default values. This takes either a path or a closure whose signature matches `FromMeta::from_word`.
-   **Custom handling for missing fields**: When a field is not present and `#[darling(default)]` is not used, derived impls will call `FromMeta::from_none` on that field's type to try and get the fallback value for the field. Usually, there is not a fallback value, so a missing field error is generated. `Option<T: FromMeta>` uses this to make options optional without requiring `#[darling(default)]` declarations, and structs and enums can use this themselves with `#[darling(from_none = ...)]`. This takes either a path or a closure whose signature matches `FromMeta::from_none`.

## Shape Validation

Some proc-macros only work on structs, while others need enums whose variants are either unit or newtype variants.
Darling makes this sort of validation extremely simple.
On the receiver that derives `FromDeriveInput`, add `#[darling(supports(...))]` and then list the shapes that your macro should accept.

| Name             | Description                                                               |
| ---------------- | ------------------------------------------------------------------------- |
| `any`            | Accept anything                                                           |
| `struct_any`     | Accept any struct                                                         |
| `struct_named`   | Accept structs with named fields, e.g. `struct Example { field: String }` |
| `struct_newtype` | Accept newtype structs, e.g. `struct Example(String)`                     |
| `struct_tuple`   | Accept tuple structs, e.g. `struct Example(String, String)`               |
| `struct_unit`    | Accept unit structs, e.g. `struct Example;`                               |
| `enum_any`       | Accept any enum                                                           |
| `enum_named`     | Accept enum variants with named fields                                    |
| `enum_newtype`   | Accept newtype enum variants                                              |
| `enum_tuple`     | Accept tuple enum variants                                                |
| `enum_unit`      | Accept unit enum variants                                                 |

Each one is additive, so listing `#[darling(supports(struct_any, enum_newtype))]` would accept all structs and any enum where every variant is a newtype variant.

This can also be used when deriving `FromVariant`, without the `enum_` prefix.

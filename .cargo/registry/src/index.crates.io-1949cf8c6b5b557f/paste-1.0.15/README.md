Macros for all your token pasting needs
=======================================

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/paste-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/paste)
[<img alt="crates.io" src="https://img.shields.io/crates/v/paste.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/paste)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-paste-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/paste)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/paste/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/paste/actions?query=branch%3Amaster)

The nightly-only [`concat_idents!`] macro in the Rust standard library is
notoriously underpowered in that its concatenated identifiers can only refer to
existing items, they can never be used to define something new.

[`concat_idents!`]: https://doc.rust-lang.org/std/macro.concat_idents.html

This crate provides a flexible way to paste together identifiers in a macro,
including using pasted identifiers to define new items.

```toml
[dependencies]
paste = "1.0"
```

This approach works with any Rust compiler 1.31+.

<br>

## Pasting identifiers

Within the `paste!` macro, identifiers inside `[<`...`>]` are pasted together to
form a single identifier.

```rust
use paste::paste;

paste! {
    // Defines a const called `QRST`.
    const [<Q R S T>]: &str = "success!";
}

fn main() {
    assert_eq!(
        paste! { [<Q R S T>].len() },
        8,
    );
}
```

<br>

## More elaborate example

The next example shows a macro that generates accessor methods for some struct
fields. It demonstrates how you might find it useful to bundle a paste
invocation inside of a macro\_rules macro.

```rust
use paste::paste;

macro_rules! make_a_struct_and_getters {
    ($name:ident { $($field:ident),* }) => {
        // Define a struct. This expands to:
        //
        //     pub struct S {
        //         a: String,
        //         b: String,
        //         c: String,
        //     }
        pub struct $name {
            $(
                $field: String,
            )*
        }

        // Build an impl block with getters. This expands to:
        //
        //     impl S {
        //         pub fn get_a(&self) -> &str { &self.a }
        //         pub fn get_b(&self) -> &str { &self.b }
        //         pub fn get_c(&self) -> &str { &self.c }
        //     }
        paste! {
            impl $name {
                $(
                    pub fn [<get_ $field>](&self) -> &str {
                        &self.$field
                    }
                )*
            }
        }
    }
}

make_a_struct_and_getters!(S { a, b, c });

fn call_some_getters(s: &S) -> bool {
    s.get_a() == s.get_b() && s.get_c().is_empty()
}
```

<br>

## Case conversion

Use `$var:lower` or `$var:upper` in the segment list to convert an interpolated
segment to lower- or uppercase as part of the paste. For example, `[<ld_
$reg:lower _expr>]` would paste to `ld_bc_expr` if invoked with $reg=`Bc`.

Use `$var:snake` to convert CamelCase input to snake\_case.
Use `$var:camel` to convert snake\_case to CamelCase.
These compose, so for example `$var:snake:upper` would give you SCREAMING\_CASE.

The precise Unicode conversions are as defined by [`str::to_lowercase`] and
[`str::to_uppercase`].

[`str::to_lowercase`]: https://doc.rust-lang.org/std/primitive.str.html#method.to_lowercase
[`str::to_uppercase`]: https://doc.rust-lang.org/std/primitive.str.html#method.to_uppercase

<br>

## Pasting documentation strings

Within the `paste!` macro, arguments to a #\[doc ...\] attribute are implicitly
concatenated together to form a coherent documentation string.

```rust
use paste::paste;

macro_rules! method_new {
    ($ret:ident) => {
        paste! {
            #[doc = "Create a new `" $ret "` object."]
            pub fn new() -> $ret { todo!() }
        }
    };
}

pub struct Paste {}

method_new!(Paste);  // expands to #[doc = "Create a new `Paste` object"]
```

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

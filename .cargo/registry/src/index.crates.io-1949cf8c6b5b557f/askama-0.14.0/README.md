# askama

[![Crates.io](https://img.shields.io/crates/v/askama?logo=rust&style=flat-square&logoColor=white "Crates.io")](https://crates.io/crates/askama)
[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/askama-rs/askama/rust.yml?branch=master&logo=github&style=flat-square&logoColor=white "GitHub Workflow Status")](https://github.com/askama-rs/askama/actions/workflows/rust.yml)
[![Book](https://img.shields.io/readthedocs/askama?label=book&logo=readthedocs&style=flat-square&logoColor=white "Book")](https://askama.readthedocs.io/)
[![docs.rs](https://img.shields.io/docsrs/askama?logo=docsdotrs&style=flat-square&logoColor=white "docs.rs")](https://docs.rs/askama/)

**Askama** implements a template rendering engine based on [Jinja](https://jinja.palletsprojects.com/),
and generates type-safe Rust code from your templates at compile time
based on a user-defined `struct` to hold the template's context.

At some point, **Askama** got forked into **Rinja** (explained [here](https://blog.guillaume-gomez.fr/articles/2024-07-31+docs.rs+switching+jinja+template+framework+from+tera+to+rinja))
before getting merged back into **Askama**.

All feedback welcome! Feel free to file bugs, requests for documentation and
any other feedback to the [issue tracker][issues].

You can find the documentation about our syntax, features, configuration in our book:
[askama.readthedocs.io](https://askama.readthedocs.io/).

Have a look at our [*Askama Playground*](https://askama-rs.github.io/askama_playground/),
if you want to try out askama's code generation online.

### Feature highlights

* Construct templates using a familiar, easy-to-use syntax
* Benefit from the safety provided by Rust's type system
* Template code is compiled into your crate for optimal performance
* Debugging features to assist you in template development
* Templates must be valid UTF-8 and produce UTF-8 when rendered
* Works on stable Rust

### Supported in templates

* Template inheritance
* Loops, if/else statements and include support
* Macro support
* Variables (no mutability allowed)
* Some built-in filters, and the ability to use your own
* Whitespace suppressing with '-' markers
* Opt-out HTML escaping
* Syntax customization

[issues]: https://github.com/askama-rs/askama/issues


How to get started
------------------

First, add the askama dependency to your crate's `Cargo.toml`:

```sh
cargo add askama
```

Now create a directory called `templates` in your crate root.
In it, create a file called `hello.html`, containing the following:

```jinja
Hello, {{ name }}!
```

In any Rust file inside your crate, add the following:

```rust
use askama::Template; // bring trait in scope

#[derive(Template)] // this will generate the code...
#[template(path = "hello.html")] // using the template in this path, relative
                                 // to the `templates` dir in the crate root
struct HelloTemplate<'a> { // the name of the struct can be anything
    name: &'a str, // the field name should match the variable name
                   // in your template
}

fn main() {
    let hello = HelloTemplate { name: "world" }; // instantiate your struct
    println!("{}", hello.render().unwrap()); // then render it.
}
```

You should now be able to compile and run this code.

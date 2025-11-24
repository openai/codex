[![Build Status](https://travis-ci.org/TedDriggs/ident_case.svg?branch=master)](https://travis-ci.org/TedDriggs/ident_case)

Crate for manipulating case of identifiers in Rust programs.

# Features
* Supports `snake_case`, `lowercase`, `camelCase`, 
  `PascalCase`, `SCREAMING_SNAKE_CASE`, and `kebab-case`
* Rename variants, and fields

# Examples
```rust
assert_eq!("helloWorld", RenameRule::CamelCase.apply_to_field("hello_world"));

assert_eq!("i_love_serde", RenameRule::SnakeCase.apply_to_variant("ILoveSerde"));
```
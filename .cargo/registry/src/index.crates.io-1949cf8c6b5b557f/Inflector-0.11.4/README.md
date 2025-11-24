# Rust Inflector


[![Build Status](https://travis-ci.org/whatisinternet/Inflector.svg?branch=master)](https://travis-ci.org/whatisinternet/Inflector) [![Crates.io](https://img.shields.io/crates/v/Inflector.svg)](https://crates.io/crates/inflector)[![Crate downloads](https://img.shields.io/crates/d/Inflector.svg)](https://crates.io/crates/inflector)


Adds String based inflections for Rust. Snake, kebab, train, camel,
sentence, class, and title cases as well as ordinalize,
deordinalize, demodulize, deconstantize, foreign key, table case, and pluralize/singularize are supported as both traits and pure functions
acting on &str and String types.

-----
## Documentation:

Documentation can be found here at the README or via rust docs below.

[Rust docs with examples](https://docs.rs/Inflector)

-----

## Installation:

### As a [crate](http://crates.io)

```toml
[dependencies]
Inflector = "*"
```

### Compile yourself:

1. Install [Rust and cargo](http://doc.crates.io/)
2. git clone https://github.com/whatisinternet/Inflector
3. Library: cd inflector && cargo build --release --lib
4. You can find the library in target/release

## Usage / Example:

```rust
...
// to use methods like String.to_lower_case();
extern crate inflector;
use inflector::Inflector;
...
fn main() {
...
  let camel_case_string: String = "some_string".to_camel_case();
...
}

```

Or

```rust
...
// to use methods like to_snake_case(&str);
extern crate inflector;

// use inflector::cases::classcase::to_class_case;
// use inflector::cases::classcase::is_class_case;

// use inflector::cases::camelcase::to_camel_case;
// use inflector::cases::camelcase::is_camel_case;

// use inflector::cases::pascalcase::to_pascal_case;
// use inflector::cases::pascalcase::is_pascal_case;

// use inflector::cases::screamingsnakecase::to_screamingsnake_case;
// use inflector::cases::screamingsnakecase::is_screamingsnake_case;

// use inflector::cases::snakecase::to_snake_case;
// use inflector::cases::snakecase::is_snake_case;

// use inflector::cases::kebabcase::to_kebab_case;
// use inflector::cases::kebabcase::is_kebab_case;

// use inflector::cases::traincase::to_train_case;
// use inflector::cases::traincase::is_train_case;

// use inflector::cases::sentencecase::to_sentence_case;
// use inflector::cases::sentencecase::is_sentence_case;

// use inflector::cases::titlecase::to_title_case;
// use inflector::cases::titlecase::is_title_case;

// use inflector::cases::tablecase::to_table_case;
// use inflector::cases::tablecase::is_table_case;

// use inflector::numbers::ordinalize::ordinalize;
// use inflector::numbers::deordinalize::deordinalize;

// use inflector::suffix::foreignkey::to_foreign_key;
// use inflector::suffix::foreignkey::is_foreign_key;

// use inflector::string::demodulize::demodulize;
// use inflector::string::deconstantize::deconstantize;

// use inflector::string::pluralize::to_plural;
// use inflector::string::singularize::to_singular;
...
fn main() {
...
  let camel_case_string: String = to_camel_case("some_string");
...
}

```

## Advanced installation and usage:

If the project doesn't require singularize, pluralize, class, table, demodulize,
deconstantize. Then in your `cargo.toml` you may wish to specify:

```toml
[dependencies.Inflector]
version = "*"
default-features = false
```

Or

```toml
Inflector = {version="*", default-features=false}

```

To test this crate locally with features off try:

```shell
cargo test --no-default-features
```

## [Contributing](CONTRIBUTING.md)

This project is intended to be a safe, welcoming space for collaboration, and contributors are expected to adhere to the [Contributor Covenant](http://contributor-covenant.org) code of conduct.

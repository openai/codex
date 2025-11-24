/*!
A high performance, zero-copy URL router.

```rust
use matchit::Router;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut router = Router::new();
    router.insert("/home", "Welcome!")?;
    router.insert("/users/{id}", "A User")?;

    let matched = router.at("/users/978")?;
    assert_eq!(matched.params.get("id"), Some("978"));
    assert_eq!(*matched.value, "A User");

    Ok(())
}
```

# Parameters

The router supports dynamic route segments. These can either be named or catch-all parameters.

Named parameters like `/{id}` match anything until the next `/` or the end of the path. Note that named parameters must be followed
by a `/` or the end of the route. Dynamic suffixes are not currently supported.

```rust
# use matchit::Router;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut m = Router::new();
m.insert("/users/{id}", true)?;

assert_eq!(m.at("/users/1")?.params.get("id"), Some("1"));
assert_eq!(m.at("/users/23")?.params.get("id"), Some("23"));
assert!(m.at("/users").is_err());
# Ok(())
# }
```

Catch-all parameters start with `*` and match anything until the end of the path. They must always be at the **end** of the route.

```rust
# use matchit::Router;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut m = Router::new();
m.insert("/{*p}", true)?;

assert_eq!(m.at("/foo.js")?.params.get("p"), Some("foo.js"));
assert_eq!(m.at("/c/bar.css")?.params.get("p"), Some("c/bar.css"));

// note that this will not match
assert!(m.at("/").is_err());
# Ok(())
# }
```

The literal characters `{` and `}` may be included in a static route by escaping them with the same character. For example, the `{` character is escaped with `{{` and the `}` character is escaped with `}}`.

```rust
# use matchit::Router;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut m = Router::new();
m.insert("/{{hello}}", true)?;
m.insert("/{hello}", true)?;

// match the static route
assert!(m.at("/{hello}")?.value);

// match the dynamic route
assert_eq!(m.at("/hello")?.params.get("hello"), Some("hello"));
# Ok(())
# }
```

# Routing Priority

Static and dynamic route segments are allowed to overlap. If they do, static segments will be given higher priority:

```rust
# use matchit::Router;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut m = Router::new();
m.insert("/", "Welcome!").unwrap();      // priority: 1
m.insert("/about", "About Me").unwrap(); // priority: 1
m.insert("/{*filepath}", "...").unwrap();  // priority: 2
# Ok(())
# }
```

# How does it work?

The router takes advantage of the fact that URL routes generally follow a hierarchical structure. Routes are stored them in a radix trie that makes heavy use of common prefixes.

```text
Priority   Path             Value
9          \                1
3          ├s               None
2          |├earch\         2
1          |└upport\        3
2          ├blog\           4
1          |    └{post}     None
1          |          └\    5
2          ├about-us\       6
1          |        └team\  7
1          └contact\        8
```

This allows us to reduce the route search to a small number of branches. Child nodes on the same level of the tree are also prioritized
by the number of children with registered values, increasing the chance of choosing the correct branch of the first try.

As it turns out, this method of routing is extremely fast. See the [benchmark results](https://github.com/ibraheemdev/matchit?tab=readme-ov-file#benchmarks) for details.
*/

#![deny(rust_2018_idioms, clippy::all)]

mod error;
mod escape;
mod params;
mod router;
mod tree;

pub use error::{InsertError, MatchError};
pub use params::{Params, ParamsIter};
pub use router::{Match, Router};

#[cfg(doctest)]
mod readme {
    #[allow(dead_code)]
    #[doc = include_str!("../README.md")]
    struct Readme;
}

Clone trait that is dyn-compatible
==================================

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/dyn--clone-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/dyn-clone)
[<img alt="crates.io" src="https://img.shields.io/crates/v/dyn-clone.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/dyn-clone)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-dyn--clone-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/dyn-clone)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/dyn-clone/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/dyn-clone/actions?query=branch%3Amaster)

This crate provides a `DynClone` trait that can be used in trait objects, and a
`clone_box` function that can clone any sized or dynamically sized
implementation of `DynClone`. Types that implement the standard library's
[`std::clone::Clone`] trait are automatically usable by a `DynClone` trait
object.

[`std::clone::Clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html

The signature of `clone_box` is:

```rust
fn clone_box<T>(t: &T) -> Box<T>
where
    T: ?Sized + DynClone
```

## Example

```rust
use dyn_clone::DynClone;

trait MyTrait: DynClone {
    fn recite(&self);
}

impl MyTrait for String {
    fn recite(&self) {
        println!("{} ♫", self);
    }
}

fn main() {
    let line = "The slithy structs did gyre and gimble the namespace";

    // Build a trait object holding a String.
    // This requires String to implement MyTrait and std::clone::Clone.
    let x: Box<dyn MyTrait> = Box::new(String::from(line));

    x.recite();

    // The type of x2 is a Box<dyn MyTrait> cloned from x.
    let x2 = dyn_clone::clone_box(&*x);

    x2.recite();
}
```

This crate includes a macro for generating the implementation `impl
std::clone::Clone for Box<dyn MyTrait>` in terms of `dyn_clone::clone_box`:

```rust
// As before.
trait MyTrait: DynClone {
    /* ... */
}

dyn_clone::clone_trait_object!(MyTrait);

// Now data structures containing Box<dyn MyTrait> can derive Clone:
#[derive(Clone)]
struct Container {
    trait_object: Box<dyn MyTrait>,
}
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

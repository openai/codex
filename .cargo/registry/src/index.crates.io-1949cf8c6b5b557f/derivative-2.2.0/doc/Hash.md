# Custom attributes
The `Hash` trait supports the following attributes:

* **Container attributes**
    * [`Hash(bound="<where-clause or empty>")`](#custom-bound)
* **Field attributes**
    * [`Hash(bound="<where-clause or empty>")`](#custom-bound)
    * [`Hash(hash_with="<path>")`](#hash-with)
    * [`Hash="ignore"`](#ignoring-a-field)

# Ignoring a field

You can use *derivative* to ignore fields from a `Hash` implementation:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(Hash)]
struct Foo {
    foo: u8,
    #[derivative(Hash="ignore")]
    bar: i32,
}

#[derive(Hash)]
struct Bar {
    foo: u8,
}

# fn hash<T: std::hash::Hash>(t: &T) -> u64 {
#     use std::hash::Hasher;
#     let mut s = std::collections::hash_map::DefaultHasher::new();
#     t.hash(&mut s);
#     s.finish()
# }
# 
assert_eq!(hash(&Foo { foo: 42, bar: -1337 }), hash(&Bar { foo: 42 }));
```

# Hash with

You can pass a field to a hash function:

```rust
# extern crate derivative;
# use derivative::Derivative;
# mod path {
#   pub struct SomeTypeThatMightNotBeHash;
#   pub mod to {
#     pub fn my_hash_fn<H>(_: &super::SomeTypeThatMightNotBeHash, state: &mut H) where H: std::hash::Hasher { unimplemented!() }
#   }
# }
# use path::SomeTypeThatMightNotBeHash;
#[derive(Derivative)]
#[derivative(Hash)]
struct Foo {
    foo: u32,
    #[derivative(Hash(hash_with="path::to::my_hash_fn"))]
    bar: SomeTypeThatMightNotBeHash,
}
```

The field `bar` will be hashed with `path::to::my_hash_fn(&bar, &mut state)`
where `state` is the current [`Hasher`].

The function must the following prototype:

```rust,ignore
fn my_hash_fn<H>(&T, state: &mut H) where H: Hasher;
```

# Limitations

On structure, `derivative(Hash)` will produce the same hash as `derive(Hash)`.
On unions however, it will produces the same hashes *only for unitary
variants*!

# Custom bound
As most other traits, `Hash` supports a custom bound on container and fields.
See [`Debug`'s documentation](Debug.md#custom-bound) for more information.

[`Hasher`]: https://doc.rust-lang.org/std/hash/trait.Hasher.html

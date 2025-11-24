# Custom attributes
The `PartialEq`, `Eq`, `PartialOrd` and `Eq` and traits support the following attributes:

* **Container attributes**
    * [`<CmpTrait>(bound="<where-clause or empty>")`](#custom-bound)
* **Field attributes**
    * [`<CmpTrait>(bound="<where-clause or empty>")`](#custom-bound)

The `PartialEq`, `PartialOrd` and `Ord` traits also supports the following attributes:

* **Container attributes**
    * [`<CmpTrait>="feature_allow_slow_enum"`](#enumerations)
* **Field attributes**
    * [`<CmpTrait>="ignore"`](#ignoring-a-field)
    * [`<CmpTrait>(compare_with="<path>")`](#compare-with)

(These attributes are not relevant for `Eq` which is just a marker trait.)

# Enumerations

Unfortunately, there is no way for derivative to derive `PartialOrd` or `Ord` on
enumerations as efficiently as the built-in `derive(…)` yet.

If you want to use derivative on enumerations anyway, you can add

```rust,ignore
#[derivative(PartialOrd="feature_allow_slow_enum")]
```

to your enumeration. This acts as a “feature-gate”.

This attribute is also allowed for `PartialEq` for historical reason. It is not
necessary anymore as of v2.1.0. It was never necessary nor allowed for `Eq`.

# Ignoring a field

You can use *derivative* to ignore a field when comparing:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(PartialEq)]
struct Foo {
    foo: u8,
    #[derivative(PartialEq="ignore")]
    bar: u8,
}

assert!(Foo { foo: 0, bar: 42 } == Foo { foo: 0, bar: 7});
assert!(Foo { foo: 42, bar: 0 } != Foo { foo: 7, bar: 0});
```

# Compare with

Usually fields are compared using `==`, `PartialOrd::partial_cmp` or `Ord::cmp`. You can use an alternative comparison
function if you like:

```rust
# extern crate derivative;
# use derivative::Derivative;
# mod path {
#   pub struct SomeTypeThatMightNotBePartialEq;
#   pub mod to {
#     pub fn my_cmp_fn(_: &super::SomeTypeThatMightNotBePartialEq, _: &super::SomeTypeThatMightNotBePartialEq) -> bool { false }
#   }
# }
# use path::SomeTypeThatMightNotBePartialEq;
#[derive(Derivative)]
#[derivative(PartialEq)]
struct Foo {
    foo: u32,
    #[derivative(PartialEq(compare_with="path::to::my_cmp_fn"))]
    bar: SomeTypeThatMightNotBePartialEq,
}
```

`foo` will be compared with `==` and `bar` will be compared with
`path::to::my_cmp_fn` which must have the following prototype:

| Trait        | Signature |
|--------------|-----------|
| `PartialEq`  | <span class="rust">`fn my_cmp_fn(&T, &T) -> bool;`</span>
| `PartialOrd` | <span class="rust">`fn my_cmp_fn(&T, &T) -> std::option::Option<std::cmp::Ordering>;`</span>
| `Ord`        | <span class="rust">`fn my_cmp_fn(&T, &T) -> std::cmp::Ordering;`</span>

# Custom bound

Usually if you derive `CmpTrait`, a `T: CmpTrait` bound is added for each type parameter `T`. You can use
override this behavior if the inferred bound is not correct for you.

Eg. comparing raw pointers does not require the type to be `Eq`, so you could
use:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(PartialEq)]
struct WithPtr<T: ?Sized> {
    #[derivative(PartialEq(bound=""))]
    foo: *const T
}
```

See [`Default`'s documentation](./Default.md#custom-bound) for more details.

# Packed structures

You can use *derivative* to implement the comparison traits on packed structures. Unlike the standard `derive`, *derivative* does not require the structure itself to be `Copy`, but like the standard `derive`, it requires each (non-ignored) field to be `Copy`.

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct Foo {
    f: u32,
    #[derivative(PartialEq = "ignore")]
    t: String,
}
```
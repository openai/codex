# Custom attributes
The `Default` trait supports the following attributes:

* **Container attributes**
    * [`Default(bound="<where-clause or empty>")`](#custom-bound)
    * [`Default="new"`](#new-function)
* **Variant attributes**
    * [`Default`](#default-enumeration)
* **Field attributes**
    * [`Default(bound="<where-clause or empty>")`](#custom-bound)
    * [`Default(value="<expr>")`](#setting-the-value-of-a-field)

# Default enumeration

You can use *derivative* to derive a default implementation on enumerations!
This does not work with *rustc*'s `#[derive(Default)]`.
All you need is to specify what variant is the default value:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Debug, Derivative)]
#[derivative(Default)]
enum Enum {
    A,
    #[derivative(Default)]
    B,
}

println!("{:?}", Enum::default()); // B
```

# Setting the value of a field

You can use *derivative* to change the default value of a field in a `Default`
implementation:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Debug, Derivative)]
#[derivative(Default)]
struct Foo {
    foo: u8,
    #[derivative(Default(value="42"))]
    bar: u8,
}

println!("{:?}", Foo::default()); // Foo { foo: 0, bar: 42 }
```

# `new` function

You can use *derivative* to derive a convenience `new` method for your type
that calls `Default::default`:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Debug, Derivative)]
#[derivative(Default(new="true"))]
struct Foo {
    foo: u8,
    bar: u8,
}

println!("{:?}", Foo::new()); // Foo { foo: 0, bar: 0 }
```

# Custom bound

The following does not work because `derive` adds a `T: Default` bound on the
`impl Default for Foo<T>`:

```rust,compile_fail
# extern crate derivative;
# use derivative::Derivative;
#[derive(Default)]
struct Foo<T> {
    foo: Option<T>,
}

struct NonDefault;

Foo::<NonDefault>::default(); // gives:
// error: no associated item named `default` found for type `Foo<NonDefault>` in the current scope
//  = note: the method `default` exists but the following trait bounds were not satisfied: `NonDefault : std::default::Default`
```

That bound however is useless as `Option<T>: Default` for any `T`.
`derivative` allows you to explicitly specify a bound if the inferred one is not
correct:

```rust
# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(Default(bound=""))] // don't need any bound
struct Foo<T> {
    foo: Option<T>,
}

struct NonDefault;

Foo::<NonDefault>::default(); // works!
```

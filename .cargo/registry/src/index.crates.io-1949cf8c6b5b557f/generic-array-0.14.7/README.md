[![Crates.io](https://img.shields.io/crates/v/generic-array.svg)](https://crates.io/crates/generic-array)
[![Build Status](https://travis-ci.org/fizyk20/generic-array.svg?branch=master)](https://travis-ci.org/fizyk20/generic-array)
# generic-array

This crate implements generic array types for Rust.

**Requires minumum Rust version of 1.36.0, or 1.41.0 for `From<[T; N]>` implementations**

[Documentation](http://fizyk20.github.io/generic-array/generic_array/)

## Usage

The Rust arrays `[T; N]` are problematic in that they can't be used generically with respect to `N`, so for example this won't work:

```rust
struct Foo<N> {
	data: [i32; N]
}
```

**generic-array** defines a new trait `ArrayLength<T>` and a struct `GenericArray<T, N: ArrayLength<T>>`, which let the above be implemented as:

```rust
struct Foo<N: ArrayLength<i32>> {
	data: GenericArray<i32, N>
}
```

The `ArrayLength<T>` trait is implemented by default for [unsigned integer types](http://fizyk20.github.io/generic-array/typenum/uint/index.html) from [typenum](http://fizyk20.github.io/generic-array/typenum/index.html) crate:

```rust
use generic_array::typenum::U5;

struct Foo<N: ArrayLength<i32>> {
    data: GenericArray<i32, N>
}

fn main() {
    let foo = Foo::<U5>{data: GenericArray::default()};
}
```

For example, `GenericArray<T, U5>` would work almost like `[T; 5]`:

```rust
use generic_array::typenum::U5;

struct Foo<T, N: ArrayLength<T>> {
    data: GenericArray<T, N>
}

fn main() {
    let foo = Foo::<i32, U5>{data: GenericArray::default()};
}
```

In version 0.1.1 an `arr!` macro was introduced, allowing for creation of arrays as shown below:

```rust
let array = arr![u32; 1, 2, 3];
assert_eq!(array[2], 3);
```

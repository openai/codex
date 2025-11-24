# Using `#[derive(Deref)]`

Deriving `Deref` only works for a single field of a struct.
It's possible to use it in two ways:

1. Dereferencing to the field, i.e. like if your type was a reference type.
2. Doing a dereference on the field, for when the field itself is a reference type like `&` and `Box`.

With `#[deref]` or `#[deref(ignore)]` it's possible to indicate the field that
you want to derive `Deref` for.




## Example usage

```rust
# use derive_more::Deref;
#
#[derive(Deref)]
struct Num {
    num: i32,
}

#[derive(Deref)]
#[deref(forward)]
struct MyBoxedInt(Box<i32>);

// You can specify the field you want to derive `Deref` for.
#[derive(Deref)]
struct CoolVec {
    cool: bool,
    #[deref]
    vec: Vec<i32>,
}

let num = Num{num: 123};
let boxed = MyBoxedInt(Box::new(123));
let cool_vec = CoolVec{cool: true, vec: vec![123]};
assert_eq!(123, *num);
assert_eq!(123, *boxed);
assert_eq!(vec![123], *cool_vec);
```




## Structs

When deriving a non-forwarded `Deref` for a struct:

```rust
# use derive_more::Deref;
#
#[derive(Deref)]
struct CoolVec {
    cool: bool,
    #[deref]
    vec: Vec<i32>,
}
```

Code like this will be generated:

```rust
# struct CoolVec {
#     cool: bool,
#     vec: Vec<i32>,
# }
impl derive_more::Deref for CoolVec {
    type Target = Vec<i32>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}
```

When deriving a forwarded `Deref` for a struct:

```rust
# use derive_more::Deref;
#
#[derive(Deref)]
#[deref(forward)]
struct MyBoxedInt(Box<i32>);
```

Code like this will be generated:

```rust
# struct MyBoxedInt(Box<i32>);
impl derive_more::Deref for MyBoxedInt {
    type Target = <Box<i32> as derive_more::Deref>::Target;
    #[inline]
    fn deref(&self) -> &Self::Target {
        <Box<i32> as derive_more::Deref>::deref(&self.0)
    }
}
```




## Enums

Deriving `Deref` is not supported for enums.

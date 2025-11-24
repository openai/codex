# What `#[derive(DerefMut)]` generates

Deriving `Deref` only works for a single field of a struct.
Furthermore it requires that the type also implements `Deref`, so usually
`Deref` should also be derived.
The resulting implementation of `Deref` will allow you to mutably dereference
the struct its member directly.

1. Dereferencing to the field, i.e. like if your type was a reference type.
2. Doing a dereference on the field, for when the field itself is a reference
   type like `&mut` and `Box`.

With `#[deref_mut]` or `#[deref_mut(ignore)]` it's possible to indicate the
field that you want to derive `DerefMut` for.




## Example usage

```rust
# use derive_more::{Deref, DerefMut};
#
#[derive(Deref, DerefMut)]
struct Num {
    num: i32,
}

#[derive(Deref, DerefMut)]
#[deref(forward)]
#[deref_mut(forward)]
struct MyBoxedInt(Box<i32>);

// You can specify the field you want to derive DerefMut for
#[derive(Deref, DerefMut)]
struct CoolVec {
    cool: bool,
    #[deref]
    #[deref_mut]
    vec: Vec<i32>,
}

let mut num = Num{num: 123};
let mut boxed = MyBoxedInt(Box::new(123));
let mut cool_vec = CoolVec{cool: true, vec: vec![123]};
*num += 123;
assert_eq!(246, *num);
*boxed += 1000;
assert_eq!(1123, *boxed);
cool_vec.push(456);
assert_eq!(vec![123, 456], *cool_vec);
```




## Structs

When deriving a non-forwarded `Deref` for a struct:

```rust
# use derive_more::{Deref, DerefMut};
#
#[derive(Deref, DerefMut)]
struct CoolVec {
    cool: bool,
    #[deref]
    #[deref_mut]
    vec: Vec<i32>,
}
```

Code like this will be generated:

```rust
# use ::core::ops::Deref;
# struct CoolVec {
#     cool: bool,
#     vec: Vec<i32>,
# }
# impl Deref for CoolVec {
#     type Target = Vec<i32>;
#     #[inline]
#     fn deref(&self) -> &Self::Target {
#         &self.vec
#     }
# }
impl derive_more::DerefMut for CoolVec {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vec
    }
}
```

When deriving `DerefMut` for a tuple struct with one field:

```rust
# use derive_more::{Deref, DerefMut};
#
#[derive(Deref, DerefMut)]
#[deref(forward)]
#[deref_mut(forward)]
struct MyBoxedInt(Box<i32>);
```

When deriving a forwarded `DerefMut` for a struct:

```rust
# use ::core::ops::Deref;
# struct MyBoxedInt(Box<i32>);
# impl Deref for MyBoxedInt {
#     type Target = <Box<i32> as Deref>::Target;
#     #[inline]
#     fn deref(&self) -> &Self::Target {
#         <Box<i32> as Deref>::deref(&self.0)
#     }
# }
impl derive_more::DerefMut for MyBoxedInt {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        <Box<i32> as derive_more::DerefMut>::deref_mut(&mut self.0)
    }
}
```




## Enums

Deriving `DerefMut` is not supported for enums.

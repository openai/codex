# Using `#[derive(IntoIterator)]`

Deriving `IntoIterator` only works for a single field of a struct.
The result is that you will call `.into_iter()` on this field directly.

With `#[into_iterator]` or `#[into_iterator(ignore)]` it's possible to indicate
the field that you want to derive `IntoIterator` for.

By using `#[into_iterator(owned, ref, ref_mut)]` it's possible to derive an
`IntoIterator` implementation for reference types as well.
You can pick any combination of `owned`, `ref` and `ref_mut`.
If that's not provided the default is `#[IntoIterator(owned)]`.




## Example usage

```rust
# use derive_more::IntoIterator;
#
#[derive(IntoIterator)]
struct MyVec(Vec<i32>);

// You can specify the field you want to derive `IntoIterator` for
#[derive(IntoIterator)]
struct Numbers {
    #[into_iterator(owned, ref,  ref_mut)]
    numbers: Vec<i32>,
    useless: bool,
}

assert_eq!(Some(5), MyVec(vec![5, 8]).into_iter().next());

let mut nums = Numbers { numbers: vec![100, 200], useless: false };
assert_eq!(Some(&100), (&nums).into_iter().next());
assert_eq!(Some(&mut 100), (&mut nums).into_iter().next());
assert_eq!(Some(100), nums.into_iter().next());
```




## Structs

When deriving `IntoIterator` for a struct:

```rust
# use derive_more::IntoIterator;
#
#[derive(IntoIterator)]
struct Numbers {
    #[into_iterator(owned, ref,  ref_mut)]
    numbers: Vec<i32>,
    useless: bool,
}
```

Code like this will be generated:

```rust
# struct Numbers {
#     numbers: Vec<i32>,
#     useless: bool,
# }
impl IntoIterator for Numbers {
    type Item = <Vec<i32> as IntoIterator>::Item;
    type IntoIter = <Vec<i32> as IntoIterator>::IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        <Vec<i32> as IntoIterator>::into_iter(self.numbers)
    }
}

impl<'__deriveMoreLifetime> IntoIterator for &'__deriveMoreLifetime Numbers {
    type Item = <&'__deriveMoreLifetime Vec<i32> as IntoIterator>::Item;
    type IntoIter = <&'__deriveMoreLifetime Vec<i32> as IntoIterator>::IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        <&'__deriveMoreLifetime Vec<i32> as IntoIterator>::into_iter(&self.numbers)
    }
}

impl<'__deriveMoreLifetime> IntoIterator for &'__deriveMoreLifetime mut Numbers {
    type Item = <&'__deriveMoreLifetime mut Vec<i32> as IntoIterator>::Item;
    type IntoIter = <&'__deriveMoreLifetime mut Vec<i32> as IntoIterator>::IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        <&'__deriveMoreLifetime mut Vec<i32> as IntoIterator>::into_iter(
            &mut self.numbers,
        )
    }
}
```




## Enums

Deriving `IntoIterator` is not supported for enums.

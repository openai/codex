# What `#[derive(IsVariant)]` generates

When an enum is decorated with `#[derive(IsVariant)]`, for each variant `foo` in
the enum, a public instance method `is_foo(&self) -> bool` is generated. If you
don't want the `is_foo` method generated for a variant you can put the
`#[is_variant(ignore)]` attribute on that variant.




## Example usage

```rust
# use derive_more::IsVariant;
#
#[derive(IsVariant)]
enum Maybe<T> {
    Just(T),
    Nothing
}

assert!(Maybe::<()>::Nothing.is_nothing());
assert!(!Maybe::<()>::Nothing.is_just());
```


### What is generated?

The derive in the above example generates code like this:
```rust
# enum Maybe<T> {
#     Just(T),
#     Nothing
# }
impl<T> Maybe<T>{
    #[must_use]
    pub const fn is_just(&self) -> bool {
        matches!(self, Self::Just(..))
    }
    #[must_use]
    pub const fn is_nothing(&self) -> bool {
        matches!(self, Self::Nothing)
    }
}
```

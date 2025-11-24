# What `#[derive(Unwrap)]` generates

When an enum is decorated with `#[derive(Unwrap)]`, for each variant `foo` in the enum, with fields `(a, b, c, ...)` a public instance method `unwrap_foo(self) -> (a, b, c, ...)` is generated.
If you don't want the `unwrap_foo` method generated for a variant, you can put the `#[unwrap(ignore)]` attribute on that variant.
If you want to treat a reference, you can put the `#[unwrap(ref)]` attribute on the enum declaration or that variant, then `unwrap_foo_ref(self) -> (&a, &b, &c, ...)` will be generated. You can also use mutable references by putting `#[unwrap(ref_mut)]`.




## Example usage

```rust
# use derive_more::Unwrap;
# 
# #[derive(Debug, PartialEq)]
#[derive(Unwrap)]
#[unwrap(ref)]
enum Maybe<T> {
    Just(T),
    Nothing,
}

fn main() {
    assert_eq!(Maybe::Just(1).unwrap_just(), 1);

    // Panics if variants are different
    // assert_eq!(Maybe::<()>::Nothing.unwrap_just(), /* panic */);
    // assert_eq!(Maybe::Just(2).unwrap_nothing(), /* panic */);

    assert_eq!((&Maybe::Just(42)).unwrap_just_ref(), &42);
}
```


### What is generated?

The derive in the above example code generates the following code:
```rust
# enum Maybe<T> {
#     Just(T),
#     Nothing,
# }
#
impl<T> Maybe<T> {
    pub fn unwrap_nothing(self) -> () {
        match self {
            Maybe::Nothing => (),
            _ => panic!(),
        }
    }
    pub fn unwrap_nothing_ref(&self) -> () {
        match self {
            Maybe::Nothing => (),
            _ => panic!(),
        }
    }
    pub fn unwrap_just(self) -> T {
        match self {
            Maybe::Just(field_0) => field_0,
            _ => panic!(),
        }
    }
    pub fn unwrap_just_ref(&self) -> &T {
        match self {
            Maybe::Just(field_0) => field_0,
            _ => panic!(),
        }
    }
}
```

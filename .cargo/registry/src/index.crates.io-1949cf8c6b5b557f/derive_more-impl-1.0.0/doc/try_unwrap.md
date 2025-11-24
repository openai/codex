# What `#[derive(TryUnwrap)]` generates

This works almost like `Unwrap`.
When an enum is decorated with `#[derive(TryUnwrap)]`, for each variant `foo` in the enum, with fields `(a, b, c, ...)` a public instance method `try_unwrap_foo(self) -> Result<(a, b, c, ...), TryUnwrapError<Self>>` is generated.
If you don't want the `try_unwrap_foo` method generated for a variant, you can put the `#[try_unwrap(ignore)]` attribute on that variant.
If you want to treat a reference, you can put the `#[try_unwrap(ref)]` attribute on the enum declaration or that variant, then `try_unwrap_foo_ref(self) -> Result<(&a, &b, &c, ...), TryUnwrapError<&Self>>` will be generated. You can also use mutable references by putting `#[unwrap(ref_mut)]`.
However, unlike `Unwrap`, it does not panic if the conversion fails. Also, values that fail to convert are not dropped but returned as an `Err`.

## Example usage

```rust
# use derive_more::TryUnwrap;
# 
# #[derive(Debug, PartialEq)]
#[derive(TryUnwrap)]
#[try_unwrap(ref)]
enum Maybe<T> {
    Nothing,
    Just(T),
}

fn main() {
    assert_eq!(Maybe::Just(1).try_unwrap_just(), Ok(1));

    // Unlike `Unwrap`, it does not panic.
    assert_eq!(
        Maybe::<()>::Nothing.try_unwrap_just().map_err(|err| err.input),
        Err(Maybe::<()>::Nothing), // and the value is returned!
    );
    assert_eq!(
        Maybe::Just(2).try_unwrap_nothing().map_err(|err| err.input),
        Err(Maybe::Just(2)),
    );
    assert_eq!(
        Maybe::<()>::Nothing.try_unwrap_just().map_err(|err| err.to_string()),
        Err("Attempt to call `Maybe::try_unwrap_just()` on a `Maybe::Nothing` value".into()),
    );

    assert_eq!((&Maybe::Just(42)).try_unwrap_just_ref(), Ok(&42));
}
```

### What is generated?

The derive in the above example code generates the following code:
```rust
# use derive_more::TryUnwrapError;
#
# enum Maybe<T> {
#     Just(T),
#     Nothing,
# }
#
impl<T> Maybe<T> {
    pub fn try_unwrap_nothing(self) -> Result<(), TryUnwrapError<Self>> {
        match self {
            Maybe::Nothing => Ok(()),
            val @ _ => Err(todo!("TryUnwrapError::new(val, /* omitted */)")),
        }
    }
    pub fn try_unwrap_nothing_ref(&self) -> Result<(), TryUnwrapError<&Self>> {
        match self {
            Maybe::Nothing => Ok(()),
            val @ _ => Err(todo!("TryUnwrapError::new(val, /* omitted */)")),
        }
    }
    pub fn try_unwrap_just(self) -> Result<T, TryUnwrapError<Self>> {
        match self {
            Maybe::Just(field_0) => Ok(field_0),
            val @ _ => Err(todo!("TryUnwrapError::new(val, /* omitted */)")),
        }
    }
    pub fn try_unwrap_just_ref(&self) -> Result<&T, TryUnwrapError<&Self>> {
        match self {
            Maybe::Just(field_0) => Ok(field_0),
            val @ _ => Err(todo!("TryUnwrapError::new(val, /* omitted */)")),
        }
    }
}
```

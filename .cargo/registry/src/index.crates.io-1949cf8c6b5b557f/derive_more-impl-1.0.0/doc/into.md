# What `#[derive(Into)]` generates

This derive creates the exact opposite of `#[derive(From)]`.
Instead of allowing you to create a new instance of the struct from the values
it should contain, it allows you to extract the values from the struct. One
thing to note is that this derive doesn't actually generate an implementation
for the `Into` trait. Instead, it derives `From` for the values contained in
the struct and thus has an indirect implementation of `Into` as
[recommended by the docs][1].




## Structs

For structs with a single field you can call `.into()` to extract the inner type.

```rust
# use derive_more::Into;
#
#[derive(Debug, Into, PartialEq)]
struct Int(i32);

assert_eq!(2, Int(2).into());
```

For structs having multiple fields, `.into()` extracts a tuple containing the
desired content for each field.

```rust
# use derive_more::Into;
#
#[derive(Debug, Into, PartialEq)]
struct Point(i32, i32);

assert_eq!((1, 2), Point(1, 2).into());
```

To specify concrete types for deriving conversions into, use `#[into(<types>)]`.

```rust
# use std::borrow::Cow;
#
# use derive_more::Into;
#
#[derive(Debug, Into, PartialEq)]
#[into(Cow<'static, str>, String)]
struct Str(Cow<'static, str>);

assert_eq!("String".to_owned(), String::from(Str("String".into())));
assert_eq!(Cow::Borrowed("Cow"), <Cow<_>>::from(Str("Cow".into())));

#[derive(Debug, Into, PartialEq)]
#[into((i64, i64), (i32, i32))]
struct Point {
    x: i32,
    y: i32,
}

assert_eq!((1_i64, 2_i64), Point { x: 1_i32, y: 2_i32 }.into());
assert_eq!((3_i32, 4_i32), Point { x: 3_i32, y: 4_i32 }.into());
```

In addition to converting to owned types, this macro supports deriving into
reference (mutable or not) via `#[into(ref(...))]`/`#[into(ref_mut(...))]`.

```rust
# use derive_more::Into;
#
#[derive(Debug, Into, PartialEq)]
#[into(owned, ref(i32), ref_mut)]
struct Int(i32);

assert_eq!(2, Int(2).into());
assert_eq!(&2, <&i32>::from(&Int(2)));
assert_eq!(&mut 2, <&mut i32>::from(&mut Int(2)));
```

In case there are fields, that shouldn't be included in the conversion, use the
`#[into(skip)]` (or `#[into(ignore)]`) attribute.

```rust
# use std::marker::PhantomData;
#
# use derive_more::Into;
#
# struct Gram;
#
#[derive(Debug, Into, PartialEq)]
#[into(i32, i64, i128)]
struct Mass<Unit> {
    value: i32,
    #[into(skip)] // or #[into(ignore)]
    _unit: PhantomData<Unit>,
}

assert_eq!(5, Mass::<Gram>::new(5).into());
assert_eq!(5_i64, Mass::<Gram>::new(5).into());
assert_eq!(5_i128, Mass::<Gram>::new(5).into());
#
# impl<Unit> Mass<Unit> {
#     fn new(value: i32) -> Self {
#         Self {
#             value,
#             _unit: PhantomData,
#         }
#     }
# }
```


### Fields

The `#[into]` attribute can also be applied to specific fields of a struct.

```rust
# use derive_more::Into;
#
#[derive(Into)]
struct Data {
    id: i32,
    #[into]
    raw: f64
}

assert_eq!(42.0, Data { id: 1, raw: 42.0 }.into());
```

In such cases, no conversion into a tuple of all fields is generated, unless
an explicit struct attribute is present.

```rust
# use derive_more::Into;
#
#[derive(Into)]
#[into]
struct Data {
    id: i32,
    #[into]
    raw: f64
}

assert_eq!(42.0, Data { id: 1, raw: 42.0 }.into());
assert_eq!((1, 42.0), Data { id: 1, raw: 42.0 }.into());
```

The `#[into(<types>)]` syntax can be used on fields as well.

```rust
# use std::marker::PhantomData;
# use derive_more::Into;
# struct Whatever;
#
#[derive(Into, Clone)]
#[into(owned, ref((u8, str)), ref_mut)]
struct Foo {
   #[into(owned(u64), ref)]
   a: u8,
   b: String,
   #[into(skip)]
   _c: PhantomData<Whatever>,
}

let mut foo = Foo { a: 1, b: "string".to_owned(), _c: PhantomData };

assert_eq!((1_u8, "string".to_owned()), foo.clone().into());
assert_eq!((&1_u8, "string"), <(&u8, &str)>::from(&foo));
assert_eq!((&mut 1_u8, &mut "string".to_owned()), <(&mut u8, &mut String)>::from(&mut foo));
assert_eq!(1_u64, foo.clone().into());
assert_eq!(&1_u8, <&u8>::from(&foo));
```

Fields, having specific conversions into them, can also be skipped for top-level
tuple conversions.

```rust
# use derive_more::Into;

#[derive(Into)]
#[into(ref((str, f64)))]
struct Foo {
    #[into(ref)]
    #[into(skip)]
    a: u8,
    b: String,
    c: f64,
}

let foo = Foo { a: 1, b: "string".to_owned(), c: 3.0 };

assert_eq!(("string", &3.0), (&foo).into());
assert_eq!(&1_u8, <&u8>::from(&foo));
```




## Enums

Deriving `Into` for enums is not supported as it would not always be successful,
so `TryInto` should be used instead.




[1]: https://doc.rust-lang.org/core/convert/trait.Into.html

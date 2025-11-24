# What `#[derive(AsRef)]` generates

Deriving `AsRef` generates one or more implementations of `AsRef`, each
corresponding to one of the fields of the decorated type.
This allows types which contain some `T` to be passed anywhere that an
`AsRef<T>` is accepted.




## Newtypes and Structs with One Field

When `AsRef` is derived for a newtype or struct with one field, a single
implementation is generated to expose the underlying field.

```rust
# use derive_more::AsRef;
#
#[derive(AsRef)]
struct MyWrapper(String);
```

Generates:

```rust
# struct MyWrapper(String);
impl derive_more::AsRef<String> for MyWrapper {
    fn as_ref(&self) -> &String {
        &self.0
    }
}
```

It's also possible to use the `#[as_ref(forward)]` attribute to forward
to the `as_ref` implementation of the field. So here `SingleFieldForward`
implements all `AsRef` for all types that `Vec<i32>` implements `AsRef` for.

```rust
# use derive_more::AsRef;
#
#[derive(AsRef)]
#[as_ref(forward)]
struct SingleFieldForward(Vec<i32>);

let item = SingleFieldForward(vec![]);
let _: &[i32] = (&item).as_ref();
```

This generates code equivalent to:

```rust
# struct SingleFieldForward(Vec<i32>);
impl<T: ?Sized> derive_more::AsRef<T> for SingleFieldForward
where
    Vec<i32>: derive_more::AsRef<T>,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}
```

Specifying concrete types, to derive impls for, is also supported via
`#[as_ref(<types>)]` attribute. These types can include both the type
of the field itself, and types for which the field type implements `AsRef`.

```rust
# use derive_more::AsRef;
#
#[derive(AsRef)]
#[as_ref(str, [u8], String)]
struct Types(String);

let item = Types("test".to_owned());
let _: &str = item.as_ref();
let _: &[u8] = item.as_ref();
let _: &String = item.as_ref();
```

> **WARNING**: When either the field type, or the specified conversion type,
> contains generic parameters, they are considered as the same type only if
> are named string-equally, otherwise are assumed as different types even
> when represent the same type in fact (type aliases, for example).
>
> ```rust
> # use derive_more::AsRef;
> #
> #[derive(AsRef)]
> #[as_ref(i32)] // generates `impl<T: AsRef<i32>> AsRef<i32> for Generic<T>`
> struct Generic<T>(T);
>
> #[derive(AsRef)]
> #[as_ref(T)] // generates `impl<T> AsRef<T> for Transparent<T>`
> struct Transparent<T>(T);
>
> #[derive(AsRef)]
> // #[as_ref(RenamedVec<T>)] // not supported, as types are not named string-equally
> struct Foo<T>(Vec<T>);
> type RenamedVec<T> = Vec<T>;
>
> #[derive(AsRef)]
> #[as_ref(RenamedString)] // generates `impl AsRef<RenamedString> for Bar`,
> struct Bar(String);      // as generics are not involved
> type RenamedString = String;
> ```

Generating code like this is not supported:

```rust
struct Generic<T>(T);

impl AsRef<i32> for Generic<i32> {
    fn as_ref(&self) -> &i32 {
        &self.0
    }
}
```




## Structs with Multiple Fields

When `AsRef` is derived for a struct with more than one field (including tuple
structs), you must also mark one or more fields with the `#[as_ref]` attribute.
An implementation will be generated for each indicated field.

```rust
# use derive_more::AsRef;
#
#[derive(AsRef)]
struct MyWrapper {
    #[as_ref(str)]
    name: String,
    #[as_ref]
    num: i32,
    valid: bool,
}
```

Generates:

```rust
# struct MyWrapper {
#     name: String,
#     num: i32,
#     valid: bool,
# }
impl AsRef<str> for MyWrapper {
    fn as_ref(&self) -> &str {
        self.name.as_ref()
    }
}

impl AsRef<i32> for MyWrapper {
    fn as_ref(&self) -> &i32 {
        &self.num
    }
}
```


### Tuples (not supported)

Only conversions that use a single field are possible with this derive.
Something like this wouldn't work, due to the nature of the `AsRef` trait
itself:

```rust,compile_fail
# use derive_more::AsRef
#
#[derive(AsRef)]
#[as_ref((str, [u8]))]
struct MyWrapper(String, Vec<u8>)
```

If you need to convert into a tuple of references, consider using the
[`Into`](crate::Into) derive with `#[into(ref)]`.


### Skipping

Or vice versa: you can exclude a specific field by using `#[as_ref(skip)]` (or
`#[as_ref(ignore)]`). Then, implementations will be generated for non-indicated fields.

```rust
# use derive_more::AsRef;
#
#[derive(AsRef)]
struct MyWrapper {
    #[as_ref(skip)]
    name: String,
    #[as_ref(ignore)]
    num: i32,
    valid: bool,
}
```

Generates:

```rust
# struct MyWrapper {
#     name: String,
#     num: i32,
#     valid: bool,
# }
impl AsRef<bool> for MyWrapper {
    fn as_ref(&self) -> &bool {
        &self.valid
    }
}
```


### Coherence

Note that `AsRef<T>` may only be implemented once for any given type `T`.
This means any attempt to mark more than one field of the same type with
`#[as_ref]` will result in a compilation error.

```rust,compile_fail
# use derive_more::AsRef;
#
// Error! Conflicting implementations of AsRef<String>
#[derive(AsRef)]
struct MyWrapper {
    #[as_ref]
    str1: String,
    #[as_ref]
    str2: String,
}
```

Similarly, if some field is annotated with `#[as_ref(forward)]`, no other
field can be marked.

```rust,compile_fail
# use derive_more::AsRef;
#
// Error! Conflicting implementations of `AsRef<i32>`
// note: upstream crates may add a new impl of trait `AsRef<i32>`
// for type `String` in future versions
#[derive(AsRef)]
struct ForwardWithOther {
    #[as_ref(forward)]
    str: String,
    #[as_ref]
    number: i32,
}
```

Multiple forwarded impls with different concrete types, however, can be used.

```rust
# use derive_more::AsRef;
#
#[derive(AsRef)]
struct Types {
    #[as_ref(str)]
    str: String,
    #[as_ref([u8])]
    vec: Vec<u8>,
}

let item = Types {
    str: "test".to_owned(),
    vec: vec![0u8],
};

let _: &str = item.as_ref();
let _: &[u8] = item.as_ref();
```




## Enums

Deriving `AsRef` for enums is not supported.

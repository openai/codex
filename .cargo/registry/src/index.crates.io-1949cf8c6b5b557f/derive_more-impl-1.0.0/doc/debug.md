# What `#[derive(Debug)]` generates

This derive macro is a clever superset of `Debug` from standard library. Additional features include:
- not imposing redundant trait bounds;
- `#[debug(skip)]` (or `#[debug(ignore)]`) attribute to skip formatting struct field or enum variant;
- `#[debug("...", args...)]` to specify custom formatting either for the whole struct or enum variant, or its particular field;
- `#[debug(bounds(...))]` to impose additional custom trait bounds.




## The format of the format

You supply a format by placing an attribute on a struct or enum variant, or its particular field:
`#[debug("...", args...)]`. The format is exactly like in [`format!()`] or any other [`format_args!()`]-based macros.

The variables available in the arguments is `self` and each member of the
struct or enum variant, with members of tuple structs being named with a
leading underscore and their index, i.e. `_0`, `_1`, `_2`, etc. Due to
ownership/lifetime limitations the member variables are all references to the
fields, except when used directly in the format string. For most purposes this
detail doesn't matter, but it is quite important when using `Pointer`
formatting. If you don't use the `{field:p}` syntax, you have to dereference
once to get the address of the field itself, instead of the address of the
reference to the field:

```rust
use derive_more::Debug;

#[derive(Debug)]
#[debug("{field:p} {:p}", *field)]
struct RefInt<'a> {
    field: &'a i32,
}

let a = &123;
assert_eq!(format!("{:?}", RefInt{field: &a}), format!("{a:p} {:p}", a));
```


### Generic data types

When deriving `Debug` for a generic struct/enum, all generic type arguments _used_ during formatting
are bound by respective formatting trait.

E.g., for a structure `Foo` defined like this:
```rust
use derive_more::Debug;

#[derive(Debug)]
struct Foo<'a, T1, T2: Trait, T3, T4> {
    #[debug("{a}")]
    a: T1,
    #[debug("{b}")]
    b: <T2 as Trait>::Type,
    #[debug("{c:?}")]
    c: Vec<T3>,
    #[debug("{d:p}")]
    d: &'a T1,
    #[debug(skip)] // or #[debug(ignore)]
    e: T4,
}

trait Trait { type Type; }
```

The following where clauses would be generated:
- `T1: Display`
- `<T2 as Trait>::Type: Display`
- `Vec<T3>: Debug`
- `&'a T1: Pointer`


### Custom trait bounds

Sometimes you may want to specify additional trait bounds on your generic type parameters, so that they could be used
during formatting. This can be done with a `#[debug(bound(...))]` attribute.

`#[debug(bound(...))]` accepts code tokens in a format similar to the format used in angle bracket list (or `where`
clause predicates): `T: MyTrait, U: Trait1 + Trait2`.

Using `#[debug("...", ...)]` formatting we'll try our best to infer trait bounds, but in more advanced cases this isn't
possible. Our aim is to avoid imposing additional bounds, as they can be added with `#[debug(bound(...))]`.
In the example below, we can infer only that `V: Display`, other bounds have to be supplied by the user:

```rust
use std::fmt::Display;
use derive_more::Debug;

#[derive(Debug)]
#[debug(bound(T: MyTrait, U: Display))]
struct MyStruct<T, U, V, F> {
    #[debug("{}", a.my_function())]
    a: T,
    #[debug("{}", b.to_string().len())]
    b: U,
    #[debug("{c}")]
    c: V,
    #[debug(skip)] // or #[debug(ignore)]
    d: F,
}

trait MyTrait { fn my_function(&self) -> i32; }
```


### Transparency

If the top-level `#[debug("...", args...)]` attribute (the one for a whole struct or variant) is specified
and can be trivially substituted with a transparent delegation call to the inner type, then all the additional
[formatting parameters][1] do work as expected:
```rust
use derive_more::Debug;

#[derive(Debug)]
#[debug("{_0:o}")] // the same as calling `Octal::fmt()`
struct MyOctalInt(i32);

// so, additional formatting parameters do work transparently
assert_eq!(format!("{:03?}", MyOctalInt(9)), "011");

#[derive(Debug)]
#[debug("{_0:02b}")]     // cannot be trivially substituted with `Binary::fmt()`,
struct MyBinaryInt(i32); // because of specified formatting parameters

// so, additional formatting parameters have no effect
assert_eq!(format!("{:07?}", MyBinaryInt(2)), "10");
```

If, for some reason, transparency in trivial cases is not desired, it may be suppressed explicitly
either with the [`format_args!()`] macro usage:
```rust
use derive_more::Debug;

#[derive(Debug)]
#[debug("{}", format_args!("{_0:o}"))] // `format_args!()` obscures the inner type
struct MyOctalInt(i32);

// so, additional formatting parameters have no effect
assert_eq!(format!("{:07?}", MyOctalInt(9)), "11");
```
Or by adding [formatting parameters][1] which cause no visual effects:
```rust
use derive_more::Debug;

#[derive(Debug)]
#[debug("{_0:^o}")] // `^` is centering, but in absence of additional width has no effect
struct MyOctalInt(i32);

// and so, additional formatting parameters have no effect
assert_eq!(format!("{:07?}", MyOctalInt(9)), "11");
```




## Example usage

```rust
use std::path::PathBuf;
use derive_more::Debug;

#[derive(Debug)]
struct MyInt(i32);

#[derive(Debug)]
struct MyIntHex(#[debug("{_0:x}")] i32);

#[derive(Debug)]
#[debug("{_0} = {_1}")]
struct StructFormat(&'static str, u8);

#[derive(Debug)]
enum E {
    Skipped {
        x: u32,
        #[debug(skip)] // or #[debug(ignore)]
        y: u32,
    },
    Binary {
        #[debug("{i:b}")]
        i: i8,
    },
    Path(#[debug("{}", _0.display())] PathBuf),
    #[debug("{_0}")]
    EnumFormat(bool)
}

assert_eq!(format!("{:?}", MyInt(-2)), "MyInt(-2)");
assert_eq!(format!("{:?}", MyIntHex(-255)), "MyIntHex(ffffff01)");
assert_eq!(format!("{:?}", StructFormat("answer", 42)), "answer = 42");
assert_eq!(format!("{:?}", E::Skipped { x: 10, y: 20 }), "Skipped { x: 10, .. }");
assert_eq!(format!("{:?}", E::Binary { i: -2 }), "Binary { i: 11111110 }");
assert_eq!(format!("{:?}", E::Path("abc".into())), "Path(abc)");
assert_eq!(format!("{:?}", E::EnumFormat(true)), "true");
```

[`format!()`]: https://doc.rust-lang.org/stable/std/macro.format.html
[`format_args!()`]: https://doc.rust-lang.org/stable/std/macro.format_args.html

[1]: https://doc.rust-lang.org/stable/std/fmt/index.html#formatting-parameters
